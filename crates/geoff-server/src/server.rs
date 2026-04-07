use std::collections::HashMap;
use std::sync::Arc;

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use camino::Utf8PathBuf;
use geoff_core::config::SiteConfig;
use geoff_graph::store::ContentStore;
use geoff_render::pipeline::build_to_memory;
use geoff_render::renderer::SiteRenderer;
use tokio::sync::{RwLock, broadcast};

use crate::watcher::FileWatcher;

/// Shared state for the dev server.
pub struct DevState {
    /// In-memory page cache: URL path -> rendered HTML.
    pub pages: RwLock<HashMap<String, String>>,
    /// The RDF store for SPARQL queries.
    pub store: Arc<ContentStore>,
    /// Broadcast channel for WebSocket reload notifications.
    pub reload_tx: Arc<broadcast::Sender<()>>,
    /// Site config.
    pub config: SiteConfig,
    /// Site root path.
    pub site_root: Utf8PathBuf,
}

/// Hot-reload script injected into every page in dev mode.
const HOT_RELOAD_SCRIPT: &str = r#"<script>
(function() {
    const ws = new WebSocket(`ws://${location.host}/ws`);
    ws.onmessage = function(event) {
        if (event.data === 'reload') {
            location.reload();
        }
    };
    ws.onclose = function() {
        setTimeout(function() { location.reload(); }, 1000);
    };
})();
</script>"#;

/// Start the dev server.
pub async fn run(
    site_root: Utf8PathBuf,
    port: u16,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let config_path = site_root.join("geoff.toml");
    let config = SiteConfig::from_file(&config_path)?;

    let store = Arc::new(ContentStore::new()?);
    let template_dir = site_root.join(&config.template_dir);
    let mut renderer = SiteRenderer::new(&template_dir)?;
    renderer.register_sparql_function(Arc::clone(&store));

    // Initial full build
    let pages = build_to_memory(&site_root, &config, &store, &renderer)?;
    let page_count = pages.len();
    eprintln!("Built {page_count} page(s)");

    let (reload_tx, _) = broadcast::channel::<()>(16);
    let reload_tx = Arc::new(reload_tx);

    let state = Arc::new(DevState {
        pages: RwLock::new(pages),
        store: Arc::clone(&store),
        reload_tx: Arc::clone(&reload_tx),
        config: config.clone(),
        site_root: site_root.clone(),
    });

    // Set up file watcher
    let content_dir = site_root.join(&config.content_dir);
    let watch_dirs: Vec<std::path::PathBuf> = [
        content_dir.as_std_path().to_path_buf(),
        template_dir.as_std_path().to_path_buf(),
        site_root.join("ontology").as_std_path().to_path_buf(),
        config_path.as_std_path().to_path_buf(),
    ]
    .to_vec();

    let watch_refs: Vec<&std::path::Path> = watch_dirs.iter().map(|p| p.as_path()).collect();
    let _watcher = FileWatcher::new(&watch_refs, Arc::clone(&reload_tx))?;

    // Spawn rebuild task
    let rebuild_state = Arc::clone(&state);
    tokio::spawn(async move {
        let mut rx = rebuild_state.reload_tx.subscribe();
        loop {
            if rx.recv().await.is_err() {
                break;
            }
            // Debounce: drain any pending notifications
            while rx.try_recv().is_ok() {}

            // Small delay to let file writes finish
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Rebuild
            let rebuild_result = tokio::task::spawn_blocking({
                let state = Arc::clone(&rebuild_state);
                move || -> std::result::Result<HashMap<String, String>, String> {
                    let template_dir = state.site_root.join(&state.config.template_dir);
                    let mut renderer =
                        SiteRenderer::new(&template_dir).map_err(|e| e.to_string())?;
                    renderer.register_sparql_function(Arc::clone(&state.store));
                    state.store.clear().map_err(|e| e.to_string())?;
                    build_to_memory(&state.site_root, &state.config, &state.store, &renderer)
                        .map_err(|e| e.to_string())
                }
            })
            .await;

            match rebuild_result {
                Ok(Ok(new_pages)) => {
                    let count = new_pages.len();
                    *rebuild_state.pages.write().await = new_pages;
                    eprintln!("Rebuilt {count} page(s)");
                }
                Ok(Err(e)) => eprintln!("Rebuild error: {e}"),
                Err(e) => eprintln!("Rebuild task panic: {e}"),
            }
        }
    });

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/api/sparql", get(sparql_handler).post(sparql_handler_post))
        .nest("/api", crate::api::api_router())
        .route("/__geoff__/", get(geoff_ui_handler))
        .route(
            "/__geoff__/components/{*path}",
            get(geoff_component_handler),
        )
        .route("/__geoff__/{*rest}", get(geoff_ui_handler))
        .fallback(get(page_handler))
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    eprintln!("Dev server listening on http://localhost:{port}");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    // Keep watcher alive
    drop(_watcher);
    Ok(())
}

/// Serve a page from the in-memory cache, injecting hot-reload script.
async fn page_handler(
    State(state): State<Arc<DevState>>,
    uri: axum::http::Uri,
) -> impl IntoResponse {
    let path = uri.path().to_string();
    let pages = state.pages.read().await;

    // Try exact path, then with .html extension
    let html = pages
        .get(&path)
        .or_else(|| pages.get(&format!("{path}.html")))
        .or_else(|| {
            let with_index = if path.ends_with('/') {
                format!("{path}index.html")
            } else {
                format!("{path}/index.html")
            };
            pages.get(&with_index)
        });

    match html {
        Some(content) => {
            // Inject hot-reload script before </body>
            let injected = if let Some(pos) = content.rfind("</body>") {
                format!(
                    "{}{HOT_RELOAD_SCRIPT}\n{}",
                    &content[..pos],
                    &content[pos..]
                )
            } else {
                format!("{content}\n{HOT_RELOAD_SCRIPT}")
            };
            Html(injected).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Html("<h1>404 Not Found</h1>".to_string()),
        )
            .into_response(),
    }
}

/// WebSocket handler for hot reload.
async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<DevState>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: Arc<DevState>) {
    let mut rx = state.reload_tx.subscribe();
    while let Ok(()) = rx.recv().await {
        // Small delay to allow rebuild to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        if socket.send(Message::Text("reload".into())).await.is_err() {
            break;
        }
    }
}

/// SPARQL query parameters.
#[derive(serde::Deserialize)]
struct SparqlQuery {
    query: String,
}

/// Dev-only SPARQL endpoint (GET).
async fn sparql_handler(
    State(state): State<Arc<DevState>>,
    Query(params): Query<SparqlQuery>,
) -> impl IntoResponse {
    match state.store.query_to_json(&params.query) {
        Ok(result) => {
            let json = serde_json::to_string_pretty(&result).unwrap_or_default();
            (StatusCode::OK, [("content-type", "application/json")], json).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            [("content-type", "application/json")],
            serde_json::json!({"error": e.to_string()}).to_string(),
        )
            .into_response(),
    }
}

/// Authoring UI shell served at `/__geoff__/`.
async fn geoff_ui_handler() -> Html<&'static str> {
    Html(crate::ui::AUTHORING_UI_HTML)
}

/// Serve web component JS files from `components/` directory.
async fn geoff_component_handler(
    State(state): State<Arc<DevState>>,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> impl IntoResponse {
    let file_path = state.site_root.join("components").join(&path);
    match std::fs::read_to_string(&file_path) {
        Ok(content) => (
            StatusCode::OK,
            [("content-type", "application/javascript")],
            content,
        )
            .into_response(),
        Err(_) => (
            StatusCode::NOT_FOUND,
            format!("Component not found: {path}"),
        )
            .into_response(),
    }
}

/// Dev-only SPARQL endpoint (POST).
async fn sparql_handler_post(
    State(state): State<Arc<DevState>>,
    axum::Json(body): axum::Json<SparqlBody>,
) -> impl IntoResponse {
    match state.store.query_to_json(&body.query) {
        Ok(result) => {
            let json = serde_json::to_string_pretty(&result).unwrap_or_default();
            (StatusCode::OK, [("content-type", "application/json")], json).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            [("content-type", "application/json")],
            serde_json::json!({"error": e.to_string()}).to_string(),
        )
            .into_response(),
    }
}

#[derive(serde::Deserialize)]
struct SparqlBody {
    query: String,
}

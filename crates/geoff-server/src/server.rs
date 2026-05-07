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
use geoff_render::pipeline::{ingest_content, render_pages};
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
    /// Uploaded preview pages: ID -> raw HTML (before CSS injection).
    pub preview_pages: RwLock<HashMap<String, String>>,
    /// Plugin registry for lifecycle hooks.
    pub registry: Arc<tokio::sync::Mutex<geoff_plugin::registry::PluginRegistry>>,
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

    // Build renderer with layered template directories if a theme is configured
    let mut renderer = if let Some(ref theme_name) = config.theme.name {
        let mut dirs = vec![site_root.join("themes").join(theme_name).join("templates")];
        if let Some(ref base_name) = config.theme.base {
            dirs.push(site_root.join("themes").join(base_name).join("templates"));
        }
        dirs.push(template_dir.clone());
        let dir_refs: Vec<&camino::Utf8Path> = dirs.iter().map(|d| d.as_path()).collect();
        SiteRenderer::with_theme_dirs(&dir_refs)?
    } else {
        SiteRenderer::new(&template_dir)?
    };
    renderer.register_sparql_function(Arc::clone(&store));
    renderer.register_component_function(site_root.join("components"));
    renderer.register_devspaces_function(&config.devspaces);

    // Register RDFa template helpers
    if config.linked_data.rdfa {
        let mappings_path = site_root.join("ontology/mappings.toml");
        let mut rdfa_registry =
            geoff_ontology::mappings::MappingRegistry::load(&mappings_path).unwrap_or_default();
        if !config.linked_data.prefixes.is_empty() {
            rdfa_registry.add_prefixes(config.linked_data.prefixes.clone());
        }
        renderer.register_rdfa_functions(Arc::clone(&store), Arc::new(rdfa_registry));
    }

    // Load and register theme tokens
    let _theme_result = geoff_render::pipeline::load_and_register_theme(
        &site_root,
        &config,
        &mut renderer,
        &store,
    )?;

    // Load plugins
    let mut registry = geoff_plugin::registry::PluginRegistry::new();
    for plugin_cfg in &config.plugins {
        eprintln!(
            "  Loading plugin: {} ({:?})",
            plugin_cfg.name, plugin_cfg.runtime
        );
        match plugin_cfg.runtime {
            geoff_core::config::PluginRuntime::Rust => {
                let lib_path = site_root.join(&plugin_cfg.path);
                let mut loader = geoff_plugin::loader::RustPluginLoader::new();
                unsafe {
                    if let Err(e) = loader.load(lib_path.as_std_path()) {
                        eprintln!(
                            "  Warning: Failed to load plugin '{}': {e}",
                            plugin_cfg.name
                        );
                    }
                }
                registry.register_all(loader.into_plugins());
            }
            geoff_core::config::PluginRuntime::Deno => {
                let script_path = site_root.join(&plugin_cfg.path);
                match geoff_deno::plugin::DenoPlugin::new(&plugin_cfg.name, script_path.as_str())
                    .await
                {
                    Ok(p) => registry.register(Box::new(p)),
                    Err(e) => {
                        eprintln!(
                            "  Warning: Failed to load Deno plugin '{}': {e}",
                            plugin_cfg.name
                        );
                    }
                }
            }
        }
    }

    // Dispatch on_init
    let plugin_options: std::collections::HashMap<
        String,
        std::collections::HashMap<String, toml::Value>,
    > = config
        .plugins
        .iter()
        .map(|p| (p.name.clone(), p.options.clone()))
        .collect();
    if let Err(e) = registry.dispatch_init(&config, &plugin_options).await {
        eprintln!("  Plugin init warning: {e}");
    }
    if let Err(e) = registry.dispatch_build_start(&config, &store).await {
        eprintln!("  Plugin build_start warning: {e}");
    }

    let registry = Arc::new(tokio::sync::Mutex::new(registry));

    // Initial full build using three-phase pipeline with plugin hooks
    let pages = build_with_hooks_async(&site_root, &config, &store, &renderer, &registry).await?;
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
        preview_pages: RwLock::new(HashMap::new()),
        registry: Arc::clone(&registry),
    });

    // Set up file watcher
    let content_dir = site_root.join(&config.content_dir);
    let watch_dirs: Vec<std::path::PathBuf> = [
        content_dir.as_std_path().to_path_buf(),
        template_dir.as_std_path().to_path_buf(),
        site_root.join("ontology").as_std_path().to_path_buf(),
        site_root.join("themes").as_std_path().to_path_buf(),
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
            while rx.try_recv().is_ok() {}
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Rebuild: sync parts in spawn_blocking, then async plugin hooks
            let state_clone = Arc::clone(&rebuild_state);
            let rebuild_result = async {
                // Sync: create renderer, ingest content, render pages
                let (_renderer, pages) = tokio::task::spawn_blocking({
                    let state = Arc::clone(&state_clone);
                    move || -> std::result::Result<(SiteRenderer, HashMap<String, String>), String>
                    {
                        let template_dir = state.site_root.join(&state.config.template_dir);
                        let mut renderer = if let Some(ref theme_name) = state.config.theme.name {
                            let mut dirs = vec![state
                                .site_root
                                .join("themes")
                                .join(theme_name)
                                .join("templates")];
                            if let Some(ref base_name) = state.config.theme.base {
                                dirs.push(
                                    state
                                        .site_root
                                        .join("themes")
                                        .join(base_name)
                                        .join("templates"),
                                );
                            }
                            dirs.push(template_dir.clone());
                            let dir_refs: Vec<&camino::Utf8Path> =
                                dirs.iter().map(|d| d.as_path()).collect();
                            SiteRenderer::with_theme_dirs(&dir_refs).map_err(|e| e.to_string())?
                        } else {
                            SiteRenderer::new(&template_dir).map_err(|e| e.to_string())?
                        };
                        renderer.register_sparql_function(Arc::clone(&state.store));
                        renderer
                            .register_component_function(state.site_root.join("components"));
                        renderer.register_devspaces_function(&state.config.devspaces);
                        if state.config.linked_data.rdfa {
                            let mappings_path = state.site_root.join("ontology/mappings.toml");
                            let mut rdfa_registry =
                                geoff_ontology::mappings::MappingRegistry::load(&mappings_path)
                                    .unwrap_or_default();
                            if !state.config.linked_data.prefixes.is_empty() {
                                rdfa_registry
                                    .add_prefixes(state.config.linked_data.prefixes.clone());
                            }
                            renderer.register_rdfa_functions(
                                Arc::clone(&state.store),
                                Arc::new(rdfa_registry),
                            );
                        }
                        state.store.clear().map_err(|e| e.to_string())?;
                        let _theme = geoff_render::pipeline::load_and_register_theme(
                            &state.site_root,
                            &state.config,
                            &mut renderer,
                            &state.store,
                        )
                        .map_err(|e| e.to_string())?;

                        // Phase 1: Ingest
                        let (to_render, stats, page_index) = ingest_content(
                            &state.site_root,
                            &state.config,
                            &state.store,
                            None,
                        )
                        .map_err(|e| e.to_string())?;
                        renderer.set_page_index(page_index);

                        // Phase 2: Render (hooks will run async after this)
                        let pages =
                            render_pages(&to_render, &state.config, &renderer, stats.skipped)
                                .map_err(|e| e.to_string())?;

                        let mut map = HashMap::new();
                        for page in pages {
                            let normalized =
                                geoff_core::types::normalize_path(&page.output_path);
                            let url_path = if normalized == "index.html" {
                                "/".to_string()
                            } else {
                                format!("/{normalized}")
                            };
                            map.insert(url_path, page.html);
                        }

                        if state.config.search.enabled {
                            if state.config.search.partition.is_some()
                                && let Ok(partitions) = state.store.export_partitioned_ntriples(
                                    &|graph: &str| {
                                        let path = graph
                                            .strip_prefix("<urn:geoff:content:")?
                                            .strip_suffix('>')?;
                                        let first_slash = path.find('/')?;
                                        Some(path[..first_slash].to_string())
                                    },
                                )
                            {
                                let mut manifest = String::new();
                                for (key, nt) in &partitions {
                                    if key.is_empty() {
                                        continue;
                                    }
                                    map.insert(format!("/search/{key}.nt"), nt.clone());
                                    use std::fmt::Write;
                                    let _ = writeln!(manifest, "<{}/search/{key}.nt> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <urn:geoff:SearchGraph> .", state.config.base_url.trim_end_matches('/'));
                                    let _ = writeln!(manifest, "<{}/search/{key}.nt> <urn:geoff:graphName> \"{key}\" .", state.config.base_url.trim_end_matches('/'));
                                }
                                let mut main_nt =
                                    partitions.get("").cloned().unwrap_or_default();
                                main_nt.push_str(&manifest);
                                map.insert(
                                    format!("/{}", state.config.search.output),
                                    main_nt,
                                );
                            } else if let Ok(nt) = state.store.export_search_ntriples() {
                                map.insert(
                                    format!("/{}", state.config.search.output),
                                    nt,
                                );
                            }
                        }

                        Ok((renderer, map))
                    }
                })
                .await
                .map_err(|e| e.to_string())??;

                // Async: dispatch plugin hooks
                let reg = state_clone.registry.lock().await;
                let _ = reg
                    .dispatch_graph_updated(&state_clone.config, &state_clone.store)
                    .await;
                drop(reg);

                Ok::<_, String>(pages)
            }
            .await;

            match rebuild_result {
                Ok(new_pages) => {
                    let count = new_pages.len();
                    *rebuild_state.pages.write().await = new_pages;
                    eprintln!("Rebuilt {count} page(s)");
                }
                Err(e) => eprintln!("Rebuild error: {e}"),
            }
        }
    });

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/api/sparql", get(sparql_handler).post(sparql_handler_post))
        .nest("/api", crate::api::api_router())
        .route("/__geoff__/", get(geoff_ui_handler))
        .route("/__geoff__/theme/", get(theme_editor_handler))
        .route(
            "/__geoff__/theme/{*path}",
            get(theme_editor_component_handler),
        )
        .route(
            "/__geoff__/components/{*path}",
            get(geoff_component_handler),
        )
        .route("/__geoff__/{*rest}", get(geoff_ui_handler))
        .route("/theme/tokens.css", get(theme_tokens_css_handler))
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
            // Serve non-HTML content (e.g. search.nt) with appropriate content type
            if path.ends_with(".json")
                || path.ends_with(".wit")
                || path == "/.well-known/mcp"
                || path == "/.mcp.json"
            {
                let ct = if path.ends_with(".wit") {
                    "text/plain"
                } else {
                    "application/json"
                };
                return (
                    StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, ct)],
                    content.clone(),
                )
                    .into_response();
            }

            if path.ends_with(".nt") {
                return (
                    StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "application/n-triples")],
                    content.clone(),
                )
                    .into_response();
            }

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
        None => {
            // Try serving from static/ directory
            let static_path = state
                .site_root
                .join("static")
                .join(path.trim_start_matches('/'));
            if static_path.is_file() {
                let content_type = match static_path.extension().unwrap_or("") {
                    "css" => "text/css",
                    "js" => "application/javascript",
                    "svg" => "image/svg+xml",
                    "png" => "image/png",
                    "jpg" | "jpeg" => "image/jpeg",
                    "ico" => "image/x-icon",
                    "woff2" => "font/woff2",
                    "woff" => "font/woff",
                    "json" => "application/json",
                    "xml" => "application/xml",
                    "wasm" => "application/wasm",
                    "wit" => "text/plain",
                    _ => "application/octet-stream",
                };
                match std::fs::read(&static_path) {
                    Ok(bytes) => (
                        StatusCode::OK,
                        [(axum::http::header::CONTENT_TYPE, content_type)],
                        bytes,
                    )
                        .into_response(),
                    Err(_) => (
                        StatusCode::NOT_FOUND,
                        Html("<h1>404 Not Found</h1>".to_string()),
                    )
                        .into_response(),
                }
            } else {
                (
                    StatusCode::NOT_FOUND,
                    Html("<h1>404 Not Found</h1>".to_string()),
                )
                    .into_response()
            }
        }
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

/// Theme editor served at `/__geoff__/theme/`.
async fn theme_editor_handler() -> Html<String> {
    Html(THEME_EDITOR_HTML.to_string())
}

/// Serve theme editor web component JS files (embedded in binary).
async fn theme_editor_component_handler(
    State(state): State<Arc<DevState>>,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> impl IntoResponse {
    // First check the site's components/ directory (allows overrides)
    let file_path = state.site_root.join("components").join(&path);
    if let Ok(content) = std::fs::read_to_string(&file_path) {
        return (
            StatusCode::OK,
            [("content-type", "application/javascript")],
            content,
        )
            .into_response();
    }

    // Fall back to embedded components
    let embedded = match path.as_str() {
        "geoff-token-field.js" => Some(include_str!("../components/geoff-token-field.js")),
        "geoff-token-group.js" => Some(include_str!("../components/geoff-token-group.js")),
        "geoff-token-editor.js" => Some(include_str!("../components/geoff-token-editor.js")),
        "geoff-theme-preview.js" => Some(include_str!("../components/geoff-theme-preview.js")),
        "geoff-theme-editor-app.js" => {
            Some(include_str!("../components/geoff-theme-editor-app.js"))
        }
        "geoff-color-palette.js" => Some(include_str!("../components/geoff-color-palette.js")),
        "geoff-token-tree.js" => Some(include_str!("../components/geoff-token-tree.js")),
        "geoff-create-theme.js" => Some(include_str!("../components/geoff-create-theme.js")),
        "geoff-solid-auth.js" => Some(include_str!("../components/geoff-solid-auth.js")),
        _ => None,
    };

    match embedded {
        Some(content) => (
            StatusCode::OK,
            [("content-type", "application/javascript")],
            content.to_string(),
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            format!("Theme component not found: {path}"),
        )
            .into_response(),
    }
}

/// Serve the deferred (non-critical) theme CSS at /theme/tokens.css.
async fn theme_tokens_css_handler(State(state): State<Arc<DevState>>) -> impl IntoResponse {
    let theme_name = match state.config.theme.name.as_deref() {
        Some(n) => n,
        None => return (StatusCode::NOT_FOUND, "No theme configured").into_response(),
    };

    let theme_dir = state.site_root.join("themes").join(theme_name);
    let tokens_path = theme_dir.join("tokens.json");
    let raw_str = match std::fs::read_to_string(&tokens_path) {
        Ok(s) => s,
        Err(_) => return (StatusCode::NOT_FOUND, "Theme tokens not found").into_response(),
    };

    let merged_str = if let Some(base_name) = state.config.theme.base.as_deref() {
        let base_path = state
            .site_root
            .join("themes")
            .join(base_name)
            .join("tokens.json");
        if let Ok(base_str) = std::fs::read_to_string(&base_path)
            && let Ok(base_json) = serde_json::from_str::<serde_json::Value>(&base_str)
            && let Ok(child_json) = serde_json::from_str::<serde_json::Value>(&raw_str)
        {
            serde_json::to_string(&geoff_theme::merge_tokens(&base_json, &child_json))
                .unwrap_or(raw_str)
        } else {
            raw_str
        }
    } else {
        raw_str
    };

    match geoff_theme::DesignTokens::from_json(&merged_str) {
        Ok(tokens) => {
            let mut flat = tokens.flatten();
            geoff_theme::resolve_references(&mut flat);
            let css = geoff_theme::generate_css(&flat, None, false);
            (
                StatusCode::OK,
                [("content-type", "text/css")],
                format!(":root {{\n{css}}}"),
            )
                .into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

const THEME_EDITOR_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Geoff Theme Editor</title>
  <script type="module" src="/__geoff__/theme/geoff-token-field.js"></script>
  <script type="module" src="/__geoff__/theme/geoff-token-group.js"></script>
  <script type="module" src="/__geoff__/theme/geoff-token-editor.js"></script>
  <script type="module" src="/__geoff__/theme/geoff-theme-preview.js"></script>
  <script type="module" src="/__geoff__/theme/geoff-color-palette.js"></script>
  <script type="module" src="/__geoff__/theme/geoff-token-tree.js"></script>
  <script type="module" src="/__geoff__/theme/geoff-create-theme.js"></script>
  <script type="module" src="/__geoff__/theme/geoff-solid-auth.js"></script>
  <script type="module" src="/__geoff__/theme/geoff-theme-editor-app.js"></script>
</head>
<body style="margin:0;height:100vh;overflow:hidden">
  <geoff-theme-editor-app></geoff-theme-editor-app>
</body>
</html>"##;

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

/// Build pages using the three-phase pipeline (ingest → hooks → render)
/// with full async plugin hook dispatch.
async fn build_with_hooks_async(
    site_root: &camino::Utf8Path,
    config: &SiteConfig,
    store: &ContentStore,
    renderer: &SiteRenderer,
    registry: &Arc<tokio::sync::Mutex<geoff_plugin::registry::PluginRegistry>>,
) -> std::result::Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    use geoff_core::types::normalize_path;

    // Phase 1: Ingest content into the graph
    let (mut to_render, stats, page_index) = ingest_content(site_root, config, store, None)?;
    renderer.set_page_index(page_index);

    // Hook: on_graph_updated — plugins can read content triples and inject new ones
    {
        let reg = registry.lock().await;
        if let Err(e) = reg.dispatch_graph_updated(config, store).await {
            eprintln!("Plugin on_graph_updated warning: {e}");
        }
    }

    // Hook: on_page_render — plugins can inject extra template variables per page
    {
        let reg = registry.lock().await;
        for page in &mut to_render {
            let mut page_data = geoff_plugin::context::PageData {
                path: page.output_path.clone(),
                title: Some(page.title.clone()),
                content_type: None,
                html: page.content_html.clone(),
                raw_body: String::new(),
                frontmatter: std::collections::HashMap::new(),
            };
            let mut extra_vars = page.extra_vars.clone();
            if let Err(e) = reg
                .dispatch_page_render(config, store, &mut page_data, &mut extra_vars)
                .await
            {
                eprintln!(
                    "Plugin on_page_render warning for {}: {e}",
                    page.output_path
                );
            }
            page.extra_vars = extra_vars;
        }
    }

    // Phase 2: Render pages (SPARQL queries + extra_vars both available)
    let pages = render_pages(&to_render, config, renderer, stats.skipped)?;

    let mut map = HashMap::new();
    for page in pages {
        let normalized = normalize_path(&page.output_path);
        let url_path = if normalized == "index.html" {
            "/".to_string()
        } else {
            format!("/{normalized}")
        };
        map.insert(url_path, page.html);
    }

    // Generate search index so it's available during dev
    if config.search.enabled {
        if let Some(ref strategy) = config.search.partition {
            if let Ok(partitions) =
                store.export_partitioned_ntriples(&|graph: &str| match strategy.as_str() {
                    "section" => {
                        let path = graph
                            .strip_prefix("<urn:geoff:content:")?
                            .strip_suffix('>')?;
                        let first_slash = path.find('/')?;
                        Some(path[..first_slash].to_string())
                    }
                    _ => {
                        let path = graph
                            .strip_prefix("<urn:geoff:content:")?
                            .strip_suffix('>')?;
                        let first_slash = path.find('/')?;
                        Some(path[..first_slash].to_string())
                    }
                })
            {
                let mut manifest = String::new();
                for (key, nt) in &partitions {
                    if key.is_empty() {
                        continue;
                    }
                    map.insert(format!("/search/{key}.nt"), nt.clone());
                    use std::fmt::Write;
                    let _ = writeln!(
                        manifest,
                        "<{}/search/{key}.nt> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <urn:geoff:SearchGraph> .",
                        config.base_url.trim_end_matches('/')
                    );
                    let _ = writeln!(
                        manifest,
                        "<{}/search/{key}.nt> <urn:geoff:graphName> \"{key}\" .",
                        config.base_url.trim_end_matches('/')
                    );
                }
                let mut main_nt = partitions.get("").cloned().unwrap_or_default();
                main_nt.push_str(&manifest);
                map.insert(format!("/{}", config.search.output), main_nt);
            }
        } else if let Ok(nt) = store.export_search_ntriples() {
            map.insert(format!("/{}", config.search.output), nt);
        }
    }

    // Generate search page if enabled
    if config.search.enabled && config.search.page {
        let search_html = generate_dev_search_page(config);
        map.insert("/search/".to_string(), search_html);
        map.insert(
            "/geoff-faceted-search.js".to_string(),
            include_str!("../components/geoff-faceted-search.js").to_string(),
        );
    }

    // Generate MCP manifest for agent discovery
    if config.mcp.enabled {
        let manifest = generate_mcp_manifest_json(config);
        map.insert("/.well-known/mcp.json".to_string(), manifest.clone());
        map.insert("/.well-known/mcp".to_string(), manifest.clone());
        map.insert("/.mcp.json".to_string(), manifest);
        map.insert(
            "/bin/geoff-sparql.wit".to_string(),
            include_str!("../components/geoff-sparql.wit").to_string(),
        );
    }

    Ok(map)
}

fn generate_mcp_manifest_json(config: &SiteConfig) -> String {
    let base = config.base_url.trim_end_matches('/');
    let wasm_url = config
        .mcp
        .wasm_url
        .as_deref()
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{base}/bin/geoff-sparql.wasm"));
    let description =
        config.mcp.description.as_deref().unwrap_or(
            "Execute a SPARQL SELECT or ASK query against the site's RDF knowledge graph",
        );
    let dataset_url = format!("{base}/{}", config.search.output);
    let site_name = config
        .base_url
        .replace("https://", "")
        .replace("http://", "")
        .replace('/', "-")
        .trim_end_matches('-')
        .to_string();

    let manifest = serde_json::json!({
        "mcp_version": "1.0",
        "name": format!("{site_name}-knowledge"),
        "description": format!("Structured knowledge base for {}", config.title),
        "tools": [{
            "name": "sparql_query",
            "description": description,
            "runtime": "wasm-wasi",
            "binary_url": wasm_url,
            "wit_url": format!("{base}/bin/geoff-sparql.wit"),
            "input_schema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "SPARQL SELECT or ASK query" },
                    "dataset_url": { "type": "string", "description": "URL of the N-Triples dataset" }
                },
                "required": ["query"]
            },
            "fixed_arguments": { "dataset_url": dataset_url },
            "datasets": []
        }]
    });
    serde_json::to_string_pretty(&manifest).unwrap_or_default()
}

fn generate_dev_search_page(config: &SiteConfig) -> String {
    let title = config.search.title.as_deref().unwrap_or("Search");
    let site_title = &config.title;
    let placeholder = &config.search.placeholder;
    let limit = config.search.limit;
    let index = &config.search.output;

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{title} | {site_title}</title>
  <script type="module" src="/geoff-faceted-search.js"></script>
</head>
<body>
  <main>
    <geoff-faceted-search
      index="/{index}"
      limit="{limit}"
      placeholder="{placeholder}">
    </geoff-faceted-search>
  </main>
</body>
</html>"#
    )
}

#[derive(serde::Deserialize)]
struct SparqlBody {
    query: String,
}

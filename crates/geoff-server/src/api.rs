//! REST API endpoints for the Geoff authoring UI.

use std::sync::Arc;

use axum::Router;
use axum::extract::Multipart;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};

use crate::server::DevState;

/// Build the API router with all authoring UI endpoints.
pub fn api_router() -> Router<Arc<DevState>> {
    Router::new()
        .route("/pages", get(list_pages))
        .route("/pages/{*path}", get(get_page).put(save_page))
        .route("/graph", get(get_graph))
        .route("/graph/{*path}", get(get_page_graph))
        .route("/vocabs", get(list_vocabs))
        .route("/vocabs/search", get(search_vocabs))
        .route("/validate", get(validate_all))
        .route("/validate/{*path}", get(validate_page))
        .route("/theme/tokens", get(get_theme_tokens).put(put_theme_tokens))
        .route("/theme/css", get(get_theme_css))
        .route("/theme/prefix", get(get_theme_prefix).put(put_theme_prefix))
        .route("/theme/preview-pages", post(upload_preview_page))
        .route("/theme/preview-pages/{id}", get(get_preview_page))
        .route("/theme/proxy", get(proxy_url_with_css))
}

// ── GET /api/pages ──────────────────────────────────────────────────

#[derive(serde::Serialize)]
struct PageMeta {
    path: String,
    title: Option<String>,
    content_type: Option<String>,
    template: Option<String>,
    date: Option<String>,
}

async fn list_pages(State(state): State<Arc<DevState>>) -> impl IntoResponse {
    let content_dir = state.site_root.join(&state.config.content_dir);

    let files = match geoff_content::scanner::scan_content_dir(&content_dir) {
        Ok(f) => f,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                json_response(&serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let mut pages = Vec::new();
    for file_path in &files {
        let raw = match std::fs::read_to_string(file_path) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let (fm_str, _body) = match geoff_content::frontmatter::split_frontmatter(&raw) {
            Ok(pair) => pair,
            Err(_) => continue,
        };
        let (frontmatter, _, _) = match geoff_content::frontmatter::parse_frontmatter(fm_str) {
            Ok(tuple) => tuple,
            Err(_) => continue,
        };

        let rel_path = file_path
            .strip_prefix(&content_dir)
            .unwrap_or(file_path)
            .to_string()
            .replace('\\', "/");

        pages.push(PageMeta {
            path: rel_path,
            title: frontmatter
                .get("title")
                .and_then(|v| v.as_str())
                .map(String::from),
            content_type: frontmatter
                .get("type")
                .and_then(|v| v.as_str())
                .map(String::from),
            template: frontmatter
                .get("template")
                .and_then(|v| v.as_str())
                .map(String::from),
            date: frontmatter.get("date").map(|v| v.to_string()),
        });
    }

    json_ok(&pages).into_response()
}

// ── GET /api/pages/:path ────────────────────────────────────────────

#[derive(serde::Serialize)]
struct PageDetail {
    path: String,
    raw_markdown: String,
    frontmatter: serde_json::Value,
    html: String,
}

async fn get_page(
    State(state): State<Arc<DevState>>,
    Path(path): Path<String>,
) -> impl IntoResponse {
    let content_dir = state.site_root.join(&state.config.content_dir);
    let file_path = content_dir.join(&path);

    if !file_path.exists() {
        return (
            StatusCode::NOT_FOUND,
            json_response(&serde_json::json!({"error": "Page not found"})),
        )
            .into_response();
    }

    let raw = match std::fs::read_to_string(&file_path) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                json_response(&serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let (fm_str, body) = match geoff_content::frontmatter::split_frontmatter(&raw) {
        Ok(pair) => pair,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                json_response(&serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let frontmatter_value: serde_json::Value =
        match toml::from_str::<toml::Value>(fm_str).map(|v| toml_to_json(&v)) {
            Ok(v) => v,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    json_response(&serde_json::json!({"error": e.to_string()})),
                )
                    .into_response();
            }
        };

    let html = geoff_content::markdown::render_markdown(body);

    json_ok(&PageDetail {
        path,
        raw_markdown: body.to_string(),
        frontmatter: frontmatter_value,
        html,
    })
    .into_response()
}

// ── PUT /api/pages/:path ────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct SavePageBody {
    frontmatter: String,
    body: String,
}

async fn save_page(
    State(state): State<Arc<DevState>>,
    Path(path): Path<String>,
    axum::Json(payload): axum::Json<SavePageBody>,
) -> impl IntoResponse {
    let content_dir = state.site_root.join(&state.config.content_dir);
    let file_path = content_dir.join(&path);

    if let Some(parent) = file_path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            json_response(&serde_json::json!({"error": e.to_string()})),
        )
            .into_response();
    }

    let content = format!(
        "+++\n{}\n+++\n\n{}\n",
        payload.frontmatter.trim(),
        payload.body
    );
    if let Err(e) = std::fs::write(&file_path, &content) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            json_response(&serde_json::json!({"error": e.to_string()})),
        )
            .into_response();
    }

    // The file watcher will trigger a rebuild automatically
    json_ok(&serde_json::json!({"saved": true, "path": path})).into_response()
}

// ── GET /api/graph ──────────────────────────────────────────────────

async fn get_graph(State(state): State<Arc<DevState>>) -> impl IntoResponse {
    let query = "SELECT ?g ?s ?p ?o WHERE { GRAPH ?g { ?s ?p ?o } } ORDER BY ?g ?s ?p LIMIT 1000";
    match state.store.query_to_json(query) {
        Ok(result) => json_ok(&result).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            json_response(&serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ── GET /api/graph/:page_path ───────────────────────────────────────

async fn get_page_graph(
    State(state): State<Arc<DevState>>,
    Path(path): Path<String>,
) -> impl IntoResponse {
    let normalized_path = path.replace('\\', "/");
    let graph_uri = format!("urn:geoff:content:{normalized_path}");
    let query =
        format!("SELECT ?s ?p ?o WHERE {{ GRAPH <{graph_uri}> {{ ?s ?p ?o }} }} ORDER BY ?s ?p");
    match state.store.query_to_json(&query) {
        Ok(result) => json_ok(&result).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            json_response(&serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ── GET /api/vocabs ─────────────────────────────────────────────────

#[derive(serde::Serialize)]
struct VocabTermJson {
    iri: String,
    label: String,
    comment: String,
    is_class: bool,
    source: String,
}

async fn list_vocabs(State(state): State<Arc<DevState>>) -> impl IntoResponse {
    let ontologies_dir = state.site_root.join("ontologies");
    let mut index = geoff_ontology::vocabulary::VocabularyIndex::new();
    if let Err(e) = index.load_directory(&ontologies_dir) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            json_response(&serde_json::json!({"error": e.to_string()})),
        )
            .into_response();
    }

    let terms: Vec<VocabTermJson> = index
        .all_terms()
        .map(|t| VocabTermJson {
            iri: t.iri.clone(),
            label: t.label.clone(),
            comment: t.comment.clone(),
            is_class: t.is_class,
            source: t.source.clone(),
        })
        .collect();

    json_ok(&terms).into_response()
}

// ── GET /api/vocabs/search?q=... ────────────────────────────────────

#[derive(serde::Deserialize)]
struct VocabSearchQuery {
    q: String,
}

#[derive(serde::Serialize)]
struct VocabSearchResult {
    iri: String,
    label: String,
    comment: String,
    is_class: bool,
    source: String,
    score: f64,
    matched_label: String,
}

async fn search_vocabs(
    State(state): State<Arc<DevState>>,
    Query(params): Query<VocabSearchQuery>,
) -> impl IntoResponse {
    let ontologies_dir = state.site_root.join("ontologies");
    let mut index = geoff_ontology::vocabulary::VocabularyIndex::new();
    if let Err(e) = index.load_directory(&ontologies_dir) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            json_response(&serde_json::json!({"error": e.to_string()})),
        )
            .into_response();
    }

    let matcher = geoff_ontology::fuzzy::FuzzyMatcher::new()
        .with_threshold(0.5)
        .with_max_results(20);
    let matches = matcher.find_matches(&params.q, &index);

    let results: Vec<VocabSearchResult> = matches
        .iter()
        .map(|m| VocabSearchResult {
            iri: m.term.iri.clone(),
            label: m.term.label.clone(),
            comment: m.term.comment.clone(),
            is_class: m.term.is_class,
            source: m.term.source.clone(),
            score: m.score,
            matched_label: m.matched_label.clone(),
        })
        .collect();

    json_ok(&results).into_response()
}

// ── GET /api/validate ───────────────────────────────────────────────

async fn validate_all(State(state): State<Arc<DevState>>) -> impl IntoResponse {
    let data_ttl = match state.store.export_turtle() {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                json_response(&serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let shapes_dir = state.site_root.join("shapes");
    let shapes_ttl = match load_shapes(&shapes_dir) {
        Ok(s) => s,
        Err(e) => {
            return json_ok(&serde_json::json!({
                "conforms": true,
                "message": format!("No shapes to validate against: {e}"),
                "violations": 0,
                "warnings": 0,
                "report": ""
            }))
            .into_response();
        }
    };

    match geoff_ontology::validation::validate_shacl(&data_ttl, &shapes_ttl) {
        Ok(outcome) => json_ok(&serde_json::json!({
            "conforms": outcome.conforms,
            "violations": outcome.violations,
            "warnings": outcome.warnings,
            "report": outcome.report_text
        }))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            json_response(&serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ── GET /api/validate/:page_path ────────────────────────────────────

async fn validate_page(
    State(state): State<Arc<DevState>>,
    Path(path): Path<String>,
) -> impl IntoResponse {
    // Export only the page's named graph via SELECT, format as NTriples
    let normalized_path = path.replace('\\', "/");
    let graph_uri = format!("urn:geoff:content:{normalized_path}");
    let select_query = format!("SELECT ?s ?p ?o WHERE {{ GRAPH <{graph_uri}> {{ ?s ?p ?o }} }}");
    let triples = match state.store.query_to_json(&select_query) {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                json_response(&serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    // Build NTriples string from query results
    let empty = vec![];
    let rows = triples.as_array().unwrap_or(&empty);
    let mut data_ttl = String::new();
    for row in rows {
        let s = row["s"].as_str().unwrap_or("");
        let p = row["p"].as_str().unwrap_or("");
        let o = row["o"].as_str().unwrap_or("");
        // Detect if object is an IRI (starts with <) or literal
        if o.starts_with('<') {
            data_ttl.push_str(&format!("{s} {p} {o} .\n"));
        } else {
            data_ttl.push_str(&format!("{s} {p} \"{o}\" .\n"));
        }
    }

    let shapes_dir = state.site_root.join("shapes");
    let shapes_ttl = match load_shapes(&shapes_dir) {
        Ok(s) => s,
        Err(e) => {
            return json_ok(&serde_json::json!({
                "conforms": true,
                "message": format!("No shapes to validate against: {e}"),
                "violations": 0,
                "warnings": 0,
                "report": ""
            }))
            .into_response();
        }
    };

    match geoff_ontology::validation::validate_shacl(&data_ttl, &shapes_ttl) {
        Ok(outcome) => json_ok(&serde_json::json!({
            "conforms": outcome.conforms,
            "violations": outcome.violations,
            "warnings": outcome.warnings,
            "report": outcome.report_text,
            "page": path
        }))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            json_response(&serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ── GET /api/theme/tokens ──────────────────────────────────────────

async fn get_theme_tokens(State(state): State<Arc<DevState>>) -> impl IntoResponse {
    let theme_name = match state.config.theme.name.as_deref() {
        Some(name) => name,
        None => {
            return json_ok(&serde_json::json!({
                "tokens": {},
                "resolved": {},
                "error": "No theme configured"
            }))
            .into_response();
        }
    };

    let theme_dir = state.site_root.join("themes").join(theme_name);
    let tokens_path = theme_dir.join("tokens.json");

    if !tokens_path.exists() {
        return json_ok(&serde_json::json!({
            "tokens": {},
            "resolved": {},
            "error": format!("Theme file not found: {}", tokens_path)
        }))
        .into_response();
    }

    let raw_json: serde_json::Value = match std::fs::read_to_string(&tokens_path)
        .map_err(|e| e.to_string())
        .and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
    {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                json_response(&serde_json::json!({"error": e})),
            )
                .into_response();
        }
    };

    // Unwrap if the token file has a single "tokens" wrapper key (Style Dictionary output)
    let raw_json = unwrap_tokens_key(raw_json);

    // Merge with base theme if configured
    let merged = if let Some(base_name) = state.config.theme.base.as_deref() {
        let base_path = state
            .site_root
            .join("themes")
            .join(base_name)
            .join("tokens.json");
        if let Ok(base_str) = std::fs::read_to_string(&base_path) {
            if let Ok(base_json) = serde_json::from_str::<serde_json::Value>(&base_str) {
                geoff_theme::merge_tokens(&base_json, &raw_json)
            } else {
                raw_json.clone()
            }
        } else {
            raw_json.clone()
        }
    } else {
        raw_json.clone()
    };

    // Parse and flatten for resolved view
    let resolved = match geoff_theme::DesignTokens::from_json(
        &serde_json::to_string(&merged).unwrap_or_default(),
    ) {
        Ok(tokens) => {
            let mut flat = tokens.flatten();
            geoff_theme::resolve_references(&mut flat);
            let resolved_map: serde_json::Map<String, serde_json::Value> = flat
                .iter()
                .map(|(path, token)| {
                    let val = match &token.value {
                        geoff_theme::TokenValue::String(s) => serde_json::Value::String(s.clone()),
                        geoff_theme::TokenValue::Number(n) => {
                            serde_json::json!(n)
                        }
                        other => serde_json::to_value(other).unwrap_or_default(),
                    };
                    (
                        path.clone(),
                        serde_json::json!({
                            "value": val,
                            "type": token.token_type,
                            "description": token.description,
                            "cssVar": format!("--{}", path.replace('.', "-"))
                        }),
                    )
                })
                .collect();
            serde_json::Value::Object(resolved_map)
        }
        Err(_) => serde_json::json!({}),
    };

    json_ok(&serde_json::json!({
        "tokens": merged,
        "resolved": resolved,
        "prefix": state.config.theme.prefix.as_deref().unwrap_or("")
    }))
    .into_response()
}

// ── PUT /api/theme/tokens ─────────────────────────────────────────

async fn put_theme_tokens(
    State(state): State<Arc<DevState>>,
    axum::Json(payload): axum::Json<serde_json::Value>,
) -> impl IntoResponse {
    let theme_name = match state.config.theme.name.as_deref() {
        Some(name) => name,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                json_response(&serde_json::json!({"error": "No theme configured"})),
            )
                .into_response();
        }
    };

    let theme_dir = state.site_root.join("themes").join(theme_name);
    let tokens_path = theme_dir.join("tokens.json");

    // Unwrap the { "tokens": ... } wrapper the editor sends
    let payload = unwrap_tokens_key(payload);

    if let Err(e) = std::fs::create_dir_all(&theme_dir) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            json_response(&serde_json::json!({"error": e.to_string()})),
        )
            .into_response();
    }

    let pretty = serde_json::to_string_pretty(&payload).unwrap_or_default();
    if let Err(e) = std::fs::write(&tokens_path, &pretty) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            json_response(&serde_json::json!({"error": e.to_string()})),
        )
            .into_response();
    }

    // Generate CSS from updated tokens
    let css = match geoff_theme::DesignTokens::from_json(&pretty) {
        Ok(tokens) => {
            let mut flat = tokens.flatten();
            geoff_theme::resolve_references(&mut flat);
            geoff_theme::generate_css(&flat, None, false)
        }
        Err(e) => {
            return json_ok(&serde_json::json!({
                "valid": false,
                "errors": [e.to_string()],
                "css": ""
            }))
            .into_response();
        }
    };

    json_ok(&serde_json::json!({
        "valid": true,
        "errors": [],
        "css": css
    }))
    .into_response()
}

// ── GET /api/theme/css ────────────────────────────────────────────

async fn get_theme_css(State(state): State<Arc<DevState>>) -> impl IntoResponse {
    let theme_name = match state.config.theme.name.as_deref() {
        Some(name) => name,
        None => {
            return (StatusCode::NOT_FOUND, "No theme configured".to_string()).into_response();
        }
    };

    let theme_dir = state.site_root.join("themes").join(theme_name);
    let tokens_path = theme_dir.join("tokens.json");

    let raw_str = match std::fs::read_to_string(&tokens_path) {
        Ok(s) => s,
        Err(_) => {
            return (StatusCode::NOT_FOUND, "Theme tokens not found".to_string()).into_response();
        }
    };

    // Merge with base if configured
    let merged_str = if let Some(base_name) = state.config.theme.base.as_deref() {
        let base_path = state
            .site_root
            .join("themes")
            .join(base_name)
            .join("tokens.json");
        if let Ok(base_str) = std::fs::read_to_string(&base_path) {
            if let Ok(base_json) = serde_json::from_str::<serde_json::Value>(&base_str) {
                if let Ok(child_json) = serde_json::from_str::<serde_json::Value>(&raw_str) {
                    serde_json::to_string(&geoff_theme::merge_tokens(&base_json, &child_json))
                        .unwrap_or(raw_str)
                } else {
                    raw_str
                }
            } else {
                raw_str
            }
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

// ── GET /api/theme/proxy?url=... ──────────────────────────────────

#[derive(serde::Deserialize)]
struct ProxyQuery {
    url: String,
}

async fn proxy_url_with_css(
    State(state): State<Arc<DevState>>,
    Query(params): Query<ProxyQuery>,
) -> impl IntoResponse {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build();
    let client = match client {
        Ok(c) => c,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    };

    let resp = match client.get(&params.url).send().await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("Failed to fetch {}: {e}", params.url),
            )
                .into_response();
        }
    };

    let html = match resp.text().await {
        Ok(h) => h,
        Err(e) => {
            return (StatusCode::BAD_GATEWAY, e.to_string()).into_response();
        }
    };

    let css = get_current_theme_css(&state);
    let injected = inject_css_into_html(&html, &css);

    (
        StatusCode::OK,
        [("content-type", "text/html; charset=utf-8")],
        injected,
    )
        .into_response()
}

// ── GET /api/theme/prefix ─────────────────────────────────────────

async fn get_theme_prefix(State(state): State<Arc<DevState>>) -> impl IntoResponse {
    json_ok(&serde_json::json!({
        "prefix": state.config.theme.prefix.as_deref().unwrap_or("")
    }))
    .into_response()
}

// ── PUT /api/theme/prefix ────────────────────────────────────────

async fn put_theme_prefix(
    State(state): State<Arc<DevState>>,
    axum::Json(payload): axum::Json<serde_json::Value>,
) -> impl IntoResponse {
    let prefix = payload.get("prefix").and_then(|v| v.as_str()).unwrap_or("");

    // Update geoff.toml with the new prefix
    let config_path = state.site_root.join("geoff.toml");
    if let Ok(mut content) = std::fs::read_to_string(&config_path) {
        if content.contains("prefix =") {
            // Replace existing prefix line
            let re_line = content
                .lines()
                .map(|line| {
                    if line.trim().starts_with("prefix") && line.contains('=') {
                        if prefix.is_empty() {
                            String::new()
                        } else {
                            format!("prefix = \"{prefix}\"")
                        }
                    } else {
                        line.to_string()
                    }
                })
                .filter(|l| !l.is_empty() || !prefix.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            content = re_line;
        } else if !prefix.is_empty() {
            // Add prefix under [theme] section
            content = content.replace("[theme]", &format!("[theme]\nprefix = \"{prefix}\""));
        }
        let _ = std::fs::write(&config_path, &content);
    }

    json_ok(&serde_json::json!({
        "prefix": prefix,
        "saved": true
    }))
    .into_response()
}

/// Unwrap a token file that has a single "tokens" wrapper key.
/// Style Dictionary and some tools output `{ "tokens": { ...actual tokens... } }`.
/// This detects and removes the wrapper so the editor sees the actual token groups.
fn unwrap_tokens_key(value: serde_json::Value) -> serde_json::Value {
    if let serde_json::Value::Object(ref obj) = value {
        // If there's a "tokens" key and it's an object, and most other keys start with $,
        // unwrap it
        if let Some(inner) = obj.get("tokens")
            && inner.is_object()
        {
            let non_meta_keys: Vec<_> = obj
                .keys()
                .filter(|k| !k.starts_with('$') && *k != "tokens")
                .collect();
            if non_meta_keys.is_empty() {
                return inner.clone();
            }
        }
    }
    value
}

// ── POST /api/theme/preview-pages ──────────────────────────────────

async fn upload_preview_page(
    State(state): State<Arc<DevState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut html_content = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            match field.bytes().await {
                Ok(bytes) => {
                    html_content = Some(String::from_utf8_lossy(&bytes).into_owned());
                }
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        json_response(&serde_json::json!({"error": e.to_string()})),
                    )
                        .into_response();
                }
            }
        }
    }

    let raw_html = match html_content {
        Some(h) => h,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                json_response(&serde_json::json!({"error": "No HTML file provided"})),
            )
                .into_response();
        }
    };

    // Generate a timestamp-based ID
    let id = format!(
        "preview-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );

    state
        .preview_pages
        .write()
        .await
        .insert(id.clone(), raw_html);

    json_ok(&serde_json::json!({
        "id": id,
        "url": format!("/api/theme/preview-pages/{id}")
    }))
    .into_response()
}

// ── GET /api/theme/preview-pages/:id ──────────────────────────────

async fn get_preview_page(
    State(state): State<Arc<DevState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let pages = state.preview_pages.read().await;
    let raw_html = match pages.get(&id) {
        Some(h) => h.clone(),
        None => {
            return (StatusCode::NOT_FOUND, "Preview page not found").into_response();
        }
    };
    drop(pages);

    // Inject the latest theme CSS into the HTML
    let css = get_current_theme_css(&state);
    let injected = inject_css_into_html(&raw_html, &css);

    (StatusCode::OK, [("content-type", "text/html")], injected).into_response()
}

/// Get the current theme CSS from the configured theme tokens.
fn get_current_theme_css(state: &DevState) -> String {
    let theme_name = match state.config.theme.name.as_deref() {
        Some(name) => name,
        None => return String::new(),
    };

    let theme_dir = state.site_root.join("themes").join(theme_name);
    let tokens_path = theme_dir.join("tokens.json");

    let raw_str = match std::fs::read_to_string(&tokens_path) {
        Ok(s) => s,
        Err(_) => return String::new(),
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
            format!(":root {{\n{css}}}")
        }
        Err(_) => String::new(),
    }
}

/// Inject a `<style>` block into HTML, preferring insertion into `<head>`.
fn inject_css_into_html(html: &str, css: &str) -> String {
    if css.is_empty() {
        return html.to_string();
    }
    let style_tag = format!("<style data-geoff-theme>{css}</style>");
    if let Some(pos) = html.find("</head>") {
        format!("{}{style_tag}\n{}", &html[..pos], &html[pos..])
    } else if let Some(pos) = html.find("<body") {
        format!("{}{style_tag}\n{}", &html[..pos], &html[pos..])
    } else {
        format!("{style_tag}\n{html}")
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

type JsonBody = ([(&'static str, &'static str); 1], String);

fn json_response(value: &serde_json::Value) -> JsonBody {
    (
        [("content-type", "application/json")],
        serde_json::to_string(value).unwrap_or_default(),
    )
}

fn json_ok<T: serde::Serialize>(value: &T) -> (StatusCode, JsonBody) {
    (
        StatusCode::OK,
        (
            [("content-type", "application/json")],
            serde_json::to_string(value).unwrap_or_default(),
        ),
    )
}

fn load_shapes(
    shapes_dir: &camino::Utf8Path,
) -> std::result::Result<String, Box<dyn std::error::Error>> {
    if !shapes_dir.exists() {
        return Err("No shapes/ directory found".into());
    }
    let mut combined = String::new();
    for entry in std::fs::read_dir(shapes_dir)? {
        let entry = entry?;
        let p = entry.path();
        if p.extension().is_some_and(|e| e == "ttl") {
            combined.push_str(&std::fs::read_to_string(&p)?);
            combined.push('\n');
        }
    }
    if combined.is_empty() {
        return Err("No .ttl shapes files found in shapes/ directory".into());
    }
    Ok(combined)
}

/// Convert a TOML value to a JSON value for API responses.
fn toml_to_json(toml: &toml::Value) -> serde_json::Value {
    match toml {
        toml::Value::String(s) => serde_json::Value::String(s.clone()),
        toml::Value::Integer(i) => serde_json::Value::Number((*i).into()),
        toml::Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        toml::Value::Boolean(b) => serde_json::Value::Bool(*b),
        toml::Value::Datetime(dt) => serde_json::Value::String(dt.to_string()),
        toml::Value::Array(arr) => serde_json::Value::Array(arr.iter().map(toml_to_json).collect()),
        toml::Value::Table(table) => {
            let map: serde_json::Map<String, serde_json::Value> = table
                .iter()
                .map(|(k, v)| (k.clone(), toml_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
    }
}

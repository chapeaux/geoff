//! REST API endpoints for the Geoff authoring UI.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;

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
        let (frontmatter, _) = match geoff_content::frontmatter::parse_frontmatter(fm_str) {
            Ok(pair) => pair,
            Err(_) => continue,
        };

        let rel_path = file_path
            .strip_prefix(&content_dir)
            .unwrap_or(file_path)
            .to_string();

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
    let graph_uri = format!("urn:geoff:content:{path}");
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
    let graph_uri = format!("urn:geoff:content:{path}");
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

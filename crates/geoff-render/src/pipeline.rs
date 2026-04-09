use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use camino::Utf8Path;
use geoff_content::frontmatter::{parse_frontmatter, split_frontmatter};
use geoff_content::markdown::render_markdown;
use geoff_content::scanner::{scan_content_dir, scan_data_dir, sidecar_ttl_path};
use geoff_core::cache::{BuildCache, hash_file};
use geoff_core::config::SiteConfig;
use geoff_core::types::{ObjectValue, PageUri, normalize_path, xsd};
use geoff_graph::store::ContentStore;
use geoff_ontology::mappings::MappingRegistry;
use serde_json::Value as JsonValue;
use rayon::prelude::*;

use crate::jsonld::build_jsonld;
use crate::renderer::{PageContext, SiteRenderer, build_page_context};

/// Result of building a single page.
pub struct BuiltPage {
    /// The output path relative to the output directory (e.g. "blog/first-post.html").
    pub output_path: String,
    /// The rendered HTML content.
    pub html: String,
}

/// Build statistics returned from the pipeline.
#[derive(Debug, Default)]
pub struct BuildStats {
    pub built: usize,
    pub skipped: usize,
    pub total: usize,
}

/// Run the full build pipeline: scan content, parse, build graph, render.
/// If `cache` is Some, performs an incremental build (skipping unchanged files).
/// Returns built pages and updated cache.
pub fn build_site(
    site_root: &Utf8Path,
    config: &SiteConfig,
    store: &ContentStore,
    renderer: &SiteRenderer,
) -> std::result::Result<Vec<BuiltPage>, Box<dyn std::error::Error>> {
    let (pages, _stats) = build_site_incremental(site_root, config, store, renderer, None)?;
    Ok(pages)
}

/// Intermediate parsed page data, ready for parallel rendering.
struct ParsedPage {
    output_path: String,
    content_html: String,
    json_ld_str: String,
    template: String,
    title: String,
    date: Option<String>,
    author: Option<String>,
    description: Option<String>,
    tags: Option<Vec<String>>,
}

/// Run the build pipeline with optional incremental support.
/// When `cache` is provided, only changed files are rebuilt.
/// Pages are rendered in parallel using rayon.
pub fn build_site_incremental(
    site_root: &Utf8Path,
    config: &SiteConfig,
    store: &ContentStore,
    renderer: &SiteRenderer,
    cache: Option<&BuildCache>,
) -> std::result::Result<(Vec<BuiltPage>, BuildStats), Box<dyn std::error::Error>> {
    let content_dir = site_root.join(&config.content_dir);

    // Load mapping registry from ontology/mappings.toml
    let mappings_path = site_root.join("ontology/mappings.toml");
    let registry = MappingRegistry::load(&mappings_path)?;

    // Load pure RDF data files from content/data/ directory
    let data_dir = content_dir.join("data");
    for ttl_file in scan_data_dir(&data_dir)? {
        store.load_turtle(&ttl_file)?;
    }

    let files = scan_content_dir(&content_dir)?;
    let mut stats = BuildStats {
        total: files.len(),
        ..Default::default()
    };

    // Check if templates changed — if so, rebuild everything
    let templates_changed = if let Some(c) = cache {
        let template_dir = site_root.join(&config.template_dir);
        let current_hash = geoff_core::cache::hash_directory(&template_dir)?;
        c.template_hash.as_deref() != Some(current_hash.as_str())
    } else {
        true
    };

    // Phase 1: Sequential parse + graph ingestion
    let mut to_render: Vec<ParsedPage> = Vec::new();

    for file_path in &files {
        // Check cache for incremental builds
        if !templates_changed
            && let Some(c) = cache
            && let Ok(current_hash) = hash_file(file_path)
            && !c.is_changed(
                &normalize_path(
                    file_path
                        .strip_prefix(&content_dir)
                        .unwrap_or(file_path)
                        .as_str(),
                ),
                &current_hash,
            )
        {
            // Still need to ingest triples for unchanged files
            // so SPARQL queries see all content
            ingest_triples_only(file_path, &content_dir, store, &registry)?;
            stats.skipped += 1;
            continue;
        }

        if let Some(parsed) = parse_and_ingest(file_path, &content_dir, config, store, &registry)? {
            to_render.push(parsed);
        }
    }

    // Phase 2: Parallel rendering
    let render_count = AtomicUsize::new(0);
    let total_to_render = to_render.len();

    let results: Vec<std::result::Result<BuiltPage, String>> = to_render
        .par_iter()
        .map(|parsed| {
            let ctx = build_page_context(&PageContext {
                title: &parsed.title,
                content_html: &parsed.content_html,
                json_ld: &parsed.json_ld_str,
                site_title: &config.title,
                date: parsed.date.as_deref(),
                author: parsed.author.as_deref(),
                description: parsed.description.as_deref(),
                tags: parsed.tags.as_deref(),
            });

            let rendered = renderer
                .render_with_context(&parsed.template, &ctx)
                .map_err(|e| format!("{}: {e}", parsed.output_path))?;

            let done = render_count.fetch_add(1, Ordering::Relaxed) + 1;
            if total_to_render > 1 {
                eprint!("\rRendered {done}/{total_to_render} pages");
            }

            Ok(BuiltPage {
                output_path: parsed.output_path.clone(),
                html: rendered,
            })
        })
        .collect();

    if total_to_render > 1 {
        eprintln!(); // newline after progress
    }

    let mut pages = Vec::with_capacity(results.len());
    for result in results {
        match result {
            Ok(page) => {
                stats.built += 1;
                pages.push(page);
            }
            Err(e) => return Err(e.into()),
        }
    }

    Ok((pages, stats))
}

/// Parse a content file, ingest its triples, and return data ready for rendering.
fn parse_and_ingest(
    file_path: &Utf8Path,
    content_dir: &Utf8Path,
    config: &SiteConfig,
    store: &ContentStore,
    registry: &MappingRegistry,
) -> std::result::Result<Option<ParsedPage>, Box<dyn std::error::Error>> {
    let raw = std::fs::read_to_string(file_path)?;

    let (fm_str, body) = match split_frontmatter(&raw) {
        Ok(pair) => pair,
        Err(_) => return Ok(None),
    };

    let (frontmatter, rdf_fields) = parse_frontmatter(fm_str)?;
    let html = render_markdown(body);

    let title = frontmatter
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Untitled")
        .to_string();
    let date = frontmatter.get("date").map(toml_value_to_string);
    let author = frontmatter
        .get("author")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let content_type = frontmatter
        .get("type")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let description = frontmatter
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let template = frontmatter
        .get("template")
        .and_then(|v| v.as_str())
        .unwrap_or("page.html")
        .to_string();

    let tags: Option<Vec<String>> = frontmatter.get("tags").and_then(|v| {
        v.as_array().map(|arr| {
            arr.iter()
                .filter_map(|t| t.as_str().map(|s| s.to_string()))
                .collect()
        })
    });

    let rel_path = file_path.strip_prefix(content_dir).unwrap_or(file_path);
    let output_name = normalize_path(rel_path.with_extension("html").as_ref());
    let page_uri = PageUri::from_path(rel_path.as_str());
    let graph_name = page_uri.as_str();

    // Compute page URL path from output name
    let page_url = output_path_to_url(&output_name);

    // Insert triples into the graph (sequential)
    insert_page_triples(&PageTriples {
        store,
        page_uri: &page_uri,
        graph_name,
        title: Some(&title),
        date: date.as_deref(),
        author: author.as_deref(),
        description: description.as_deref(),
        content_type: content_type.as_deref(),
        url: Some(&page_url),
        registry,
    })?;

    // Insert [rdf.custom] fields as triples
    insert_custom_triples(store, &page_uri, graph_name, &rdf_fields)?;

    if let Some(sidecar_path) = sidecar_ttl_path(file_path) {
        store.load_turtle_into(&sidecar_path, graph_name)?;
    }

    // Build JSON-LD
    let page_output_path = normalize_path(rel_path.with_extension("").as_ref());
    let jsonld = build_jsonld(
        &config.base_url,
        &page_output_path,
        Some(&title),
        date.as_deref(),
        author.as_deref(),
        content_type.as_deref(),
    );
    let json_ld_str = serde_json::to_string_pretty(&jsonld)?;

    Ok(Some(ParsedPage {
        output_path: output_name,
        content_html: html,
        json_ld_str,
        template,
        title,
        date,
        author,
        description,
        tags,
    }))
}

/// Ingest triples for a file without rendering it (for incremental builds).
fn ingest_triples_only(
    file_path: &Utf8Path,
    content_dir: &Utf8Path,
    store: &ContentStore,
    registry: &MappingRegistry,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let raw = std::fs::read_to_string(file_path)?;
    let (fm_str, _body) = match split_frontmatter(&raw) {
        Ok(pair) => pair,
        Err(_) => return Ok(()),
    };
    let (frontmatter, rdf_fields) = parse_frontmatter(fm_str)?;

    let rel_path = file_path.strip_prefix(content_dir).unwrap_or(file_path);
    let output_name = normalize_path(rel_path.with_extension("html").as_ref());
    let page_uri = PageUri::from_path(rel_path.as_str());
    let graph_name = page_uri.as_str();
    let date_str = frontmatter.get("date").map(toml_value_to_string);
    let page_url = output_path_to_url(&output_name);

    insert_page_triples(&PageTriples {
        store,
        page_uri: &page_uri,
        graph_name,
        title: frontmatter.get("title").and_then(|v| v.as_str()),
        date: date_str.as_deref(),
        author: frontmatter.get("author").and_then(|v| v.as_str()),
        description: frontmatter
            .get("description")
            .and_then(|v| v.as_str()),
        content_type: frontmatter.get("type").and_then(|v| v.as_str()),
        url: Some(&page_url),
        registry,
    })?;

    // Insert [rdf.custom] fields as triples
    insert_custom_triples(store, &page_uri, graph_name, &rdf_fields)?;

    if let Some(sidecar_path) = sidecar_ttl_path(file_path) {
        store.load_turtle_into(&sidecar_path, graph_name)?;
    }

    Ok(())
}

/// Default type mappings used when the mapping registry has no entry.
fn default_type_iri(content_type: &str) -> &str {
    match content_type {
        "Blog Post" | "BlogPosting" => "http://schema.org/BlogPosting",
        "Article" => "http://schema.org/Article",
        "How-To Guide" | "HowTo" => "http://schema.org/HowTo",
        "FAQ Page" | "FAQPage" => "http://schema.org/FAQPage",
        "Event" => "http://schema.org/Event",
        "Web Page" | "WebPage" => "http://schema.org/WebPage",
        _ => "http://schema.org/WebPage",
    }
}

struct PageTriples<'a> {
    store: &'a ContentStore,
    page_uri: &'a PageUri,
    graph_name: &'a str,
    title: Option<&'a str>,
    date: Option<&'a str>,
    author: Option<&'a str>,
    description: Option<&'a str>,
    content_type: Option<&'a str>,
    url: Option<&'a str>,
    registry: &'a MappingRegistry,
}

/// Convert a TOML value to a clean string, handling Datetime specially
/// to avoid the `{ "$__toml_private_datetime" = "..." }` output.
fn toml_value_to_string(v: &toml::Value) -> String {
    match v {
        toml::Value::Datetime(dt) => dt.to_string(),
        other => other.to_string(),
    }
}

/// Convert an output file path to a URL path.
/// e.g. "blog/2026-03-30-welcome.html" → "/blog/2026-03-30-welcome.html"
///      "blog/index.html" → "/blog/"
///      "index.html" → "/"
fn output_path_to_url(output_path: &str) -> String {
    if output_path == "index.html" {
        "/".to_string()
    } else if let Some(dir) = output_path.strip_suffix("/index.html") {
        format!("/{dir}/")
    } else {
        format!("/{output_path}")
    }
}

fn insert_page_triples(p: &PageTriples<'_>) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let store = p.store;
    let page_uri = p.page_uri;
    let graph_name = p.graph_name;
    if let Some(t) = p.title {
        store.insert_triple_into(
            page_uri.as_str(),
            "http://schema.org/name",
            &ObjectValue::Literal(t.to_string()),
            graph_name,
        )?;
    }
    if let Some(d) = p.date {
        // Use xsd:date for date-only values, xsd:dateTime for datetime values
        let datatype = if d.contains('T') {
            xsd::DATE_TIME
        } else {
            xsd::DATE
        };
        store.insert_triple_into(
            page_uri.as_str(),
            "http://schema.org/datePublished",
            &ObjectValue::TypedLiteral {
                value: d.to_string(),
                datatype: datatype.to_string(),
            },
            graph_name,
        )?;
    }
    if let Some(a) = p.author {
        store.insert_triple_into(
            page_uri.as_str(),
            "http://schema.org/author",
            &ObjectValue::Literal(a.to_string()),
            graph_name,
        )?;
    }
    if let Some(desc) = p.description {
        store.insert_triple_into(
            page_uri.as_str(),
            "http://schema.org/description",
            &ObjectValue::Literal(desc.to_string()),
            graph_name,
        )?;
    }
    if let Some(ct) = p.content_type {
        // Try mapping registry first, then fall back to defaults
        let type_iri = p
            .registry
            .resolve_type(ct)
            .map(|s| s.to_string())
            .unwrap_or_else(|| default_type_iri(ct).to_string());
        store.insert_triple_into(
            page_uri.as_str(),
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
            &ObjectValue::Iri(type_iri),
            graph_name,
        )?;
    }
    if let Some(url) = p.url {
        store.insert_triple_into(
            page_uri.as_str(),
            "http://schema.org/url",
            &ObjectValue::Literal(url.to_string()),
            graph_name,
        )?;
    }
    Ok(())
}

/// Insert `[rdf.custom]` fields as triples in the graph.
/// Keys are predicate IRIs (full or prefixed, e.g. "geoff:stage" or "http://example.org/prop").
/// Values are converted from JSON to RDF literals.
fn insert_custom_triples(
    store: &ContentStore,
    page_uri: &PageUri,
    graph_name: &str,
    rdf_fields: &HashMap<String, JsonValue>,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    for (key, value) in rdf_fields {
        // Expand prefixed IRIs (e.g. "geoff:stage" → "urn:geoff:ontology:stage")
        let predicate = MappingRegistry::expand_iri(key).unwrap_or_else(|| key.clone());
        let obj = json_to_object_value(value);
        store.insert_triple_into(page_uri.as_str(), &predicate, &obj, graph_name)?;
    }
    Ok(())
}

/// Convert a JSON value to an RDF object value.
fn json_to_object_value(value: &JsonValue) -> ObjectValue {
    match value {
        JsonValue::String(s) => ObjectValue::Literal(s.clone()),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                ObjectValue::TypedLiteral {
                    value: i.to_string(),
                    datatype: xsd::INTEGER.to_string(),
                }
            } else if let Some(f) = n.as_f64() {
                ObjectValue::TypedLiteral {
                    value: f.to_string(),
                    datatype: xsd::DOUBLE.to_string(),
                }
            } else {
                ObjectValue::Literal(n.to_string())
            }
        }
        JsonValue::Bool(b) => ObjectValue::TypedLiteral {
            value: b.to_string(),
            datatype: xsd::BOOLEAN.to_string(),
        },
        _ => ObjectValue::Literal(value.to_string()),
    }
}

/// Build all pages and return them as an in-memory map of URL path -> HTML.
pub fn build_to_memory(
    site_root: &Utf8Path,
    config: &SiteConfig,
    store: &ContentStore,
    renderer: &SiteRenderer,
) -> std::result::Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let pages = build_site(site_root, config, store, renderer)?;
    let mut map = HashMap::new();
    for page in pages {
        // Normalize path: "index.html" -> "/", "blog/first-post.html" -> "/blog/first-post.html"
        let normalized = normalize_path(&page.output_path);
        let url_path = if normalized == "index.html" {
            "/".to_string()
        } else {
            format!("/{normalized}")
        };
        map.insert(url_path, page.html);
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn json_to_object_value_string() {
        let val = JsonValue::String("hello".into());
        assert_eq!(json_to_object_value(&val), ObjectValue::Literal("hello".into()));
    }

    #[test]
    fn json_to_object_value_integer() {
        let val = serde_json::json!(42);
        assert_eq!(
            json_to_object_value(&val),
            ObjectValue::TypedLiteral {
                value: "42".into(),
                datatype: xsd::INTEGER.into(),
            }
        );
    }

    #[test]
    fn json_to_object_value_bool() {
        let val = serde_json::json!(true);
        assert_eq!(
            json_to_object_value(&val),
            ObjectValue::TypedLiteral {
                value: "true".into(),
                datatype: xsd::BOOLEAN.into(),
            }
        );
    }

    #[test]
    fn insert_custom_triples_expands_prefixed_iris() {
        let store = ContentStore::new().unwrap();
        let page_uri = PageUri::from_path("test.md");
        let graph_name = page_uri.as_str();

        let mut fields = HashMap::new();
        fields.insert("geoff:stage".to_string(), JsonValue::String("develop".into()));
        fields.insert(
            "http://example.org/custom".to_string(),
            JsonValue::String("value".into()),
        );

        insert_custom_triples(&store, &page_uri, graph_name, &fields).unwrap();

        // Query the expanded geoff:stage triple
        let results = store
            .query_to_json(
                "SELECT ?val WHERE { GRAPH ?g { ?s <urn:geoff:ontology:stage> ?val } }",
            )
            .unwrap();
        let arr = results.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["val"], "develop");

        // Query the full IRI triple
        let results = store
            .query_to_json(
                "SELECT ?val WHERE { GRAPH ?g { ?s <http://example.org/custom> ?val } }",
            )
            .unwrap();
        let arr = results.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["val"], "value");
    }

    #[test]
    fn build_site_ingests_rdf_custom_fields() {
        let dir = tempfile::tempdir().unwrap();
        let site_root = camino::Utf8Path::from_path(dir.path()).unwrap();

        // Create geoff.toml
        std::fs::write(
            site_root.join("geoff.toml"),
            "base_url = \"https://example.com\"\ntitle = \"Test\"\n",
        )
        .unwrap();

        // Create content with [rdf.custom]
        let content_dir = site_root.join("content");
        std::fs::create_dir_all(&content_dir).unwrap();
        std::fs::write(
            content_dir.join("project.md"),
            r#"+++
title = "My Project"
template = "page.html"
type = "Web Page"
description = "A test project"

[rdf.custom]
"geoff:stage" = "develop"
"geoff:status" = "Active"
"geoff:language" = "Rust"
+++

# My Project
"#,
        )
        .unwrap();

        // Create minimal template
        let tmpl_dir = site_root.join("templates");
        std::fs::create_dir_all(&tmpl_dir).unwrap();
        std::fs::write(
            tmpl_dir.join("page.html"),
            "<h1>{{ title }}</h1>\n{{ content | safe }}",
        )
        .unwrap();

        // Build
        let config = SiteConfig::from_file(&site_root.join("geoff.toml")).unwrap();
        let store = Arc::new(ContentStore::new().unwrap());
        let mut renderer = crate::renderer::SiteRenderer::new(
            &site_root.join(&config.template_dir),
        )
        .unwrap();
        renderer.register_sparql_function(store.clone());

        let pages = build_site(site_root, &config, &store, &renderer).unwrap();
        assert_eq!(pages.len(), 1);

        // Verify custom fields are in the graph
        let results = store
            .query_to_json(
                "SELECT ?stage ?status ?lang WHERE { GRAPH ?g { ?s <urn:geoff:ontology:stage> ?stage . ?s <urn:geoff:ontology:status> ?status . ?s <urn:geoff:ontology:language> ?lang } }",
            )
            .unwrap();
        let arr = results.as_array().unwrap();
        assert_eq!(arr.len(), 1, "Expected 1 result, got: {arr:?}");
        assert_eq!(arr[0]["stage"], "develop");
        assert_eq!(arr[0]["status"], "Active");
        assert_eq!(arr[0]["lang"], "Rust");

        // Verify description is also in the graph
        let results = store
            .query_to_json(
                "SELECT ?desc WHERE { GRAPH ?g { ?s <http://schema.org/description> ?desc } }",
            )
            .unwrap();
        let arr = results.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["desc"], "A test project");
    }
}

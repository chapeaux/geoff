use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use camino::Utf8Path;
use geoff_content::frontmatter::{parse_frontmatter, split_frontmatter, toml_to_json};
use geoff_content::markdown::render_markdown;
use geoff_content::scanner::{scan_content_dir, scan_data_dir, sidecar_ttl_path};
use geoff_core::cache::{BuildCache, hash_file};
use geoff_core::config::SiteConfig;
use geoff_core::types::{ObjectValue, PageUri, normalize_path, xsd};
use geoff_graph::store::ContentStore;
use geoff_ontology::mappings::MappingRegistry;
use geoff_theme::TokenValue;
use rayon::prelude::*;
use serde_json::Value as JsonValue;

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
pub struct ParsedPage {
    pub output_path: String,
    pub page_url: String,
    pub page_uri: String,
    pub rdfa_attrs: String,
    pub critical_css: String,
    pub content_html: String,
    pub json_ld_str: String,
    pub template: String,
    pub title: String,
    pub date: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub frontmatter: serde_json::Value,
    pub extra_vars: std::collections::HashMap<String, serde_json::Value>,
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
    let (to_render, stats, page_index) = ingest_content(site_root, config, store, cache)?;
    renderer.set_page_index(page_index);
    let pages = render_pages(&to_render, config, renderer, stats.skipped)?;
    let built = pages.len();
    Ok((pages, BuildStats { built, ..stats }))
}

/// Intermediate result from content ingestion, ready for a hook point before rendering.
pub struct IngestResult {
    pub to_render: Vec<ParsedPage>,
    pub stats: BuildStats,
}

/// Phase 1: Scan, parse, and ingest all content into the RDF graph.
/// Returns parsed pages ready for rendering, build statistics, and a page index
/// containing metadata for ALL pages (including those skipped by incremental builds).
/// Call this, then run plugin hooks (e.g. on_graph_updated), then call render_pages.
/// Result of content ingestion: (pages to render, build stats, page index for all pages).
pub type IngestOutput = (Vec<ParsedPage>, BuildStats, Vec<serde_json::Value>);

pub fn ingest_content(
    site_root: &Utf8Path,
    config: &SiteConfig,
    store: &ContentStore,
    cache: Option<&BuildCache>,
) -> std::result::Result<IngestOutput, Box<dyn std::error::Error>> {
    let content_dir = site_root.join(&config.content_dir);

    let mappings_path = site_root.join("ontology/mappings.toml");
    let mut registry = MappingRegistry::load(&mappings_path)?;
    if !config.linked_data.prefixes.is_empty() {
        registry.add_prefixes(config.linked_data.prefixes.clone());
    }

    let data_dir = content_dir.join("data");
    for ttl_file in scan_data_dir(&data_dir)? {
        store.load_turtle(&ttl_file)?;
    }

    let files = scan_content_dir(&content_dir)?;
    let mut stats = BuildStats {
        total: files.len(),
        ..Default::default()
    };

    let templates_changed = if let Some(c) = cache {
        let template_dir = site_root.join(&config.template_dir);
        let current_hash = geoff_core::cache::hash_directory(&template_dir)?;
        c.template_hash.as_deref() != Some(current_hash.as_str())
    } else {
        true
    };

    let sparql_templates = find_sparql_templates(&site_root.join(&config.template_dir))?;

    let mut to_render: Vec<ParsedPage> = Vec::new();
    let mut page_index: Vec<serde_json::Value> = Vec::new();

    for file_path in &files {
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
            if !sparql_templates.is_empty()
                && let Ok(template) = read_frontmatter_template(file_path)
                && sparql_templates.contains(&template)
            {
                if let Some(parsed) =
                    parse_and_ingest(file_path, &content_dir, config, store, &registry)?
                {
                    page_index.push(build_page_entry(&parsed));
                    to_render.push(parsed);
                }
                continue;
            }

            if let Some(entry) =
                ingest_triples_only(file_path, &content_dir, config, store, &registry)?
            {
                page_index.push(entry);
            }
            stats.skipped += 1;
            continue;
        }

        if let Some(parsed) = parse_and_ingest(file_path, &content_dir, config, store, &registry)? {
            page_index.push(build_page_entry(&parsed));
            to_render.push(parsed);
        }
    }

    // Populate critical CSS per page from static/critical*.css files
    let critical_files = scan_critical_css(&site_root.join("static"));
    if !critical_files.is_empty() {
        for page in &mut to_render {
            page.critical_css = build_critical_css_for_page(&page.template, &critical_files);
        }
    }

    Ok((to_render, stats, page_index))
}

/// Scan the static directory for critical CSS files.
/// Returns a list of (category, css_content) where category is:
/// - `"*"` for `critical.css` (global, inlined on all pages)
/// - template stem for `critical-{stem}.css` (template-specific)
fn scan_critical_css(static_dir: &Utf8Path) -> Vec<(String, String)> {
    let mut files = Vec::new();
    if !static_dir.exists() {
        return files;
    }

    let Ok(entries) = std::fs::read_dir(static_dir) else {
        return files;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        if name == "critical.css"
            && let Ok(css) = std::fs::read_to_string(&path)
        {
            files.push(("*".to_string(), css));
        } else if let Some(stem) = name.strip_prefix("critical-")
            && let Some(stem) = stem.strip_suffix(".css")
            && let Ok(css) = std::fs::read_to_string(&path)
        {
            files.push((stem.to_string(), css));
        }
    }

    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

/// Build the critical CSS string for a page based on its template.
fn build_critical_css_for_page(template: &str, critical_files: &[(String, String)]) -> String {
    let template_stem = template.strip_suffix(".html").unwrap_or(template);
    let mut css = String::new();
    for (category, content) in critical_files {
        if category == "*" || category == template_stem {
            css.push_str(content);
            css.push('\n');
        }
    }
    css
}

/// Phase 2: Render parsed pages in parallel using Rayon.
/// Call this after ingest_content and any plugin hooks.
pub fn render_pages(
    to_render: &[ParsedPage],
    config: &SiteConfig,
    renderer: &SiteRenderer,
    _skipped: usize,
) -> std::result::Result<Vec<BuiltPage>, Box<dyn std::error::Error>> {
    let render_count = AtomicUsize::new(0);
    let total_to_render = to_render.len();

    let results: Vec<std::result::Result<BuiltPage, String>> = to_render
        .par_iter()
        .map(|parsed| {
            let mut ctx = build_page_context(&PageContext {
                title: &parsed.title,
                content_html: &parsed.content_html,
                json_ld: &parsed.json_ld_str,
                site_title: &config.title,
                page_url: &parsed.page_url,
                page_uri: &parsed.page_uri,
                rdfa_attrs: &parsed.rdfa_attrs,
                critical_css: &parsed.critical_css,
                date: parsed.date.as_deref(),
                author: parsed.author.as_deref(),
                description: parsed.description.as_deref(),
                tags: parsed.tags.as_deref(),
            });

            ctx.insert("frontmatter", &parsed.frontmatter);

            // Inject plugin-provided extra variables into the template context
            for (k, v) in &parsed.extra_vars {
                ctx.insert(k, v);
            }

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
        eprintln!();
    }

    let mut pages = Vec::with_capacity(results.len());
    for result in results {
        pages.push(result.map_err(|e| -> Box<dyn std::error::Error> { e.into() })?);
    }

    Ok(pages)
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

    let (frontmatter, rdf_fields, data_fields) = parse_frontmatter(fm_str)?;
    let mut html = render_markdown(body);

    if config.linked_data.rdfa_links {
        html = geoff_content::markdown::rewrite_rdfa_links(&html, registry);
    }

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
    let raw_output = normalize_path(rel_path.with_extension("html").as_ref());
    let output_name = apply_url_style(&raw_output, &config.build.url_style);
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
    insert_custom_triples(store, &page_uri, graph_name, &rdf_fields, registry)?;

    // Insert [data] fields as triples (friendly names resolved via registry)
    insert_data_triples(store, &page_uri, graph_name, &data_fields, registry)?;

    if let Some(sidecar_path) = sidecar_ttl_path(file_path) {
        store.load_turtle_into(&sidecar_path, graph_name)?;
    }

    // Build JSON-LD (rich graph-based version includes all triples)
    let jsonld = if config.linked_data.rich_jsonld {
        crate::jsonld::build_jsonld_from_graph(
            store,
            page_uri.as_str(),
            &config.base_url,
            &page_url,
            &config.linked_data.default_vocab,
            registry,
        )
    } else {
        let page_output_path = normalize_path(rel_path.with_extension("").as_ref());
        build_jsonld(
            &config.base_url,
            &page_output_path,
            Some(&title),
            date.as_deref(),
            author.as_deref(),
            content_type.as_deref(),
        )
    };
    let json_ld_str = serde_json::to_string_pretty(&jsonld)?;

    let rdfa_attrs = build_rdfa_attrs(
        content_type.as_deref(),
        &page_url,
        &config.linked_data.default_vocab,
        registry,
    );

    Ok(Some(ParsedPage {
        output_path: output_name,
        page_url,
        page_uri: page_uri.as_str().to_string(),
        rdfa_attrs,
        critical_css: String::new(),
        content_html: html,
        json_ld_str,
        template,
        title,
        date,
        author,
        description,
        tags,
        frontmatter: toml_to_json(&frontmatter),
        extra_vars: std::collections::HashMap::new(),
    }))
}

/// Ingest triples for a file without rendering it (for incremental builds).
/// Returns a page index entry if the file was successfully parsed.
fn ingest_triples_only(
    file_path: &Utf8Path,
    content_dir: &Utf8Path,
    config: &SiteConfig,
    store: &ContentStore,
    registry: &MappingRegistry,
) -> std::result::Result<Option<serde_json::Value>, Box<dyn std::error::Error>> {
    let raw = std::fs::read_to_string(file_path)?;
    let (fm_str, _body) = match split_frontmatter(&raw) {
        Ok(pair) => pair,
        Err(_) => return Ok(None),
    };
    let (frontmatter, rdf_fields, data_fields) = parse_frontmatter(fm_str)?;

    let rel_path = file_path.strip_prefix(content_dir).unwrap_or(file_path);
    let raw_output = normalize_path(rel_path.with_extension("html").as_ref());
    let output_name = apply_url_style(&raw_output, &config.build.url_style);
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
        description: frontmatter.get("description").and_then(|v| v.as_str()),
        content_type: frontmatter.get("type").and_then(|v| v.as_str()),
        url: Some(&page_url),
        registry,
    })?;

    // Insert [rdf.custom] fields as triples
    insert_custom_triples(store, &page_uri, graph_name, &rdf_fields, registry)?;

    // Insert [data] fields as triples (friendly names resolved via registry)
    insert_data_triples(store, &page_uri, graph_name, &data_fields, registry)?;

    if let Some(sidecar_path) = sidecar_ttl_path(file_path) {
        store.load_turtle_into(&sidecar_path, graph_name)?;
    }

    Ok(Some(build_page_entry_from_frontmatter(
        &frontmatter,
        &page_url,
    )))
}

/// Default type mappings used when the mapping registry has no entry.
fn default_type_iri(content_type: &str) -> &str {
    match content_type {
        "Blog Post" | "BlogPosting" => "https://schema.org/BlogPosting",
        "Article" => "https://schema.org/Article",
        "How-To Guide" | "HowTo" => "https://schema.org/HowTo",
        "FAQ Page" | "FAQPage" => "https://schema.org/FAQPage",
        "Event" => "https://schema.org/Event",
        "Web Page" | "WebPage" => "https://schema.org/WebPage",
        _ => "https://schema.org/WebPage",
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

/// Build RDFa attributes for the page container element.
/// Returns a string like `vocab="https://schema.org/" typeof="BlogPosting" resource="/blog/welcome/"`.
fn build_rdfa_attrs(
    content_type: Option<&str>,
    page_url: &str,
    default_vocab: &str,
    registry: &MappingRegistry,
) -> String {
    let mut attrs = format!("vocab=\"{}\"", default_vocab);

    if let Some(ct) = content_type {
        let type_iri = registry
            .resolve_type(ct)
            .map(|s| s.to_string())
            .unwrap_or_else(|| default_type_iri(ct).to_string());

        // Compact for display: strip the default vocab prefix if possible
        let type_name = if let Some(local) = type_iri.strip_prefix(default_vocab) {
            local.to_string()
        } else {
            registry.compact_iri(&type_iri)
        };
        attrs.push_str(&format!(" typeof=\"{type_name}\""));
    }

    attrs.push_str(&format!(" resource=\"{}\"", page_url));
    attrs
}

/// Build a page index entry from a ParsedPage.
fn build_page_entry(parsed: &ParsedPage) -> serde_json::Value {
    let mut entry = match &parsed.frontmatter {
        serde_json::Value::Object(m) => serde_json::Value::Object(m.clone()),
        _ => serde_json::Value::Object(serde_json::Map::new()),
    };
    let obj = entry.as_object_mut().unwrap();
    obj.insert("url".to_string(), serde_json::json!(parsed.page_url));
    obj.insert("title".to_string(), serde_json::json!(parsed.title));
    if let Some(d) = &parsed.date {
        obj.insert("date".to_string(), serde_json::json!(d));
    }
    if let Some(a) = &parsed.author {
        obj.insert("author".to_string(), serde_json::json!(a));
    }
    if let Some(desc) = &parsed.description {
        obj.insert("description".to_string(), serde_json::json!(desc));
    }
    if let Some(t) = &parsed.tags {
        obj.insert("tags".to_string(), serde_json::json!(t));
    }
    entry
}

/// Build a page index entry from raw frontmatter (for incremental builds where
/// the page is not being rendered but still needs to appear in pages()/tree()).
fn build_page_entry_from_frontmatter(
    frontmatter: &toml::Value,
    page_url: &str,
) -> serde_json::Value {
    let mut entry = match toml_to_json(frontmatter) {
        serde_json::Value::Object(m) => serde_json::Value::Object(m),
        _ => serde_json::Value::Object(serde_json::Map::new()),
    };
    let obj = entry.as_object_mut().unwrap();
    obj.insert("url".to_string(), serde_json::json!(page_url));
    if !obj.contains_key("title") {
        obj.insert("title".to_string(), serde_json::json!("Untitled"));
    }
    entry
}

/// Convert an output file path to a URL path.
/// e.g. "blog/2026-03-30-welcome.html" → "/blog/2026-03-30-welcome.html"
///      "blog/index.html" → "/blog/"
///      "index.html" → "/"
/// Convert a file path to directory-style output: `about.html` → `about/index.html`.
/// Leaves `index.html` and paths already ending in `/index.html` unchanged.
fn apply_url_style(output_path: &str, style: &geoff_core::config::UrlStyle) -> String {
    match style {
        geoff_core::config::UrlStyle::File => output_path.to_string(),
        geoff_core::config::UrlStyle::Directory => {
            if output_path == "index.html" || output_path.ends_with("/index.html") {
                output_path.to_string()
            } else if let Some(stem) = output_path.strip_suffix(".html") {
                format!("{stem}/index.html")
            } else {
                output_path.to_string()
            }
        }
    }
}

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
            "https://schema.org/name",
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
            "https://schema.org/datePublished",
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
            "https://schema.org/author",
            &ObjectValue::Literal(a.to_string()),
            graph_name,
        )?;
    }
    if let Some(desc) = p.description {
        store.insert_triple_into(
            page_uri.as_str(),
            "https://schema.org/description",
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
            "https://schema.org/url",
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
    registry: &MappingRegistry,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    for (key, value) in rdf_fields {
        let predicate = registry.expand_iri(key).unwrap_or_else(|| key.clone());
        let obj = json_to_object_value(value);
        store.insert_triple_into(page_uri.as_str(), &predicate, &obj, graph_name)?;
    }
    Ok(())
}

/// Insert `[data]` fields as triples, resolving friendly names via the mapping registry.
/// Resolution chain: registry.resolve_property() -> registry.expand_iri() -> urn:geoff:meta:{key}
fn insert_data_triples(
    store: &ContentStore,
    page_uri: &PageUri,
    graph_name: &str,
    data_fields: &HashMap<String, JsonValue>,
    registry: &MappingRegistry,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    for (key, value) in data_fields {
        let predicate = if let Some(iri) = registry.resolve_property(key) {
            iri.to_string()
        } else if let Some(expanded) = registry.expand_iri(key) {
            expanded
        } else {
            format!("urn:geoff:meta:{key}")
        };
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

/// Result of loading and processing a theme's design tokens.
pub struct ThemeResult {
    /// Full CSS custom properties (non-critical, deferred).
    pub css: String,
    /// Critical CSS custom properties.
    pub critical_css: String,
    /// The merged DTCG token JSON (after base merge, before CSS conversion).
    pub merged_json: serde_json::Value,
}

/// Load theme tokens, resolve references, generate CSS, register the theme
/// function on the renderer, and insert tokens into the RDF graph.
///
/// Returns the generated CSS strings for writing to output.
/// Load design system tokens from `[design]` config.
/// Merges multiple files in order (later overrides earlier).
fn load_design_system_tokens(
    site_root: &Utf8Path,
    config: &SiteConfig,
) -> std::result::Result<
    Option<std::collections::BTreeMap<String, geoff_theme::FlatToken>>,
    Box<dyn std::error::Error>,
> {
    if config.design.tokens.is_empty() {
        return Ok(None);
    }

    let mut merged: Option<serde_json::Value> = None;
    for token_path in &config.design.tokens {
        let path = site_root.join(token_path);
        if !path.exists() {
            return Err(format!("Design system token file not found: {path}").into());
        }
        let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
        merged = Some(match merged {
            Some(base) => geoff_theme::merge_tokens(&base, &json),
            None => json,
        });
    }

    let merged = merged.unwrap();
    let tokens = geoff_theme::DesignTokens::from_json(&merged.to_string())?;
    let mut flat = tokens.flatten();
    geoff_theme::resolve_references(&mut flat);

    // Check for unresolved references within the design system
    let unresolved = geoff_theme::find_unresolved(&flat);
    if !unresolved.is_empty() {
        let file_list = config
            .design
            .tokens
            .iter()
            .enumerate()
            .map(|(i, p)| format!("    {}. {p}", i + 1))
            .collect::<Vec<_>>()
            .join("\n");
        let ref_list = unresolved
            .iter()
            .map(|u| format!("  - token `{}` references `{}`", u.token_path, u.reference))
            .collect::<Vec<_>>()
            .join("\n");
        return Err(format!(
            "Unresolved token references in design system:\n{ref_list}\n\n  Token files loaded:\n{file_list}"
        )
        .into());
    }

    Ok(Some(flat))
}

pub fn load_and_register_theme(
    site_root: &Utf8Path,
    config: &SiteConfig,
    renderer: &mut SiteRenderer,
    store: &ContentStore,
) -> std::result::Result<Option<ThemeResult>, Box<dyn std::error::Error>> {
    let theme_name = match &config.theme.name {
        Some(name) => name.clone(),
        None => return Ok(None),
    };

    let theme_dir = site_root.join("themes").join(&theme_name);

    // Load design system tokens if configured
    let system_flat = load_design_system_tokens(site_root, config)?;

    // Determine which token file to load for the theme
    let theme_token_file = if system_flat.is_some() {
        theme_dir.join("theme.json")
    } else {
        theme_dir.join("tokens.json")
    };

    if !theme_token_file.exists() {
        if system_flat.is_some() {
            eprintln!(
                "warning: No theme.json found at {theme_token_file}. Run `geoff theme generate {theme_name}` to create one from the design system."
            );
        }
        return Ok(None);
    }

    // 1. Read theme tokens
    let theme_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&theme_token_file)?)?;

    // 2. If base theme is set, read and merge base tokens
    let merged_json = if let Some(base_name) = &config.theme.base {
        let base_tokens_path = site_root.join("themes").join(base_name).join("tokens.json");
        if base_tokens_path.exists() {
            let base_json: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&base_tokens_path)?)?;
            geoff_theme::merge_tokens(&base_json, &theme_json)
        } else {
            theme_json
        }
    } else {
        theme_json
    };

    // 3. Parse and flatten tokens
    let tokens = geoff_theme::DesignTokens::from_json(&merged_json.to_string())?;
    let mut flat = tokens.flatten();

    // 4. Resolve references — against design system if available, else self-only
    if let Some(ref system) = system_flat {
        geoff_theme::resolve_references_with_base(&mut flat, system);
    } else {
        geoff_theme::resolve_references(&mut flat);
    }

    // 4b. Check for unresolved references
    let unresolved = geoff_theme::find_unresolved(&flat);
    if !unresolved.is_empty() {
        let mut file_hierarchy = Vec::new();
        for (i, p) in config.design.tokens.iter().enumerate() {
            file_hierarchy.push(format!("    {}. {p} (design system)", i + 1));
        }
        file_hierarchy.push(format!(
            "    {}. {} (theme)",
            config.design.tokens.len() + 1,
            theme_token_file
        ));
        let ref_list = unresolved
            .iter()
            .map(|u| format!("  - token `{}` references `{}`", u.token_path, u.reference))
            .collect::<Vec<_>>()
            .join("\n");
        return Err(format!(
            "Unresolved token references in {}:\n{ref_list}\n\n  Token files loaded (in order):\n{}",
            theme_token_file,
            file_hierarchy.join("\n")
        )
        .into());
    }

    // 5. Handle dark mode tokens if configured (legacy path, no [design])
    let dark_flat = if system_flat.is_none() {
        if let Some(dark_file) = &config.theme.modes.dark {
            let dark_path = theme_dir.join(dark_file);
            if dark_path.exists() {
                let dark_tokens = geoff_theme::DesignTokens::from_file(camino::Utf8Path::new(
                    dark_path.as_str(),
                ))?;
                let mut dark_flat = dark_tokens.flatten();
                geoff_theme::resolve_references(&mut dark_flat);
                Some(dark_flat)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    // 6. Generate CSS
    let prefix = config.theme.prefix.as_deref();
    let css = geoff_theme::generate_css_with_prefix(&flat, dark_flat.as_ref(), false, prefix);
    let critical_css =
        geoff_theme::generate_css_with_prefix(&flat, dark_flat.as_ref(), true, prefix);

    // 7. Register the theme_css function on the renderer
    renderer.register_theme_function(css.clone(), critical_css.clone());

    // 8. Insert tokens as triples into the RDF graph
    insert_tokens_into_graph(store, &flat)?;

    Ok(Some(ThemeResult {
        css,
        critical_css,
        merged_json: merged_json.clone(),
    }))
}

/// Insert flattened design tokens into the RDF graph as triples.
///
/// For each token, three triples are inserted:
/// - `<urn:geoff:design-token:{path}> <urn:geoff:design-token:type> "{token_type}"`
/// - `<urn:geoff:design-token:{path}> <urn:geoff:design-token:value> "{css_value}"`
/// - `<urn:geoff:design-token:{path}> <urn:geoff:design-token:cssVariable> "--{var-name}"`
fn insert_tokens_into_graph(
    store: &ContentStore,
    tokens: &std::collections::BTreeMap<String, geoff_theme::FlatToken>,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let graph = "urn:geoff:design-tokens";

    for (path, token) in tokens {
        let subject = format!("urn:geoff:design-token:{path}");

        // Insert type
        if let Some(ref token_type) = token.token_type {
            store.insert_triple_into(
                &subject,
                "urn:geoff:design-token:type",
                &ObjectValue::Literal(token_type.clone()),
                graph,
            )?;
        }

        // Insert value as a CSS-formatted string
        let css_value = token_value_to_string(&token.value);
        store.insert_triple_into(
            &subject,
            "urn:geoff:design-token:value",
            &ObjectValue::Literal(css_value),
            graph,
        )?;

        // Insert CSS variable name
        let var_name = path_to_css_var(path);
        store.insert_triple_into(
            &subject,
            "urn:geoff:design-token:cssVariable",
            &ObjectValue::Literal(var_name),
            graph,
        )?;
    }

    Ok(())
}

/// Convert a token value to its string representation for RDF storage.
fn token_value_to_string(value: &TokenValue) -> String {
    match value {
        TokenValue::String(s) => s.clone(),
        TokenValue::Number(n) => {
            if n.fract() == 0.0 {
                format!("{}", *n as i64)
            } else {
                format!("{n}")
            }
        }
        TokenValue::Bool(b) => b.to_string(),
        TokenValue::Object(obj) => serde_json::to_string(obj).unwrap_or_default(),
        TokenValue::Array(arr) => serde_json::to_string(arr).unwrap_or_default(),
    }
}

/// Convert a dot-separated token path to a CSS custom property name.
fn path_to_css_var(path: &str) -> String {
    let mut result = String::with_capacity(path.len() + 2);
    result.push_str("--");
    let chars: Vec<char> = path.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c == '.' {
            result.push('-');
        } else if c.is_uppercase() && i > 0 && chars[i - 1] != '.' {
            result.push('-');
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c.to_ascii_lowercase());
        }
    }
    result
}

/// Scan template files for `sparql(` usage and return the set of template names that contain it.
fn find_sparql_templates(
    template_dir: &Utf8Path,
) -> std::result::Result<std::collections::HashSet<String>, Box<dyn std::error::Error>> {
    let mut result = std::collections::HashSet::new();
    if !template_dir.exists() {
        return Ok(result);
    }
    fn walk(
        dir: &std::path::Path,
        base: &std::path::Path,
        out: &mut std::collections::HashSet<String>,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                walk(&path, base, out)?;
            } else if let Ok(content) = std::fs::read_to_string(&path)
                && content.contains("sparql(")
                && let Ok(rel) = path.strip_prefix(base)
            {
                out.insert(rel.to_string_lossy().into_owned());
            }
        }
        Ok(())
    }
    walk(
        template_dir.as_std_path(),
        template_dir.as_std_path(),
        &mut result,
    )?;
    Ok(result)
}

/// Read just the template name from a content file's frontmatter.
fn read_frontmatter_template(
    file_path: &Utf8Path,
) -> std::result::Result<String, Box<dyn std::error::Error>> {
    let raw = std::fs::read_to_string(file_path)?;
    let (fm_str, _) = split_frontmatter(&raw)?;
    let (frontmatter, _, _) = parse_frontmatter(fm_str)?;
    Ok(frontmatter
        .get("template")
        .and_then(|v| v.as_str())
        .unwrap_or("page.html")
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn json_to_object_value_string() {
        let val = JsonValue::String("hello".into());
        assert_eq!(
            json_to_object_value(&val),
            ObjectValue::Literal("hello".into())
        );
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
        fields.insert(
            "geoff:stage".to_string(),
            JsonValue::String("develop".into()),
        );
        fields.insert(
            "http://example.org/custom".to_string(),
            JsonValue::String("value".into()),
        );

        let registry = MappingRegistry::new();
        insert_custom_triples(&store, &page_uri, graph_name, &fields, &registry).unwrap();

        // Query the expanded geoff:stage triple
        let results = store
            .query_to_json("SELECT ?val WHERE { GRAPH ?g { ?s <urn:geoff:ontology:stage> ?val } }")
            .unwrap();
        let arr = results.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["val"], "develop");

        // Query the full IRI triple
        let results = store
            .query_to_json("SELECT ?val WHERE { GRAPH ?g { ?s <http://example.org/custom> ?val } }")
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
        let mut renderer =
            crate::renderer::SiteRenderer::new(&site_root.join(&config.template_dir)).unwrap();
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
                "SELECT ?desc WHERE { GRAPH ?g { ?s <https://schema.org/description> ?desc } }",
            )
            .unwrap();
        let arr = results.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["desc"], "A test project");
    }

    #[test]
    fn find_sparql_templates_detects_usage() {
        let dir = tempfile::tempdir().unwrap();
        let tmpl_dir = Utf8Path::from_path(dir.path()).unwrap();

        std::fs::write(
            tmpl_dir.join("page.html").as_std_path(),
            "<h1>{{ title }}</h1>",
        )
        .unwrap();
        std::fs::write(
            tmpl_dir.join("listing.html").as_std_path(),
            r#"{% set items = sparql(query="SELECT ?t WHERE { ?s ?p ?t }") %}"#,
        )
        .unwrap();

        let result = find_sparql_templates(tmpl_dir).unwrap();
        assert!(result.contains("listing.html"));
        assert!(!result.contains("page.html"));
    }

    #[test]
    fn sparql_dependent_page_rebuilt_on_incremental() {
        let dir = tempfile::tempdir().unwrap();
        let site_root = Utf8Path::from_path(dir.path()).unwrap();

        std::fs::write(
            site_root.join("geoff.toml"),
            "base_url = \"https://example.com\"\ntitle = \"Test\"\n",
        )
        .unwrap();

        let content_dir = site_root.join("content");
        std::fs::create_dir_all(&content_dir).unwrap();

        // A listing page with a SPARQL template
        std::fs::write(
            content_dir.join("index.md"),
            "+++\ntitle = \"Home\"\ntemplate = \"listing.html\"\n+++\nHome page\n",
        )
        .unwrap();

        // A regular page
        std::fs::write(
            content_dir.join("about.md"),
            "+++\ntitle = \"About\"\ntemplate = \"page.html\"\n+++\nAbout\n",
        )
        .unwrap();

        let tmpl_dir = site_root.join("templates");
        std::fs::create_dir_all(&tmpl_dir).unwrap();
        std::fs::write(
            tmpl_dir.join("page.html"),
            "<h1>{{ title }}</h1>\n{{ content | safe }}",
        )
        .unwrap();
        std::fs::write(
            tmpl_dir.join("listing.html"),
            r#"{% set results = sparql(query="SELECT ?title WHERE { GRAPH ?g { ?s <https://schema.org/name> ?title } }") %}{% for row in results %}{{ row.title }} {% endfor %}"#,
        )
        .unwrap();

        let config = SiteConfig::from_file(&site_root.join("geoff.toml")).unwrap();
        let store = Arc::new(ContentStore::new().unwrap());
        let mut renderer =
            crate::renderer::SiteRenderer::new(&site_root.join(&config.template_dir)).unwrap();
        renderer.register_sparql_function(store.clone());

        // First build: everything renders
        let (pages, stats) =
            build_site_incremental(site_root, &config, &store, &renderer, None).unwrap();
        assert_eq!(stats.built, 2);
        assert_eq!(stats.skipped, 0);
        let listing = pages
            .iter()
            .find(|p| p.output_path == "index.html")
            .unwrap();
        assert!(listing.html.contains("Home"));
        assert!(listing.html.contains("About"));

        // Build a cache from the first build
        let mut cache = BuildCache::default();
        let template_hash =
            geoff_core::cache::hash_directory(&site_root.join(&config.template_dir)).unwrap();
        cache.template_hash = Some(template_hash);
        for file in scan_content_dir(&content_dir).unwrap() {
            let rel = normalize_path(file.strip_prefix(&content_dir).unwrap_or(&file).as_str());
            let hash = hash_file(&file).unwrap();
            cache.record(rel, hash);
        }

        // Add a new blog post
        std::fs::write(
            content_dir.join("new-post.md"),
            "+++\ntitle = \"New Post\"\ntemplate = \"page.html\"\ntype = \"Blog Post\"\n+++\nNew content\n",
        )
        .unwrap();

        // Second incremental build with cache
        let store2 = Arc::new(ContentStore::new().unwrap());
        let mut renderer2 =
            crate::renderer::SiteRenderer::new(&site_root.join(&config.template_dir)).unwrap();
        renderer2.register_sparql_function(store2.clone());

        let (pages2, stats2) =
            build_site_incremental(site_root, &config, &store2, &renderer2, Some(&cache)).unwrap();

        // The new post should be built (it's new, not in cache)
        // The listing page should ALSO be rebuilt (uses SPARQL)
        // The about page should be skipped (unchanged, no SPARQL)
        assert_eq!(stats2.skipped, 1, "about.md should be skipped");
        assert!(
            stats2.built >= 2,
            "new-post.md and index.md should be built"
        );

        let listing2 = pages2
            .iter()
            .find(|p| p.output_path == "index.html")
            .expect("listing page should be in built pages");
        assert!(
            listing2.html.contains("New Post"),
            "listing should include the new post, got: {}",
            listing2.html
        );
    }

    #[test]
    fn page_url_and_frontmatter_available_in_templates() {
        let dir = tempfile::tempdir().unwrap();
        let site_root = camino::Utf8Path::from_path(dir.path()).unwrap();

        std::fs::write(
            site_root.join("geoff.toml"),
            "base_url = \"https://example.com\"\ntitle = \"Test\"\n",
        )
        .unwrap();

        let content_dir = site_root.join("content");
        std::fs::create_dir_all(&content_dir).unwrap();
        std::fs::write(
            content_dir.join("about.md"),
            "+++\ntitle = \"About Us\"\nnavSection = \"about\"\norder = 2\nheading = \"Learn More\"\n+++\nAbout page\n",
        )
        .unwrap();

        let tmpl_dir = site_root.join("templates");
        std::fs::create_dir_all(&tmpl_dir).unwrap();
        std::fs::write(
            tmpl_dir.join("page.html"),
            "url={{ page_url }} nav={{ frontmatter.navSection }} order={{ frontmatter.order }} heading={{ frontmatter.heading }} title={{ frontmatter.title }}",
        )
        .unwrap();

        let config = SiteConfig::from_file(&site_root.join("geoff.toml")).unwrap();
        let store = Arc::new(ContentStore::new().unwrap());
        let mut renderer =
            crate::renderer::SiteRenderer::new(&site_root.join(&config.template_dir)).unwrap();
        renderer.register_sparql_function(store.clone());

        let pages = build_site(site_root, &config, &store, &renderer).unwrap();
        assert_eq!(pages.len(), 1);
        let html = &pages[0].html;
        assert!(
            html.contains("url=/about.html"),
            "page_url should be /about.html, got: {html}"
        );
        assert!(
            html.contains("nav=about"),
            "frontmatter.navSection should be 'about', got: {html}"
        );
        assert!(
            html.contains("order=2"),
            "frontmatter.order should be 2, got: {html}"
        );
        assert!(
            html.contains("heading=Learn More"),
            "frontmatter.heading should be available, got: {html}"
        );
        assert!(
            html.contains("title=About Us"),
            "frontmatter.title should also be accessible, got: {html}"
        );
    }

    #[test]
    fn page_url_reflects_directory_style() {
        let dir = tempfile::tempdir().unwrap();
        let site_root = camino::Utf8Path::from_path(dir.path()).unwrap();

        std::fs::write(
            site_root.join("geoff.toml"),
            "base_url = \"https://example.com\"\ntitle = \"Test\"\n\n[build]\nurl_style = \"directory\"\n",
        )
        .unwrap();

        let content_dir = site_root.join("content");
        std::fs::create_dir_all(&content_dir).unwrap();
        std::fs::write(
            content_dir.join("about.md"),
            "+++\ntitle = \"About\"\n+++\nAbout\n",
        )
        .unwrap();

        let tmpl_dir = site_root.join("templates");
        std::fs::create_dir_all(&tmpl_dir).unwrap();
        std::fs::write(tmpl_dir.join("page.html"), "url={{ page_url }}").unwrap();

        let config = SiteConfig::from_file(&site_root.join("geoff.toml")).unwrap();
        let store = Arc::new(ContentStore::new().unwrap());
        let mut renderer =
            crate::renderer::SiteRenderer::new(&site_root.join(&config.template_dir)).unwrap();
        renderer.register_sparql_function(store.clone());

        let pages = build_site(site_root, &config, &store, &renderer).unwrap();
        assert_eq!(pages.len(), 1);
        assert!(
            pages[0].html.contains("url=/about/"),
            "directory style page_url should be /about/, got: {}",
            pages[0].html
        );
    }

    #[test]
    fn pages_function_filters_and_sorts() {
        let dir = tempfile::tempdir().unwrap();
        let site_root = camino::Utf8Path::from_path(dir.path()).unwrap();

        std::fs::write(
            site_root.join("geoff.toml"),
            "base_url = \"https://example.com\"\ntitle = \"Test\"\n",
        )
        .unwrap();

        let content_dir = site_root.join("content");
        let about_dir = content_dir.join("about");
        std::fs::create_dir_all(&about_dir).unwrap();
        std::fs::write(
            content_dir.join("index.md"),
            "+++\ntitle = \"Home\"\ntemplate = \"listing.html\"\n+++\nHome\n",
        )
        .unwrap();
        std::fs::write(
            about_dir.join("team.md"),
            "+++\ntitle = \"Team\"\norder = 2\nnavSection = \"about\"\n+++\nTeam\n",
        )
        .unwrap();
        std::fs::write(
            about_dir.join("mission.md"),
            "+++\ntitle = \"Mission\"\norder = 1\nnavSection = \"about\"\n+++\nMission\n",
        )
        .unwrap();
        std::fs::write(
            content_dir.join("blog.md"),
            "+++\ntitle = \"Blog\"\nnavSection = \"blog\"\n+++\nBlog\n",
        )
        .unwrap();

        let tmpl_dir = site_root.join("templates");
        std::fs::create_dir_all(&tmpl_dir).unwrap();
        std::fs::write(tmpl_dir.join("page.html"), "{{ title }}").unwrap();
        std::fs::write(
            tmpl_dir.join("listing.html"),
            r#"{% set nav = pages(navSection="about", sort="order") %}{% for p in nav %}{{ p.title }}({{ p.order }}) {% endfor %}"#,
        )
        .unwrap();

        let config = SiteConfig::from_file(&site_root.join("geoff.toml")).unwrap();
        let store = Arc::new(ContentStore::new().unwrap());
        let mut renderer =
            crate::renderer::SiteRenderer::new(&site_root.join(&config.template_dir)).unwrap();
        renderer.register_sparql_function(store.clone());

        let pages = build_site(site_root, &config, &store, &renderer).unwrap();
        let listing = pages
            .iter()
            .find(|p| p.output_path == "index.html")
            .unwrap();
        assert!(
            listing.html.contains("Mission(1)") && listing.html.contains("Team(2)"),
            "pages() should filter by navSection and sort by order, got: {}",
            listing.html
        );
        let mission_pos = listing.html.find("Mission").unwrap();
        let team_pos = listing.html.find("Team").unwrap();
        assert!(
            mission_pos < team_pos,
            "Mission (order=1) should come before Team (order=2)"
        );
    }

    #[test]
    fn tree_function_builds_hierarchy() {
        let dir = tempfile::tempdir().unwrap();
        let site_root = camino::Utf8Path::from_path(dir.path()).unwrap();

        std::fs::write(
            site_root.join("geoff.toml"),
            "base_url = \"https://example.com\"\ntitle = \"Test\"\n\n[build]\nurl_style = \"directory\"\n",
        )
        .unwrap();

        let content_dir = site_root.join("content");
        let about_dir = content_dir.join("about");
        std::fs::create_dir_all(&about_dir).unwrap();
        std::fs::write(
            content_dir.join("index.md"),
            "+++\ntitle = \"Home\"\ntemplate = \"nav.html\"\n+++\nHome\n",
        )
        .unwrap();
        std::fs::write(
            content_dir.join("about.md"),
            "+++\ntitle = \"About\"\norder = 1\n+++\nAbout\n",
        )
        .unwrap();
        std::fs::write(
            about_dir.join("team.md"),
            "+++\ntitle = \"Team\"\norder = 1\n+++\nTeam\n",
        )
        .unwrap();
        std::fs::write(
            about_dir.join("history.md"),
            "+++\ntitle = \"History\"\norder = 2\n+++\nHistory\n",
        )
        .unwrap();

        let tmpl_dir = site_root.join("templates");
        std::fs::create_dir_all(&tmpl_dir).unwrap();
        std::fs::write(tmpl_dir.join("page.html"), "{{ title }}").unwrap();
        std::fs::write(
            tmpl_dir.join("nav.html"),
            r#"{% set nav = tree(sort="order") %}{% for section in nav %}[{{ section.title }}{% for child in section.children %}|{{ child.title }}{% endfor %}]{% endfor %}"#,
        )
        .unwrap();

        let config = SiteConfig::from_file(&site_root.join("geoff.toml")).unwrap();
        let store = Arc::new(ContentStore::new().unwrap());
        let mut renderer =
            crate::renderer::SiteRenderer::new(&site_root.join(&config.template_dir)).unwrap();
        renderer.register_sparql_function(store.clone());

        let pages = build_site(site_root, &config, &store, &renderer).unwrap();
        let nav_page = pages
            .iter()
            .find(|p| p.output_path == "index.html")
            .unwrap();
        assert!(
            nav_page.html.contains("[About|Team|History]")
                || nav_page.html.contains("[About|History|Team]"),
            "tree() should build hierarchical navigation, got: {}",
            nav_page.html
        );
    }

    #[test]
    fn insert_data_triples_resolves_friendly_names() {
        let store = ContentStore::new().unwrap();
        let page_uri = PageUri::from_path("test.md");
        let graph_name = page_uri.as_str();

        let mut registry = MappingRegistry::new();
        registry.add_property("wordCount", "https://schema.org/wordCount");

        let mut data_fields = HashMap::new();
        // "wordCount" should resolve via registry.resolve_property()
        data_fields.insert("wordCount".to_string(), serde_json::json!(1500));
        // "schema:author" should resolve via registry.expand_iri()
        data_fields.insert(
            "schema:author".to_string(),
            JsonValue::String("Alice".into()),
        );
        // "customField" has no mapping — should fall back to urn:geoff:meta:customField
        data_fields.insert(
            "customField".to_string(),
            JsonValue::String("some value".into()),
        );

        insert_data_triples(&store, &page_uri, graph_name, &data_fields, &registry).unwrap();

        // Verify wordCount resolved via property mapping
        let results = store
            .query_to_json(
                "SELECT ?val WHERE { GRAPH ?g { ?s <https://schema.org/wordCount> ?val } }",
            )
            .unwrap();
        let arr = results.as_array().unwrap();
        assert_eq!(arr.len(), 1, "wordCount should resolve to schema:wordCount");

        // Verify schema:author resolved via expand_iri
        let results = store
            .query_to_json("SELECT ?val WHERE { GRAPH ?g { ?s <https://schema.org/author> ?val } }")
            .unwrap();
        let arr = results.as_array().unwrap();
        assert_eq!(arr.len(), 1, "schema:author should expand to full IRI");
        assert_eq!(arr[0]["val"], "Alice");

        // Verify unmapped key falls back to urn:geoff:meta:{key}
        let results = store
            .query_to_json(
                "SELECT ?val WHERE { GRAPH ?g { ?s <urn:geoff:meta:customField> ?val } }",
            )
            .unwrap();
        let arr = results.as_array().unwrap();
        assert_eq!(
            arr.len(),
            1,
            "unmapped key should use urn:geoff:meta: fallback"
        );
        assert_eq!(arr[0]["val"], "some value");
    }

    #[test]
    fn build_site_ingests_data_fields() {
        let dir = tempfile::tempdir().unwrap();
        let site_root = camino::Utf8Path::from_path(dir.path()).unwrap();

        // Create geoff.toml
        std::fs::write(
            site_root.join("geoff.toml"),
            "base_url = \"https://example.com\"\ntitle = \"Test\"\n",
        )
        .unwrap();

        // Create ontology/mappings.toml with a property mapping
        let ontology_dir = site_root.join("ontology");
        std::fs::create_dir_all(&ontology_dir).unwrap();
        std::fs::write(
            ontology_dir.join("mappings.toml"),
            r#"
[properties]
wordCount = "https://schema.org/wordCount"
"#,
        )
        .unwrap();

        // Create content with [data] section
        let content_dir = site_root.join("content");
        std::fs::create_dir_all(&content_dir).unwrap();
        std::fs::write(
            content_dir.join("article.md"),
            r#"+++
title = "My Article"
template = "page.html"

[data]
wordCount = 2500
difficulty = "intermediate"
+++

# My Article
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
        let mut renderer =
            crate::renderer::SiteRenderer::new(&site_root.join(&config.template_dir)).unwrap();
        renderer.register_sparql_function(store.clone());

        let pages = build_site(site_root, &config, &store, &renderer).unwrap();
        assert_eq!(pages.len(), 1);

        // Verify wordCount resolved via property mapping to schema:wordCount
        let results = store
            .query_to_json(
                "SELECT ?val WHERE { GRAPH ?g { ?s <https://schema.org/wordCount> ?val } }",
            )
            .unwrap();
        let arr = results.as_array().unwrap();
        assert_eq!(
            arr.len(),
            1,
            "wordCount should be mapped to schema:wordCount"
        );

        // Verify unmapped key falls back to urn:geoff:meta:difficulty
        let results = store
            .query_to_json("SELECT ?val WHERE { GRAPH ?g { ?s <urn:geoff:meta:difficulty> ?val } }")
            .unwrap();
        let arr = results.as_array().unwrap();
        assert_eq!(
            arr.len(),
            1,
            "difficulty should fall back to urn:geoff:meta:"
        );
        assert_eq!(arr[0]["val"], "intermediate");
    }

    #[test]
    fn rdfa_attrs_available_in_templates() {
        let dir = tempfile::tempdir().unwrap();
        let site_root = camino::Utf8Path::from_path(dir.path()).unwrap();

        std::fs::write(
            site_root.join("geoff.toml"),
            "base_url = \"https://example.com\"\ntitle = \"Test\"\n",
        )
        .unwrap();

        let content_dir = site_root.join("content");
        std::fs::create_dir_all(&content_dir).unwrap();
        std::fs::write(
            content_dir.join("about.md"),
            "+++\ntitle = \"About\"\ntype = \"Web Page\"\n+++\nAbout\n",
        )
        .unwrap();

        let tmpl_dir = site_root.join("templates");
        std::fs::create_dir_all(&tmpl_dir).unwrap();
        std::fs::write(
            tmpl_dir.join("page.html"),
            "<article {{ rdfa_attrs | safe }}>{{ content | safe }}</article>",
        )
        .unwrap();

        let config = SiteConfig::from_file(&site_root.join("geoff.toml")).unwrap();
        let store = Arc::new(ContentStore::new().unwrap());
        let mut renderer =
            crate::renderer::SiteRenderer::new(&site_root.join(&config.template_dir)).unwrap();
        renderer.register_sparql_function(store.clone());

        let pages = build_site(site_root, &config, &store, &renderer).unwrap();
        assert_eq!(pages.len(), 1);
        let html = &pages[0].html;
        assert!(
            html.contains("vocab=\"https://schema.org/\""),
            "rdfa_attrs should include vocab, got: {html}"
        );
        assert!(
            html.contains("typeof=\"WebPage\""),
            "rdfa_attrs should include typeof, got: {html}"
        );
        assert!(
            html.contains("resource=\"/about.html\""),
            "rdfa_attrs should include resource, got: {html}"
        );
    }

    #[test]
    fn critical_css_inlined_per_template() {
        let dir = tempfile::tempdir().unwrap();
        let site_root = camino::Utf8Path::from_path(dir.path()).unwrap();

        std::fs::write(
            site_root.join("geoff.toml"),
            "base_url = \"https://example.com\"\ntitle = \"Test\"\n",
        )
        .unwrap();

        let content_dir = site_root.join("content");
        std::fs::create_dir_all(&content_dir).unwrap();
        std::fs::write(
            content_dir.join("about.md"),
            "+++\ntitle = \"About\"\ntemplate = \"page.html\"\n+++\nAbout\n",
        )
        .unwrap();
        std::fs::write(
            content_dir.join("post.md"),
            "+++\ntitle = \"Post\"\ntemplate = \"blog.html\"\n+++\nPost\n",
        )
        .unwrap();

        let tmpl_dir = site_root.join("templates");
        std::fs::create_dir_all(&tmpl_dir).unwrap();
        std::fs::write(
            tmpl_dir.join("page.html"),
            "<style>{{ critical_css | safe }}</style>{{ content | safe }}",
        )
        .unwrap();
        std::fs::write(
            tmpl_dir.join("blog.html"),
            "<style>{{ critical_css | safe }}</style>{{ content | safe }}",
        )
        .unwrap();

        // Global critical CSS
        let static_dir = site_root.join("static");
        std::fs::create_dir_all(&static_dir).unwrap();
        std::fs::write(static_dir.join("critical.css"), "body { margin: 0; }").unwrap();
        // Template-specific critical CSS
        std::fs::write(
            static_dir.join("critical-blog.css"),
            ".post { max-width: 48rem; }",
        )
        .unwrap();

        let config = SiteConfig::from_file(&site_root.join("geoff.toml")).unwrap();
        let store = Arc::new(ContentStore::new().unwrap());
        let mut renderer =
            crate::renderer::SiteRenderer::new(&site_root.join(&config.template_dir)).unwrap();
        renderer.register_sparql_function(store.clone());

        let pages = build_site(site_root, &config, &store, &renderer).unwrap();
        assert_eq!(pages.len(), 2);

        let about = pages
            .iter()
            .find(|p| p.output_path.contains("about"))
            .unwrap();
        let post = pages
            .iter()
            .find(|p| p.output_path.contains("post"))
            .unwrap();

        // Global critical CSS appears on both pages
        assert!(
            about.html.contains("body { margin: 0; }"),
            "global critical CSS should be in about page, got: {}",
            about.html
        );
        assert!(
            post.html.contains("body { margin: 0; }"),
            "global critical CSS should be in post page, got: {}",
            post.html
        );

        // Template-specific critical CSS only on matching template
        assert!(
            !about.html.contains(".post"),
            "blog-specific critical CSS should NOT be in about page"
        );
        assert!(
            post.html.contains(".post { max-width: 48rem; }"),
            "blog-specific critical CSS should be in post page, got: {}",
            post.html
        );
    }
}

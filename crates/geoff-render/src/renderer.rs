use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use camino::{Utf8Path, Utf8PathBuf};
use geoff_graph::store::ContentStore;
use geoff_ontology::mappings::MappingRegistry;
use tera::{Context, Tera};

use crate::ssr::SsrWorker;

/// Site renderer using Tera templates.
pub struct SiteRenderer {
    tera: Tera,
    page_index: Arc<RwLock<Vec<serde_json::Value>>>,
}

impl SiteRenderer {
    /// Create a renderer loading templates from the given directory.
    pub fn new(template_dir: &Utf8Path) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let glob = format!("{}/**/*", template_dir);
        let mut tera = Tera::new(&glob)?;
        tera.autoescape_on(vec![]);
        let page_index = Self::register_page_functions(&mut tera);
        Ok(Self { tera, page_index })
    }

    /// Create a renderer with layered template directories.
    /// Templates are loaded in order (first directory wins for name conflicts):
    /// 1. `themes/{name}/templates/` (theme-specific)
    /// 2. `themes/{base}/templates/` (base theme, if set)
    /// 3. `templates/` (site default)
    pub fn with_theme_dirs(
        template_dirs: &[&Utf8Path],
    ) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let mut tera = Tera::default();
        tera.autoescape_on(vec![]);

        // Load in reverse order so that higher-priority directories override
        // lower-priority ones (last loaded wins in Tera).
        for dir in template_dirs.iter().rev() {
            if !dir.exists() {
                continue;
            }
            let glob = format!("{}/**/*", dir);
            if let Ok(override_tera) = Tera::new(&glob) {
                tera.extend(&override_tera)?;
            }
        }

        let page_index = Self::register_page_functions(&mut tera);
        Ok(Self { tera, page_index })
    }

    /// Register the `sparql()` template function backed by the given store.
    pub fn register_sparql_function(&mut self, store: Arc<ContentStore>) {
        self.tera
            .register_function("sparql", SparqlFunction { store });
    }

    /// Register the `theme_css()` template function that returns generated CSS.
    ///
    /// - `{{ theme_css() }}` returns deferred (non-critical) CSS variables
    /// - `{{ theme_css(critical=true) }}` returns only critical CSS variables
    pub fn register_theme_function(&mut self, css: String, critical_css: String) {
        self.tera
            .register_function("theme_css", ThemeCssFunction { css, critical_css });
    }

    /// Register the `devspaces_url()` template function that returns the Dev Spaces
    /// factory URL for the "Edit Source Code" button.
    ///
    /// Returns the combined URL `{url}#{git_repo}`, or an empty string if not configured.
    pub fn register_devspaces_function(&mut self, config: &geoff_core::config::DevSpacesConfig) {
        let url = match (&config.url, &config.git_repo) {
            (Some(url), Some(repo)) => format!("{}#{}", url, repo),
            _ => String::new(),
        };
        self.tera
            .register_function("devspaces_url", DevSpacesUrlFunction { url });
    }

    /// Register the `component()` template function for server-side rendering
    /// of web components via a Deno SSR worker.
    ///
    /// The worker is lazy-spawned on the first `{{ component() }}` call.
    /// If the worker script is missing or Deno is not installed, rendering
    /// falls back to emitting the custom element tag without shadow DOM content.
    pub fn register_component_function(&mut self, components_dir: Utf8PathBuf) {
        let worker = Arc::new(Mutex::new(None));
        self.tera.register_function(
            "component",
            ComponentFunction {
                worker,
                components_dir,
            },
        );
    }

    /// Populate the page index used by `pages()` and `tree()` template functions.
    /// Call this after ingesting content and before rendering pages.
    pub fn set_page_index(&self, index: Vec<serde_json::Value>) {
        *self.page_index.write().unwrap() = index;
    }

    fn register_page_functions(tera: &mut Tera) -> Arc<RwLock<Vec<serde_json::Value>>> {
        let page_index = Arc::new(RwLock::new(Vec::new()));
        tera.register_function("pages", PagesFunction { index: page_index.clone() });
        tera.register_function("tree", TreeFunction { index: page_index.clone() });
        page_index
    }

    /// Register RDFa template helpers: `rdfa_prefix()`, `rdfa_prop()`, `rdfa_meta()` functions
    /// and `rdfa` filter. These resolve friendly property names to IRIs via the mapping registry.
    pub fn register_rdfa_functions(
        &mut self,
        store: Arc<ContentStore>,
        registry: Arc<MappingRegistry>,
    ) {
        self.tera.register_function(
            "rdfa_prefix",
            RdfaPrefixFunction {
                registry: registry.clone(),
            },
        );
        self.tera.register_function(
            "rdfa_prop",
            RdfaPropFunction {
                registry: registry.clone(),
            },
        );
        self.tera.register_function(
            "rdfa_meta",
            RdfaMetaFunction {
                store,
                registry: registry.clone(),
            },
        );
        self.tera.register_filter("rdfa", RdfaFilter { registry });
    }

    /// Render a page with a pre-built Tera context.
    pub fn render_with_context(
        &self,
        template_name: &str,
        ctx: &Context,
    ) -> std::result::Result<String, Box<dyn std::error::Error>> {
        let rendered = self.tera.render(template_name, ctx)?;
        Ok(rendered)
    }
}

/// Tera function that executes SPARQL queries against the site graph.
struct SparqlFunction {
    store: Arc<ContentStore>,
}

impl tera::Function for SparqlFunction {
    fn call(
        &self,
        args: &std::collections::HashMap<String, tera::Value>,
    ) -> tera::Result<tera::Value> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tera::Error::msg("sparql() requires a 'query' argument"))?;

        let result = self
            .store
            .query_to_json(query)
            .map_err(|e| tera::Error::msg(format!("SPARQL query error: {e}")))?;

        Ok(result)
    }

    fn is_safe(&self) -> bool {
        true
    }
}

/// Tera function that returns generated CSS custom properties from design tokens.
struct ThemeCssFunction {
    css: String,
    critical_css: String,
}

impl tera::Function for ThemeCssFunction {
    fn call(
        &self,
        args: &std::collections::HashMap<String, tera::Value>,
    ) -> tera::Result<tera::Value> {
        let critical = args
            .get("critical")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let output = if critical {
            &self.critical_css
        } else {
            &self.css
        };

        Ok(tera::Value::String(output.clone()))
    }

    fn is_safe(&self) -> bool {
        true
    }
}

/// Tera function that returns the Dev Spaces factory URL for "Edit Source Code".
struct DevSpacesUrlFunction {
    url: String,
}

impl tera::Function for DevSpacesUrlFunction {
    fn call(
        &self,
        _args: &std::collections::HashMap<String, tera::Value>,
    ) -> tera::Result<tera::Value> {
        Ok(tera::Value::String(self.url.clone()))
    }

    fn is_safe(&self) -> bool {
        true
    }
}

/// Tera function that renders a web component via the SSR worker.
///
/// Usage in templates:
/// ```tera
/// {{ component(name="my-element", title="Hello") }}
/// {{ component(name="my-element", slot_content="<p>child content</p>") }}
/// {{ component(name="my-element", no_ssr=true) }}
/// ```
struct ComponentFunction {
    worker: Arc<Mutex<Option<SsrWorker>>>,
    components_dir: Utf8PathBuf,
}

impl ComponentFunction {
    fn render_tag_only(&self, name: &str, args: &HashMap<String, tera::Value>) -> String {
        let mut attributes = HashMap::new();
        for (k, v) in args {
            if k == "name" || k == "slot_content" || k == "no_ssr" {
                continue;
            }
            if let Some(s) = v.as_str() {
                attributes.insert(k.clone(), s.to_string());
            }
        }
        let attrs = self.build_attrs_str(&attributes);
        let children = args
            .get("slot_content")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        format!("<{name}{attrs}>{children}</{name}>")
    }

    fn build_attrs_str(&self, attrs: &HashMap<String, String>) -> String {
        if attrs.is_empty() {
            return String::new();
        }
        let mut s = String::new();
        for (k, v) in attrs {
            s.push(' ');
            s.push_str(k);
            s.push_str("=\"");
            s.push_str(&v.replace('"', "&quot;"));
            s.push('"');
        }
        s
    }
}

impl tera::Function for ComponentFunction {
    fn call(&self, args: &HashMap<String, tera::Value>) -> tera::Result<tera::Value> {
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tera::Error::msg("component() requires 'name' parameter"))?;

        // Find the component JS file
        let script_path = self.components_dir.join(name).join(format!("{name}.js"));
        let script_fallback = self.components_dir.join(format!("{name}.js"));
        let actual_path = if script_path.exists() {
            script_path
        } else if script_fallback.exists() {
            script_fallback
        } else {
            // No component found -- render just the tag with attributes
            return Ok(tera::Value::String(self.render_tag_only(name, args)));
        };

        // Extract renderer option
        let renderer = args
            .get("renderer")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Build attributes map (everything except reserved params)
        let mut attributes = HashMap::new();
        for (k, v) in args {
            if k == "name" || k == "slot_content" || k == "no_ssr" || k == "renderer" {
                continue;
            }
            if let Some(s) = v.as_str() {
                attributes.insert(k.clone(), s.to_string());
            }
        }

        // Check for no_ssr flag
        if let Some(no_ssr) = args.get("no_ssr")
            && no_ssr.as_bool().unwrap_or(false)
        {
            return Ok(tera::Value::String(self.render_tag_only(name, args)));
        }

        let children = args
            .get("slot_content")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Try to render via SSR worker
        let mut worker_guard = self.worker.lock().unwrap();
        if worker_guard.is_none() {
            // Lazy-spawn the worker
            let worker_script = self.components_dir.join("ssr-worker.ts");
            if worker_script.exists() {
                match SsrWorker::spawn(&worker_script) {
                    Ok(w) => *worker_guard = Some(w),
                    Err(e) => {
                        eprintln!("SSR worker spawn failed: {e}");
                        return Ok(tera::Value::String(self.render_tag_only(name, args)));
                    }
                }
            } else {
                return Ok(tera::Value::String(self.render_tag_only(name, args)));
            }
        }

        let worker = worker_guard.as_mut().unwrap();
        let abs_path = std::fs::canonicalize(actual_path.as_std_path())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| actual_path.to_string());

        match worker.render_component(&abs_path, name, &attributes, children, renderer.as_deref()) {
            Ok(result) => {
                // Lit SSR returns full element HTML with DSD already included
                if renderer.as_deref() == Some("lit") {
                    Ok(tera::Value::String(result.html))
                } else {
                    let attrs_str = self.build_attrs_str(&attributes);
                    if result.has_shadow_root {
                        Ok(tera::Value::String(format!(
                            "<{name}{attrs_str}>\n  <template shadowrootmode=\"open\">\n{}\n  </template>\n{children}</{name}>",
                            result.html
                        )))
                    } else {
                        Ok(tera::Value::String(format!(
                            "<{name}{attrs_str}>{}{children}</{name}>",
                            result.html
                        )))
                    }
                }
            }
            Err(e) => {
                eprintln!("SSR render failed for {name}: {e}");
                Ok(tera::Value::String(self.render_tag_only(name, args)))
            }
        }
    }

    fn is_safe(&self) -> bool {
        true
    }
}

/// Metadata for building a page's template context.
pub struct PageContext<'a> {
    pub title: &'a str,
    pub content_html: &'a str,
    pub json_ld: &'a str,
    pub site_title: &'a str,
    pub page_url: &'a str,
    pub page_uri: &'a str,
    pub rdfa_attrs: &'a str,
    pub date: Option<&'a str>,
    pub author: Option<&'a str>,
    pub description: Option<&'a str>,
    pub tags: Option<&'a [String]>,
}

/// Build a Tera context for a page from its metadata.
pub fn build_page_context(page: &PageContext<'_>) -> Context {
    let mut ctx = Context::new();
    ctx.insert("title", page.title);
    ctx.insert("content", page.content_html);
    ctx.insert("json_ld", page.json_ld);
    ctx.insert("page_url", page.page_url);
    ctx.insert("page_uri", page.page_uri);
    ctx.insert("rdfa_attrs", page.rdfa_attrs);

    let mut config = std::collections::HashMap::new();
    config.insert("title", page.site_title);
    ctx.insert("config", &config);

    if let Some(d) = page.date {
        ctx.insert("date", d);
    }
    if let Some(a) = page.author {
        ctx.insert("author", a);
    }
    if let Some(desc) = page.description {
        ctx.insert("description", desc);
    }
    if let Some(t) = page.tags {
        ctx.insert("tags", t);
    }
    ctx
}

/// Tera function that returns `prefix="schema: https://schema.org/ ..."` for the `<html>` tag.
struct RdfaPrefixFunction {
    registry: Arc<MappingRegistry>,
}

impl tera::Function for RdfaPrefixFunction {
    fn call(
        &self,
        _args: &HashMap<String, tera::Value>,
    ) -> tera::Result<tera::Value> {
        let mut parts = Vec::new();
        for (prefix, namespace) in self.registry.all_prefixes() {
            if prefix == "geoff" || prefix == "rdf" || prefix == "rdfs" {
                continue;
            }
            parts.push(format!("{prefix}: {namespace}"));
        }
        Ok(tera::Value::String(format!("prefix=\"{}\"", parts.join(" "))))
    }

    fn is_safe(&self) -> bool {
        true
    }
}

/// Tera function that returns `property="schema:name"` for a friendly property name.
struct RdfaPropFunction {
    registry: Arc<MappingRegistry>,
}

impl tera::Function for RdfaPropFunction {
    fn call(
        &self,
        args: &HashMap<String, tera::Value>,
    ) -> tera::Result<tera::Value> {
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tera::Error::msg("rdfa_prop() requires 'name' parameter"))?;

        let resolved = resolve_property_name(name, &self.registry);
        Ok(tera::Value::String(format!("property=\"{resolved}\"")))
    }

    fn is_safe(&self) -> bool {
        true
    }
}

/// Tera function that returns hidden `<meta>` tags for non-visible RDFa properties.
struct RdfaMetaFunction {
    store: Arc<ContentStore>,
    registry: Arc<MappingRegistry>,
}

impl tera::Function for RdfaMetaFunction {
    fn call(
        &self,
        args: &HashMap<String, tera::Value>,
    ) -> tera::Result<tera::Value> {
        let page_uri = args
            .get("page_uri")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if page_uri.is_empty() {
            return Ok(tera::Value::String(String::new()));
        }

        let skip_props = [
            "https://schema.org/name",
            "https://schema.org/url",
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
        ];

        let query = format!(
            "SELECT ?p ?o WHERE {{ GRAPH <{page_uri}> {{ <{page_uri}> ?p ?o }} }}"
        );

        let mut metas = Vec::new();
        if let Ok(results) = self.store.query_to_json(&query) {
            if let Some(rows) = results.as_array() {
                for row in rows {
                    let pred = match row.get("p").and_then(|v| v.as_str()) {
                        Some(p) => p.trim_start_matches('<').trim_end_matches('>'),
                        None => continue,
                    };
                    let obj = match row.get("o").and_then(|v| v.as_str()) {
                        Some(o) => o,
                        None => continue,
                    };

                    if pred.starts_with("urn:geoff:") || skip_props.contains(&pred) {
                        continue;
                    }

                    let prop = self.registry.compact_iri(pred);
                    let val = obj.replace('"', "&quot;");
                    metas.push(format!("<meta property=\"{prop}\" content=\"{val}\">"));
                }
            }
        }

        Ok(tera::Value::String(metas.join("\n")))
    }

    fn is_safe(&self) -> bool {
        true
    }
}

/// Tera filter: `{{ value | rdfa(prop="author") }}` → `<span property="schema:author">value</span>`
struct RdfaFilter {
    registry: Arc<MappingRegistry>,
}

impl tera::Filter for RdfaFilter {
    fn filter(
        &self,
        value: &tera::Value,
        args: &HashMap<String, tera::Value>,
    ) -> tera::Result<tera::Value> {
        let prop_name = args
            .get("prop")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tera::Error::msg("rdfa filter requires 'prop' parameter"))?;

        let resolved = resolve_property_name(prop_name, &self.registry);
        let text = value.as_str().unwrap_or("");
        Ok(tera::Value::String(format!(
            "<span property=\"{resolved}\">{text}</span>"
        )))
    }

    fn is_safe(&self) -> bool {
        true
    }
}

fn resolve_property_name(name: &str, registry: &MappingRegistry) -> String {
    if let Some(iri) = registry.resolve_property(name) {
        return registry.compact_iri(iri);
    }
    if let Some(expanded) = registry.expand_iri(name) {
        return registry.compact_iri(&expanded);
    }
    name.to_string()
}

/// Tera function that queries the page index with optional filtering and sorting.
///
/// Parameters:
/// - `section`: Filter by URL prefix (e.g. `section="about"` matches `/about/...`)
/// - `sort`: Sort results by this field name
/// - `reverse`: Reverse the sort order (default: false)
/// - Any other parameter filters pages where that field equals the given value
struct PagesFunction {
    index: Arc<RwLock<Vec<serde_json::Value>>>,
}

impl tera::Function for PagesFunction {
    fn call(&self, args: &HashMap<String, tera::Value>) -> tera::Result<tera::Value> {
        let index = self
            .index
            .read()
            .map_err(|e| tera::Error::msg(format!("page index lock: {e}")))?;

        let mut results: Vec<serde_json::Value> = index.clone();

        if let Some(section) = args.get("section").and_then(|v| v.as_str()) {
            let prefix = if section.starts_with('/') {
                section.to_string()
            } else {
                format!("/{section}")
            };
            results.retain(|p| {
                p.get("url")
                    .and_then(|u| u.as_str())
                    .map(|u| u.starts_with(&prefix))
                    .unwrap_or(false)
            });
        }

        let control_keys = ["section", "sort", "reverse"];
        for (key, val) in args {
            if control_keys.contains(&key.as_str()) {
                continue;
            }
            results.retain(|p| p.get(key).map(|v| v == val).unwrap_or(false));
        }

        if let Some(sort_key) = args.get("sort").and_then(|v| v.as_str()) {
            results.sort_by(|a, b| compare_json_values(a.get(sort_key), b.get(sort_key)));
        }

        if args
            .get("reverse")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            results.reverse();
        }

        Ok(serde_json::Value::Array(results))
    }

    fn is_safe(&self) -> bool {
        true
    }
}

/// Tera function that builds a navigation tree from the page index.
///
/// Parameters:
/// - `root`: Root section path (default: "/" for entire site)
/// - `sort`: Sort children by this field (default: alphabetical by title)
/// - `depth`: Maximum tree depth (default: unlimited)
struct TreeFunction {
    index: Arc<RwLock<Vec<serde_json::Value>>>,
}

impl tera::Function for TreeFunction {
    fn call(&self, args: &HashMap<String, tera::Value>) -> tera::Result<tera::Value> {
        let index = self
            .index
            .read()
            .map_err(|e| tera::Error::msg(format!("page index lock: {e}")))?;
        let root = args
            .get("root")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let sort_key = args.get("sort").and_then(|v| v.as_str());
        let depth = args
            .get("depth")
            .and_then(|v| v.as_u64())
            .map(|d| d as usize);

        let tree = build_nav_tree(&index, root, sort_key, depth);
        Ok(serde_json::Value::Array(tree))
    }

    fn is_safe(&self) -> bool {
        true
    }
}

fn compare_json_values(
    a: Option<&serde_json::Value>,
    b: Option<&serde_json::Value>,
) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(a), Some(b)) => {
            if let (Some(na), Some(nb)) = (a.as_f64(), b.as_f64()) {
                na.partial_cmp(&nb).unwrap_or(Ordering::Equal)
            } else {
                let sa = a.as_str().unwrap_or("");
                let sb = b.as_str().unwrap_or("");
                sa.cmp(sb)
            }
        }
    }
}

fn normalize_url_key(url: &str) -> String {
    let s = url.trim_end_matches('/');
    let s = s.strip_suffix(".html").unwrap_or(s);
    if s.is_empty() {
        "/".to_string()
    } else {
        s.to_string()
    }
}

fn parent_url_key(key: &str) -> Option<String> {
    if key == "/" {
        return None;
    }
    match key.rfind('/') {
        Some(0) | None => Some("/".to_string()),
        Some(pos) => Some(key[..pos].to_string()),
    }
}

fn titlecase_segment(s: &str) -> String {
    s.split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn build_nav_tree(
    pages: &[serde_json::Value],
    root: &str,
    sort_key: Option<&str>,
    max_depth: Option<usize>,
) -> Vec<serde_json::Value> {
    use std::collections::BTreeMap;

    let mut page_map: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    for page in pages {
        if let Some(url) = page.get("url").and_then(|u| u.as_str()) {
            let key = normalize_url_key(url);
            page_map.insert(key, page.clone());
        }
    }

    let root_key = if root.is_empty() || root == "/" {
        "/".to_string()
    } else {
        let r = root.trim_start_matches('/').trim_end_matches('/');
        format!("/{r}")
    };

    // Create synthetic nodes for missing intermediate directories
    let keys: Vec<String> = page_map.keys().cloned().collect();
    for key in &keys {
        let mut current = key.clone();
        while let Some(parent) = parent_url_key(&current) {
            if page_map.contains_key(&parent) || parent == root_key {
                break;
            }
            let parent_segment = parent
                .rsplit('/')
                .find(|s| !s.is_empty())
                .unwrap_or("")
                .to_string();
            let title = titlecase_segment(&parent_segment);
            page_map.entry(parent.clone()).or_insert_with(|| {
                serde_json::json!({
                    "url": if parent == "/" { "/".to_string() } else { format!("{parent}/") },
                    "title": title,
                    "synthetic": true
                })
            });
            current = parent;
        }
    }

    // Build parent → children mapping
    let all_keys: Vec<String> = page_map.keys().cloned().collect();
    let mut children_of: HashMap<String, Vec<String>> = HashMap::new();

    for key in &all_keys {
        if *key == root_key {
            continue;
        }
        if let Some(parent) = parent_url_key(key) {
            let under_root = if root_key == "/" {
                true
            } else {
                parent == root_key || parent.starts_with(&format!("{root_key}/"))
            };
            if under_root {
                children_of.entry(parent).or_default().push(key.clone());
            }
        }
    }

    fn make_node(
        key: &str,
        depth: usize,
        page_map: &BTreeMap<String, serde_json::Value>,
        children_of: &HashMap<String, Vec<String>>,
        sort_key: Option<&str>,
        max_depth: Option<usize>,
    ) -> serde_json::Value {
        let mut node = page_map
            .get(key)
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));

        let child_nodes = if max_depth.is_some_and(|d| depth >= d) {
            Vec::new()
        } else {
            let mut nodes: Vec<serde_json::Value> = children_of
                .get(key)
                .map(|kids| {
                    kids.iter()
                        .map(|k| {
                            make_node(k, depth + 1, page_map, children_of, sort_key, max_depth)
                        })
                        .collect()
                })
                .unwrap_or_default();

            sort_nodes(&mut nodes, sort_key);
            nodes
        };

        if let Some(obj) = node.as_object_mut() {
            obj.insert(
                "children".to_string(),
                serde_json::Value::Array(child_nodes),
            );
        }

        node
    }

    fn sort_nodes(nodes: &mut [serde_json::Value], sort_key: Option<&str>) {
        if let Some(sk) = sort_key {
            nodes.sort_by(|a, b| compare_json_values(a.get(sk), b.get(sk)));
        } else {
            nodes.sort_by(|a, b| {
                let ta = a.get("title").and_then(|v| v.as_str()).unwrap_or("");
                let tb = b.get("title").and_then(|v| v.as_str()).unwrap_or("");
                ta.cmp(tb)
            });
        }
    }

    let mut result: Vec<serde_json::Value> = children_of
        .get(&root_key)
        .map(|kids| {
            kids.iter()
                .map(|k| make_node(k, 1, &page_map, &children_of, sort_key, max_depth))
                .collect()
        })
        .unwrap_or_default();

    sort_nodes(&mut result, sort_key);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use geoff_core::types::ObjectValue;

    #[test]
    fn render_simple_template() {
        let dir = tempfile::tempdir().unwrap();
        let tmpl_path = dir.path().join("page.html");
        std::fs::write(
            &tmpl_path,
            "<h1>{{ title }}</h1>\n{{ content }}\n{{ json_ld }}",
        )
        .unwrap();

        let utf8_dir = Utf8Path::from_path(dir.path()).unwrap();
        let renderer = SiteRenderer::new(utf8_dir).unwrap();
        let ctx = build_page_context(&PageContext {
            title: "Test",
            content_html: "<p>Hello</p>",
            json_ld: "{\"@type\": \"WebPage\"}",
            site_title: "My Site",
            page_url: "/test.html",
            page_uri: "urn:geoff:content:test.md",
            rdfa_attrs: "vocab=\"https://schema.org/\" typeof=\"WebPage\"",
            date: None,
            author: None,
            description: None,
            tags: None,
        });
        let result = renderer.render_with_context("page.html", &ctx).unwrap();

        assert!(result.contains("<h1>Test</h1>"));
        assert!(result.contains("<p>Hello</p>"));
        assert!(result.contains("{\"@type\": \"WebPage\"}"));
    }

    #[test]
    fn sparql_template_function() {
        let store = Arc::new(ContentStore::new().unwrap());
        store
            .insert_triple_into(
                "urn:geoff:content:blog/test.md",
                "https://schema.org/name",
                &ObjectValue::Literal("Test Post".into()),
                "urn:geoff:content:blog/test.md",
            )
            .unwrap();

        let dir = tempfile::tempdir().unwrap();
        let tmpl_path = dir.path().join("sparql.html");
        std::fs::write(
            &tmpl_path,
            r#"{% set results = sparql(query="SELECT ?title WHERE { GRAPH ?g { ?s <https://schema.org/name> ?title } }") %}{% for row in results %}{{ row.title }}{% endfor %}"#,
        )
        .unwrap();

        let utf8_dir = Utf8Path::from_path(dir.path()).unwrap();
        let mut renderer = SiteRenderer::new(utf8_dir).unwrap();
        renderer.register_sparql_function(store);

        let ctx = Context::new();
        let result = renderer.render_with_context("sparql.html", &ctx).unwrap();
        assert!(
            result.contains("Test Post"),
            "sparql() should return query results usable in templates, got: {result}"
        );
    }

    #[test]
    fn sparql_function_invalid_query_returns_error() {
        let store = Arc::new(ContentStore::new().unwrap());
        let dir = tempfile::tempdir().unwrap();
        let tmpl_path = dir.path().join("bad.html");
        std::fs::write(&tmpl_path, r#"{{ sparql(query="INVALID SPARQL") }}"#).unwrap();

        let utf8_dir = Utf8Path::from_path(dir.path()).unwrap();
        let mut renderer = SiteRenderer::new(utf8_dir).unwrap();
        renderer.register_sparql_function(store);

        let ctx = Context::new();
        let result = renderer.render_with_context("bad.html", &ctx);
        assert!(result.is_err(), "Invalid SPARQL should produce an error");
    }
}

use std::sync::Arc;

use camino::Utf8Path;
use geoff_graph::store::ContentStore;
use tera::{Context, Tera};

/// Site renderer using Tera templates.
pub struct SiteRenderer {
    tera: Tera,
}

impl SiteRenderer {
    /// Create a renderer loading templates from the given directory.
    pub fn new(template_dir: &Utf8Path) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let glob = format!("{}/**/*", template_dir);
        let mut tera = Tera::new(&glob)?;
        tera.autoescape_on(vec![]);
        Ok(Self { tera })
    }

    /// Register the `sparql()` template function backed by the given store.
    pub fn register_sparql_function(&mut self, store: Arc<ContentStore>) {
        self.tera
            .register_function("sparql", SparqlFunction { store });
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

/// Metadata for building a page's template context.
pub struct PageContext<'a> {
    pub title: &'a str,
    pub content_html: &'a str,
    pub json_ld: &'a str,
    pub site_title: &'a str,
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
                "http://schema.org/name",
                &ObjectValue::Literal("Test Post".into()),
                "urn:geoff:content:blog/test.md",
            )
            .unwrap();

        let dir = tempfile::tempdir().unwrap();
        let tmpl_path = dir.path().join("sparql.html");
        std::fs::write(
            &tmpl_path,
            r#"{% set results = sparql(query="SELECT ?title WHERE { GRAPH ?g { ?s <http://schema.org/name> ?title } }") %}{% for row in results %}{{ row.title }}{% endfor %}"#,
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

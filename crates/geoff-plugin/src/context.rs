use std::collections::HashMap;

use geoff_core::config::SiteConfig;
use geoff_graph::store::ContentStore;
use serde::{Deserialize, Serialize};

/// Plugin-facing view of a page's parsed content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageData {
    /// Content-relative path (e.g. "blog/hello.md").
    pub path: String,
    /// Page title from frontmatter.
    pub title: Option<String>,
    /// Content type from frontmatter (e.g. "Blog Post").
    pub content_type: Option<String>,
    /// Rendered HTML body.
    pub html: String,
    /// Raw Markdown body.
    pub raw_body: String,
    /// All frontmatter fields.
    pub frontmatter: HashMap<String, serde_json::Value>,
}

/// Context passed to plugins during initialization.
pub struct InitContext<'a> {
    pub config: &'a SiteConfig,
    pub plugin_options: &'a HashMap<String, toml::Value>,
}

/// Context passed at the start of a build.
pub struct BuildContext<'a> {
    pub config: &'a SiteConfig,
    pub store: &'a ContentStore,
}

/// Context passed after a single content file is parsed.
pub struct ContentContext<'a> {
    pub config: &'a SiteConfig,
    pub page: &'a mut PageData,
}

/// Context passed after all content is ingested into the graph.
pub struct GraphContext<'a> {
    pub config: &'a SiteConfig,
    pub store: &'a ContentStore,
}

/// Context passed after SHACL validation completes.
pub struct ValidationContext<'a> {
    pub config: &'a SiteConfig,
    pub store: &'a ContentStore,
    pub conforms: bool,
    pub violations: usize,
}

/// Context passed before rendering a single page.
pub struct RenderContext<'a> {
    pub config: &'a SiteConfig,
    pub store: &'a ContentStore,
    pub page: &'a mut PageData,
    /// Extra template variables that plugins can inject.
    pub extra_vars: &'a mut HashMap<String, serde_json::Value>,
}

/// Context passed after all output files are written.
pub struct OutputContext<'a> {
    pub config: &'a SiteConfig,
    pub store: &'a ContentStore,
    /// Map of output path -> rendered HTML.
    pub outputs: &'a HashMap<String, String>,
    /// The output directory path.
    pub output_dir: &'a camino::Utf8Path,
}

/// Context passed when a file change is detected during `serve`.
pub struct WatchContext<'a> {
    pub config: &'a SiteConfig,
    pub changed_path: &'a str,
}

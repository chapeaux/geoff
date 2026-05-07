use std::collections::HashMap;

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

/// Plugin runtime type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginRuntime {
    Rust,
    Deno,
}

/// Configuration for a single plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub name: String,
    pub runtime: PluginRuntime,
    pub path: Utf8PathBuf,
    #[serde(default)]
    pub options: HashMap<String, toml::Value>,
}

/// Client-side search configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_search_output")]
    pub output: String,
    /// Partitioning strategy for faceted search graphs.
    /// Options: "type", "section", "type+section", "date-year", "date-month",
    /// or any frontmatter field name (resolved via the mapping registry).
    #[serde(default)]
    pub partition: Option<String>,
    /// Generate a built-in /search page (default: true when search is enabled).
    #[serde(default = "default_true")]
    pub page: bool,
    /// Title for the search page.
    #[serde(default = "default_search_title")]
    pub title: Option<String>,
    /// Custom template for the search page (overrides the built-in template).
    #[serde(default)]
    pub template: Option<String>,
    /// Maximum results per query.
    #[serde(default = "default_search_limit")]
    pub limit: u32,
    /// Placeholder text for the search input.
    #[serde(default = "default_search_placeholder")]
    pub placeholder: String,
}

fn default_search_title() -> Option<String> {
    Some("Search".to_string())
}

fn default_search_limit() -> u32 {
    50
}

fn default_search_placeholder() -> String {
    "Search…".to_string()
}

fn default_search_output() -> String {
    "search.nt".to_string()
}

/// Image optimization configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageOptConfig {
    /// Convert PNG/JPEG images to WebP format.
    #[serde(default)]
    pub webp: bool,
    /// WebP quality (0-100).
    #[serde(default = "default_image_quality")]
    pub quality: u8,
    /// Maximum width in pixels; wider images are resized proportionally.
    #[serde(default)]
    pub max_width: Option<u32>,
}

impl Default for ImageOptConfig {
    fn default() -> Self {
        Self {
            webp: false,
            quality: default_image_quality(),
            max_width: None,
        }
    }
}

fn default_image_quality() -> u8 {
    80
}

/// Asset optimization configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OptimizeConfig {
    /// Minify CSS files in the output directory using lightningcss.
    #[serde(default)]
    pub minify_css: bool,
    /// Minify JS files in the output directory (basic comment/whitespace stripping).
    #[serde(default)]
    pub minify_js: bool,
    /// Add content hashes to CSS/JS filenames and update HTML references.
    #[serde(default)]
    pub hash_assets: bool,
    /// Image optimization settings.
    #[serde(default)]
    pub images: ImageOptConfig,
}

/// Dark mode configuration for themes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ThemeModes {
    /// Path to dark mode token file (relative to theme directory).
    pub dark: Option<String>,
}

/// Theme configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ThemeConfig {
    /// Theme name — corresponds to a directory under `themes/`.
    #[serde(default)]
    pub name: Option<String>,
    /// Base theme to inherit from.
    #[serde(default)]
    pub base: Option<String>,
    /// Whether to share this theme (expose its tokens for child themes).
    #[serde(default)]
    pub share: bool,
    /// Color mode variants (e.g. dark mode).
    #[serde(default)]
    pub modes: ThemeModes,
    /// Prefix for all CSS custom property names (e.g. "rh" → --rh-color-primary).
    #[serde(default)]
    pub prefix: Option<String>,
    /// Asset optimization settings.
    #[serde(default)]
    pub optimize: OptimizeConfig,
}

fn default_true() -> bool {
    true
}

fn default_vocab() -> String {
    "https://schema.org/".to_string()
}

/// Linked data and RDFa configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedDataConfig {
    /// Enable RDFa template helpers (default: true).
    #[serde(default = "default_true")]
    pub rdfa: bool,
    /// Default vocabulary for RDFa output.
    #[serde(default = "default_vocab")]
    pub default_vocab: String,
    /// Include all graph data in JSON-LD, not just standard fields.
    #[serde(default = "default_true")]
    pub rich_jsonld: bool,
    /// Enable [text](rdfa:prop) Markdown link rewriting.
    #[serde(default = "default_true")]
    pub rdfa_links: bool,
    /// Additional vocabulary namespace prefixes (merged with built-in prefixes).
    #[serde(default)]
    pub prefixes: std::collections::HashMap<String, String>,
}

impl Default for LinkedDataConfig {
    fn default() -> Self {
        Self {
            rdfa: true,
            default_vocab: default_vocab(),
            rich_jsonld: true,
            rdfa_links: true,
            prefixes: std::collections::HashMap::new(),
        }
    }
}

/// MCP agent discovery configuration.
/// Generates a `.well-known/mcp.json` manifest and optionally bundles
/// a WASM SPARQL engine so AI agents can query the site's RDF graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    #[serde(default)]
    pub enabled: bool,
    /// WASM delivery: "cdn" (default) or "local" (bundled in dist/bin/).
    #[serde(default = "default_wasm_source")]
    pub wasm_source: String,
    /// Custom WASM URL (overrides cdn/local).
    #[serde(default)]
    pub wasm_url: Option<String>,
    /// Custom description for the manifest tool entry.
    #[serde(default)]
    pub description: Option<String>,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            wasm_source: default_wasm_source(),
            wasm_url: None,
            description: None,
        }
    }
}

fn default_wasm_source() -> String {
    "cdn".to_string()
}

/// Design system token configuration.
/// Points to external token files (e.g. from node_modules) that provide
/// the raw primitives a theme is built from.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DesignSystemConfig {
    #[serde(default)]
    pub tokens: Vec<String>,
}

/// Dev Spaces configuration for "Edit Source Code" button.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DevSpacesConfig {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub git_repo: Option<String>,
}

/// Site configuration loaded from `geoff.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfig {
    pub base_url: String,
    pub title: String,
    #[serde(default = "default_content_dir")]
    pub content_dir: Utf8PathBuf,
    #[serde(default = "default_output_dir")]
    pub output_dir: Utf8PathBuf,
    #[serde(default = "default_template_dir")]
    pub template_dir: Utf8PathBuf,
    #[serde(default, rename = "plugins")]
    pub plugins: Vec<PluginConfig>,
    #[serde(default)]
    pub search: SearchConfig,
    #[serde(default)]
    pub theme: ThemeConfig,
    #[serde(default)]
    pub devspaces: DevSpacesConfig,
    #[serde(default)]
    pub build: BuildConfig,
    #[serde(default)]
    pub linked_data: LinkedDataConfig,
    #[serde(default)]
    pub design: DesignSystemConfig,
    #[serde(default)]
    pub mcp: McpConfig,
}

/// URL style for generated pages.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UrlStyle {
    /// Output as `about.html` → URL `/about.html`
    #[default]
    File,
    /// Output as `about/index.html` → URL `/about/`
    Directory,
}

/// Build configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BuildConfig {
    /// How to structure output URLs.
    #[serde(default)]
    pub url_style: UrlStyle,
}

fn default_content_dir() -> Utf8PathBuf {
    Utf8PathBuf::from("content")
}

fn default_output_dir() -> Utf8PathBuf {
    Utf8PathBuf::from("dist")
}

fn default_template_dir() -> Utf8PathBuf {
    Utf8PathBuf::from("templates")
}

impl SiteConfig {
    /// Load site configuration from a TOML file.
    pub fn from_file(
        path: &camino::Utf8Path,
    ) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: SiteConfig = toml::from_str(&content)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_config() {
        let toml_str = r#"
            base_url = "https://example.com"
            title = "My Site"
        "#;
        let config: SiteConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.base_url, "https://example.com");
        assert_eq!(config.title, "My Site");
        assert_eq!(config.content_dir.as_str(), "content");
        assert_eq!(config.output_dir.as_str(), "dist");
        assert_eq!(config.template_dir.as_str(), "templates");
    }

    #[test]
    fn parse_full_config() {
        let toml_str = r#"
            base_url = "https://example.com"
            title = "My Site"
            content_dir = "src/content"
            output_dir = "public"
            template_dir = "layouts"
        "#;
        let config: SiteConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.content_dir.as_str(), "src/content");
        assert_eq!(config.output_dir.as_str(), "public");
        assert_eq!(config.template_dir.as_str(), "layouts");
    }

    #[test]
    fn parse_config_with_plugins() {
        let toml_str = r#"
            base_url = "https://example.com"
            title = "My Site"

            [[plugins]]
            name = "reading-time"
            runtime = "rust"
            path = "plugins/geoff-reading-time"

            [[plugins]]
            name = "sitemap"
            runtime = "deno"
            path = "plugins/sitemap.ts"
            [plugins.options]
            exclude = "/drafts/"
        "#;
        let config: SiteConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.plugins.len(), 2);
        assert_eq!(config.plugins[0].name, "reading-time");
        assert_eq!(config.plugins[0].runtime, PluginRuntime::Rust);
        assert_eq!(config.plugins[1].name, "sitemap");
        assert_eq!(config.plugins[1].runtime, PluginRuntime::Deno);
        assert!(config.plugins[1].options.contains_key("exclude"));
    }

    #[test]
    fn parse_config_no_plugins() {
        let toml_str = r#"
            base_url = "https://example.com"
            title = "My Site"
        "#;
        let config: SiteConfig = toml::from_str(toml_str).unwrap();
        assert!(config.plugins.is_empty());
    }

    #[test]
    fn parse_linked_data_config() {
        let toml_str = r#"
            base_url = "https://example.com"
            title = "My Site"

            [linked_data]
            rdfa = true
            default_vocab = "https://schema.org/"
            rich_jsonld = false
            rdfa_links = false

            [linked_data.prefixes]
            dc = "http://purl.org/dc/terms/"
            foaf = "http://xmlns.com/foaf/0.1/"
        "#;
        let config: SiteConfig = toml::from_str(toml_str).unwrap();
        assert!(config.linked_data.rdfa);
        assert_eq!(config.linked_data.default_vocab, "https://schema.org/");
        assert!(!config.linked_data.rich_jsonld);
        assert!(!config.linked_data.rdfa_links);
        assert_eq!(config.linked_data.prefixes.len(), 2);
        assert_eq!(
            config.linked_data.prefixes.get("dc").unwrap(),
            "http://purl.org/dc/terms/"
        );
        assert_eq!(
            config.linked_data.prefixes.get("foaf").unwrap(),
            "http://xmlns.com/foaf/0.1/"
        );
    }

    #[test]
    fn linked_data_defaults_when_omitted() {
        let toml_str = r#"
            base_url = "https://example.com"
            title = "My Site"
        "#;
        let config: SiteConfig = toml::from_str(toml_str).unwrap();
        assert!(config.linked_data.rdfa);
        assert_eq!(config.linked_data.default_vocab, "https://schema.org/");
        assert!(config.linked_data.rich_jsonld);
        assert!(config.linked_data.rdfa_links);
        assert!(config.linked_data.prefixes.is_empty());
    }
}

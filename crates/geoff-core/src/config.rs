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
}

fn default_search_output() -> String {
    "search.nt".to_string()
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
}

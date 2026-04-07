use std::collections::HashMap;

use camino::Utf8Path;
use serde::{Deserialize, Serialize};

/// Known namespace prefixes for compact IRI display.
const KNOWN_PREFIXES: &[(&str, &str)] = &[
    ("schema", "https://schema.org/"),
    ("dc", "http://purl.org/dc/terms/"),
    ("foaf", "http://xmlns.com/foaf/0.1/"),
    ("geoff", "urn:geoff:ontology:"),
    ("rdfs", "http://www.w3.org/2000/01/rdf-schema#"),
    ("rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#"),
];

/// Persisted mapping from user-friendly names to ontology IRIs.
///
/// Stored in `ontology/mappings.toml` so users never need to know IRIs.
/// Example:
/// ```toml
/// [types]
/// "Blog Post" = "https://schema.org/BlogPosting"
///
/// [properties]
/// "author" = "https://schema.org/author"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingRegistry {
    #[serde(default)]
    pub types: HashMap<String, String>,
    #[serde(default)]
    pub properties: HashMap<String, String>,
}

impl Default for MappingRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl MappingRegistry {
    pub fn new() -> Self {
        Self {
            types: HashMap::new(),
            properties: HashMap::new(),
        }
    }

    /// Load mappings from a TOML file. Returns empty registry if file doesn't exist.
    pub fn load(path: &Utf8Path) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let content = std::fs::read_to_string(path)?;
        let registry: MappingRegistry = toml::from_str(&content)?;
        Ok(registry)
    }

    /// Save mappings to a TOML file.
    pub fn save(&self, path: &Utf8Path) -> std::result::Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Look up a type mapping by user-friendly name (case-insensitive).
    pub fn resolve_type(&self, name: &str) -> Option<&str> {
        let name_lower = name.to_lowercase();
        self.types
            .iter()
            .find(|(k, _)| k.to_lowercase() == name_lower)
            .map(|(_, v)| v.as_str())
    }

    /// Look up a property mapping by user-friendly name (case-insensitive).
    pub fn resolve_property(&self, name: &str) -> Option<&str> {
        let name_lower = name.to_lowercase();
        self.properties
            .iter()
            .find(|(k, _)| k.to_lowercase() == name_lower)
            .map(|(_, v)| v.as_str())
    }

    /// Add a type mapping.
    pub fn add_type(&mut self, name: &str, iri: &str) {
        self.types.insert(name.to_string(), iri.to_string());
    }

    /// Add a property mapping.
    pub fn add_property(&mut self, name: &str, iri: &str) {
        self.properties.insert(name.to_string(), iri.to_string());
    }

    /// Compact an IRI to prefixed form (e.g. "https://schema.org/BlogPosting" → "schema:BlogPosting").
    pub fn compact_iri(iri: &str) -> String {
        for &(prefix, namespace) in KNOWN_PREFIXES {
            if let Some(local) = iri.strip_prefix(namespace) {
                return format!("{prefix}:{local}");
            }
        }
        iri.to_string()
    }

    /// Expand a prefixed IRI to full form (e.g. "schema:BlogPosting" → "https://schema.org/BlogPosting").
    pub fn expand_iri(prefixed: &str) -> Option<String> {
        let (prefix, local) = prefixed.split_once(':')?;
        for &(p, namespace) in KNOWN_PREFIXES {
            if p == prefix {
                return Some(format!("{namespace}{local}"));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_save_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = camino::Utf8PathBuf::try_from(dir.path().join("mappings.toml")).unwrap();

        let mut registry = MappingRegistry::new();
        registry.add_type("Blog Post", "https://schema.org/BlogPosting");
        registry.add_property("author", "https://schema.org/author");
        registry.save(&path).unwrap();

        let loaded = MappingRegistry::load(&path).unwrap();
        assert_eq!(
            loaded.resolve_type("Blog Post"),
            Some("https://schema.org/BlogPosting")
        );
        assert_eq!(
            loaded.resolve_property("author"),
            Some("https://schema.org/author")
        );
    }

    #[test]
    fn case_insensitive_lookup() {
        let mut registry = MappingRegistry::new();
        registry.add_type("Blog Post", "https://schema.org/BlogPosting");
        assert_eq!(
            registry.resolve_type("blog post"),
            Some("https://schema.org/BlogPosting")
        );
        assert_eq!(
            registry.resolve_type("BLOG POST"),
            Some("https://schema.org/BlogPosting")
        );
    }

    #[test]
    fn missing_file_returns_empty() {
        let path = camino::Utf8Path::new("/nonexistent/mappings.toml");
        let registry = MappingRegistry::load(path).unwrap();
        assert!(registry.types.is_empty());
        assert!(registry.properties.is_empty());
    }

    #[test]
    fn compact_iri_known_prefix() {
        assert_eq!(
            MappingRegistry::compact_iri("https://schema.org/BlogPosting"),
            "schema:BlogPosting"
        );
        assert_eq!(
            MappingRegistry::compact_iri("http://purl.org/dc/terms/title"),
            "dc:title"
        );
    }

    #[test]
    fn compact_iri_unknown_prefix() {
        assert_eq!(
            MappingRegistry::compact_iri("http://example.org/Foo"),
            "http://example.org/Foo"
        );
    }

    #[test]
    fn expand_iri_roundtrip() {
        let iri = "https://schema.org/BlogPosting";
        let compact = MappingRegistry::compact_iri(iri);
        let expanded = MappingRegistry::expand_iri(&compact).unwrap();
        assert_eq!(expanded, iri);
    }
}

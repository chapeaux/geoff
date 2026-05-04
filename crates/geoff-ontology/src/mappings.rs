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
    #[serde(skip)]
    pub extra_prefixes: HashMap<String, String>,
}

impl Default for MappingRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl MappingRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            types: HashMap::new(),
            properties: HashMap::new(),
            extra_prefixes: HashMap::new(),
        };
        reg.seed_defaults();
        reg
    }

    /// Load mappings from a TOML file. Returns empty registry if file doesn't exist.
    /// User-defined mappings override the built-in defaults.
    pub fn load(path: &Utf8Path) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let registry = if !path.exists() {
            Self::new()
        } else {
            let content = std::fs::read_to_string(path)?;
            let mut loaded: MappingRegistry = toml::from_str(&content)?;
            loaded.seed_defaults();
            loaded
        };
        Ok(registry)
    }

    fn seed_defaults(&mut self) {
        let defaults = [
            ("title", "https://schema.org/name"),
            ("name", "https://schema.org/name"),
            ("date", "https://schema.org/datePublished"),
            ("datePublished", "https://schema.org/datePublished"),
            ("author", "https://schema.org/author"),
            ("description", "https://schema.org/description"),
            ("url", "https://schema.org/url"),
            ("image", "https://schema.org/image"),
            ("keywords", "https://schema.org/keywords"),
            ("tags", "https://schema.org/keywords"),
            ("wordCount", "https://schema.org/wordCount"),
            ("language", "http://purl.org/dc/terms/language"),
            ("about", "https://schema.org/about"),
            ("publisher", "https://schema.org/publisher"),
        ];
        for (name, iri) in defaults {
            self.properties
                .entry(name.to_string())
                .or_insert_with(|| iri.to_string());
        }
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

    /// Merge additional namespace prefixes (from config) into this registry.
    pub fn add_prefixes(&mut self, prefixes: HashMap<String, String>) {
        self.extra_prefixes.extend(prefixes);
    }

    /// Returns all active prefixes (built-in + user-declared).
    pub fn all_prefixes(&self) -> Vec<(&str, &str)> {
        let mut result: Vec<(&str, &str)> = KNOWN_PREFIXES.iter().map(|&(p, ns)| (p, ns)).collect();
        for (p, ns) in &self.extra_prefixes {
            result.push((p.as_str(), ns.as_str()));
        }
        result
    }

    /// Compact an IRI to prefixed form (e.g. "https://schema.org/BlogPosting" → "schema:BlogPosting").
    /// Checks both built-in and user-declared prefixes.
    pub fn compact_iri(&self, iri: &str) -> String {
        for &(prefix, namespace) in KNOWN_PREFIXES {
            if let Some(local) = iri.strip_prefix(namespace) {
                return format!("{prefix}:{local}");
            }
        }
        for (prefix, namespace) in &self.extra_prefixes {
            if let Some(local) = iri.strip_prefix(namespace.as_str()) {
                return format!("{prefix}:{local}");
            }
        }
        iri.to_string()
    }

    /// Expand a prefixed IRI to full form (e.g. "schema:BlogPosting" → "https://schema.org/BlogPosting").
    /// Checks both built-in and user-declared prefixes.
    pub fn expand_iri(&self, prefixed: &str) -> Option<String> {
        let (prefix, local) = prefixed.split_once(':')?;
        for &(p, namespace) in KNOWN_PREFIXES {
            if p == prefix {
                return Some(format!("{namespace}{local}"));
            }
        }
        if let Some(namespace) = self.extra_prefixes.get(prefix) {
            return Some(format!("{namespace}{local}"));
        }
        None
    }

    /// Expand a prefixed IRI using only the built-in prefixes (static convenience method).
    pub fn expand_iri_builtin(prefixed: &str) -> Option<String> {
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
    fn missing_file_returns_defaults() {
        let path = camino::Utf8Path::new("/nonexistent/mappings.toml");
        let registry = MappingRegistry::load(path).unwrap();
        assert!(registry.types.is_empty());
        assert!(
            !registry.properties.is_empty(),
            "should have default property mappings"
        );
        assert_eq!(
            registry.resolve_property("title"),
            Some("https://schema.org/name")
        );
        assert_eq!(
            registry.resolve_property("author"),
            Some("https://schema.org/author")
        );
    }

    #[test]
    fn compact_iri_known_prefix() {
        let registry = MappingRegistry::new();
        assert_eq!(
            registry.compact_iri("https://schema.org/BlogPosting"),
            "schema:BlogPosting"
        );
        assert_eq!(
            registry.compact_iri("http://purl.org/dc/terms/title"),
            "dc:title"
        );
    }

    #[test]
    fn compact_iri_unknown_prefix() {
        let registry = MappingRegistry::new();
        assert_eq!(
            registry.compact_iri("http://example.org/Foo"),
            "http://example.org/Foo"
        );
    }

    #[test]
    fn expand_iri_roundtrip() {
        let registry = MappingRegistry::new();
        let iri = "https://schema.org/BlogPosting";
        let compact = registry.compact_iri(iri);
        let expanded = registry.expand_iri(&compact).unwrap();
        assert_eq!(expanded, iri);
    }

    #[test]
    fn extra_prefixes_expand_and_compact() {
        let mut registry = MappingRegistry::new();
        registry.add_prefixes(HashMap::from([
            (
                "skos".to_string(),
                "http://www.w3.org/2004/02/skos/core#".to_string(),
            ),
            ("org".to_string(), "http://www.w3.org/ns/org#".to_string()),
        ]));

        let expanded = registry.expand_iri("skos:broader").unwrap();
        assert_eq!(expanded, "http://www.w3.org/2004/02/skos/core#broader");

        let compacted = registry.compact_iri("http://www.w3.org/ns/org#member");
        assert_eq!(compacted, "org:member");

        assert!(registry.expand_iri("unknown:foo").is_none());
    }

    #[test]
    fn all_prefixes_includes_builtins_and_extras() {
        let mut registry = MappingRegistry::new();
        registry.add_prefixes(HashMap::from([(
            "skos".to_string(),
            "http://www.w3.org/2004/02/skos/core#".to_string(),
        )]));
        let prefixes = registry.all_prefixes();
        assert!(prefixes.iter().any(|(p, _)| *p == "schema"));
        assert!(prefixes.iter().any(|(p, _)| *p == "skos"));
    }
}

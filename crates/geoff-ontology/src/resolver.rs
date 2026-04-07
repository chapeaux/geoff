use crate::fuzzy::FuzzyMatcher;
use crate::mappings::MappingRegistry;
use crate::vocabulary::VocabularyIndex;

/// The result of resolving a user-provided type name to an ontology IRI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveResult {
    /// Found an exact mapping in the registry.
    Mapped(String),
    /// Found an exact label match in the vocabulary.
    ExactMatch(String),
    /// Found fuzzy matches — caller should prompt the user to choose.
    FuzzyMatches(Vec<ResolveCandidate>),
    /// No match found at all.
    NotFound,
}

/// A candidate from fuzzy matching, ready for user selection.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolveCandidate {
    pub iri: String,
    pub label: String,
    pub score: f64,
    pub source: String,
}

// Eq is safe because score is always a well-defined f64 from jaro_winkler
impl Eq for ResolveCandidate {}

/// Resolves user-friendly type names to ontology IRIs using a layered strategy:
/// 1. Check the mapping registry (persisted in mappings.toml)
/// 2. Check for exact label match in the vocabulary index
/// 3. Fall back to fuzzy matching
pub struct TypeResolver<'a> {
    pub registry: &'a MappingRegistry,
    pub index: &'a VocabularyIndex,
    pub matcher: FuzzyMatcher,
}

impl<'a> TypeResolver<'a> {
    pub fn new(registry: &'a MappingRegistry, index: &'a VocabularyIndex) -> Self {
        Self {
            registry,
            index,
            matcher: FuzzyMatcher::new(),
        }
    }

    /// Resolve a type name through the layered strategy.
    pub fn resolve_type(&self, name: &str) -> ResolveResult {
        // Layer 1: Check persisted mappings
        if let Some(iri) = self.registry.resolve_type(name) {
            return ResolveResult::Mapped(iri.to_string());
        }

        // Layer 2: Exact label match in vocabulary
        let exact = self.index.lookup_by_label(name);
        let exact_classes: Vec<_> = exact.iter().filter(|t| t.is_class).collect();
        if exact_classes.len() == 1 {
            return ResolveResult::ExactMatch(exact_classes[0].iri.clone());
        }

        // Layer 3: Fuzzy matching
        let fuzzy = self.matcher.find_class_matches(name, self.index);
        if fuzzy.is_empty() {
            return ResolveResult::NotFound;
        }

        // If the top fuzzy match is very high confidence (>= 0.95), treat as exact
        if fuzzy[0].score >= 0.95 {
            return ResolveResult::ExactMatch(fuzzy[0].term.iri.clone());
        }

        ResolveResult::FuzzyMatches(
            fuzzy
                .into_iter()
                .map(|m| ResolveCandidate {
                    iri: m.term.iri.clone(),
                    label: m.term.label.clone(),
                    score: m.score,
                    source: m.term.source.clone(),
                })
                .collect(),
        )
    }

    /// Resolve a property name through the layered strategy.
    pub fn resolve_property(&self, name: &str) -> ResolveResult {
        // Layer 1: Check persisted mappings
        if let Some(iri) = self.registry.resolve_property(name) {
            return ResolveResult::Mapped(iri.to_string());
        }

        // Layer 2: Exact label match in vocabulary
        let exact = self.index.lookup_by_label(name);
        let exact_props: Vec<_> = exact.iter().filter(|t| !t.is_class).collect();
        if exact_props.len() == 1 {
            return ResolveResult::ExactMatch(exact_props[0].iri.clone());
        }

        // Layer 3: Fuzzy matching
        let fuzzy = self.matcher.find_property_matches(name, self.index);
        if fuzzy.is_empty() {
            return ResolveResult::NotFound;
        }

        if fuzzy[0].score >= 0.95 {
            return ResolveResult::ExactMatch(fuzzy[0].term.iri.clone());
        }

        ResolveResult::FuzzyMatches(
            fuzzy
                .into_iter()
                .map(|m| ResolveCandidate {
                    iri: m.term.iri.clone(),
                    label: m.term.label.clone(),
                    score: m.score,
                    source: m.term.source.clone(),
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8Path;

    fn setup() -> (MappingRegistry, VocabularyIndex) {
        let mut index = VocabularyIndex::new();
        let manifest_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
        let ttl_path = manifest_dir.join("../../ontologies/schema-org.ttl");
        index.load_ttl(&ttl_path, "schema.org").unwrap();

        let registry = MappingRegistry::new();
        (registry, index)
    }

    #[test]
    fn resolve_from_registry() {
        let (mut registry, index) = setup();
        registry.add_type("Blog Post", "https://schema.org/BlogPosting");

        let resolver = TypeResolver::new(&registry, &index);
        let result = resolver.resolve_type("Blog Post");
        assert_eq!(
            result,
            ResolveResult::Mapped("https://schema.org/BlogPosting".to_string())
        );
    }

    #[test]
    fn resolve_exact_label() {
        let (registry, index) = setup();
        let resolver = TypeResolver::new(&registry, &index);

        // "Blog Post" is the rdfs:label of schema:BlogPosting
        let result = resolver.resolve_type("Blog Post");
        assert!(
            matches!(result, ResolveResult::ExactMatch(ref iri) if iri.contains("BlogPosting")),
            "Expected ExactMatch for 'Blog Post', got: {result:?}"
        );
    }

    #[test]
    fn resolve_fuzzy() {
        let (registry, index) = setup();
        let resolver = TypeResolver::new(&registry, &index);

        let result = resolver.resolve_type("blogpost");
        match result {
            ResolveResult::ExactMatch(iri) => {
                assert!(
                    iri.contains("BlogPosting"),
                    "Expected BlogPosting, got: {iri}"
                );
            }
            ResolveResult::FuzzyMatches(candidates) => {
                assert!(
                    candidates.iter().any(|c| c.iri.contains("BlogPosting")),
                    "Expected BlogPosting in fuzzy candidates"
                );
            }
            other => panic!("Expected ExactMatch or FuzzyMatches for 'blogpost', got: {other:?}"),
        }
    }

    #[test]
    fn resolve_not_found() {
        let (registry, index) = setup();
        let resolver = TypeResolver::new(&registry, &index);

        let result = resolver.resolve_type("CompletelyMadeUpTypeThatDoesNotExist");
        assert_eq!(result, ResolveResult::NotFound);
    }

    #[test]
    fn registry_takes_priority() {
        let (mut registry, index) = setup();
        // Map "Article" to a custom IRI, even though it matches a vocab term
        registry.add_type("Article", "urn:custom:MyArticle");

        let resolver = TypeResolver::new(&registry, &index);
        let result = resolver.resolve_type("Article");
        assert_eq!(
            result,
            ResolveResult::Mapped("urn:custom:MyArticle".to_string())
        );
    }

    #[test]
    fn resolve_property_exact() {
        let (registry, index) = setup();
        let resolver = TypeResolver::new(&registry, &index);

        // "name" is the rdfs:label of schema:name
        let result = resolver.resolve_property("name");
        assert!(
            matches!(
                result,
                ResolveResult::ExactMatch(_) | ResolveResult::FuzzyMatches(_)
            ),
            "Expected match for property 'name', got: {result:?}"
        );
    }
}

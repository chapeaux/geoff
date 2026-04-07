use std::collections::HashMap;

use camino::Utf8Path;
use geoff_graph::store::ContentStore;

/// A vocabulary term (class or property) with human-readable labels.
#[derive(Debug, Clone)]
pub struct VocabTerm {
    /// The full IRI (e.g. "https://schema.org/BlogPosting").
    pub iri: String,
    /// The primary label (rdfs:label).
    pub label: String,
    /// Description (rdfs:comment).
    pub comment: String,
    /// Alternative labels (skos:altLabel).
    pub alt_labels: Vec<String>,
    /// Whether this is a class (true) or property (false).
    pub is_class: bool,
    /// The vocabulary source (e.g. "schema.org", "Dublin Core").
    pub source: String,
}

/// In-memory index of vocabulary terms for fast lookup.
pub struct VocabularyIndex {
    /// All terms indexed by IRI.
    terms: HashMap<String, VocabTerm>,
    /// Label → IRI lookup (lowercase).
    label_index: HashMap<String, Vec<String>>,
}

impl Default for VocabularyIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl VocabularyIndex {
    /// Create an empty vocabulary index.
    pub fn new() -> Self {
        Self {
            terms: HashMap::new(),
            label_index: HashMap::new(),
        }
    }

    /// Load vocabulary terms from a Turtle (.ttl) file by parsing it into a
    /// temporary Oxigraph store and querying for labels.
    pub fn load_ttl(
        &mut self,
        path: &Utf8Path,
        source_name: &str,
    ) -> std::result::Result<usize, Box<dyn std::error::Error>> {
        let store = ContentStore::new()?;
        store.load_turtle(path)?;

        // Query for classes
        let classes_query = r#"
            PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
            PREFIX skos: <http://www.w3.org/2004/02/skos/core#>
            SELECT ?term ?label ?comment (GROUP_CONCAT(?alt; SEPARATOR="\t") AS ?alts) WHERE {
                ?term a rdfs:Class .
                ?term rdfs:label ?label .
                OPTIONAL { ?term rdfs:comment ?comment }
                OPTIONAL { ?term skos:altLabel ?alt }
            }
            GROUP BY ?term ?label ?comment
        "#;
        self.load_from_query(&store, classes_query, true, source_name)?;

        // Query for properties
        let props_query = r#"
            PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
            PREFIX skos: <http://www.w3.org/2004/02/skos/core#>
            SELECT ?term ?label ?comment (GROUP_CONCAT(?alt; SEPARATOR="\t") AS ?alts) WHERE {
                ?term a rdf:Property .
                ?term rdfs:label ?label .
                OPTIONAL { ?term rdfs:comment ?comment }
                OPTIONAL { ?term skos:altLabel ?alt }
            }
            GROUP BY ?term ?label ?comment
        "#;
        self.load_from_query(&store, props_query, false, source_name)?;

        Ok(self.terms.len())
    }

    fn load_from_query(
        &mut self,
        store: &ContentStore,
        query: &str,
        is_class: bool,
        source_name: &str,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let results = store.query_to_json(query)?;
        let rows = results.as_array().ok_or("Expected array from SPARQL")?;

        for row in rows {
            let iri = row["term"]
                .as_str()
                .ok_or("Missing term IRI")?
                .trim_matches('<')
                .trim_matches('>')
                .to_string();
            let label = row["label"].as_str().unwrap_or("").to_string();
            let comment = row["comment"].as_str().unwrap_or("").to_string();
            let alts_str = row["alts"].as_str().unwrap_or("");
            let alt_labels: Vec<String> = alts_str
                .split('\t')
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty())
                .collect();

            // Index by all labels (lowercase)
            let all_labels: Vec<String> = std::iter::once(label.to_lowercase())
                .chain(alt_labels.iter().map(|a| a.to_lowercase()))
                .collect();

            for lbl in &all_labels {
                self.label_index
                    .entry(lbl.clone())
                    .or_default()
                    .push(iri.clone());
            }

            self.terms.insert(
                iri.clone(),
                VocabTerm {
                    iri,
                    label,
                    comment,
                    alt_labels,
                    is_class,
                    source: source_name.to_string(),
                },
            );
        }
        Ok(())
    }

    /// Load all .ttl files from a directory.
    pub fn load_directory(
        &mut self,
        dir: &Utf8Path,
    ) -> std::result::Result<usize, Box<dyn std::error::Error>> {
        if !dir.exists() {
            return Ok(0);
        }
        let mut count = 0;
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "ttl") {
                let utf8_path = camino::Utf8PathBuf::try_from(path.clone())
                    .map_err(|e| format!("Non-UTF8 path: {e}"))?;
                let source = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");
                self.load_ttl(&utf8_path, source)?;
                count += 1;
            }
        }
        Ok(count)
    }

    /// Get a term by its IRI.
    pub fn get(&self, iri: &str) -> Option<&VocabTerm> {
        self.terms.get(iri)
    }

    /// Look up terms by exact label match (case-insensitive).
    pub fn lookup_by_label(&self, label: &str) -> Vec<&VocabTerm> {
        self.label_index
            .get(&label.to_lowercase())
            .map(|iris| iris.iter().filter_map(|iri| self.terms.get(iri)).collect())
            .unwrap_or_default()
    }

    /// Get all terms in the index.
    pub fn all_terms(&self) -> impl Iterator<Item = &VocabTerm> {
        self.terms.values()
    }

    /// Get all class terms.
    pub fn classes(&self) -> impl Iterator<Item = &VocabTerm> {
        self.terms.values().filter(|t| t.is_class)
    }

    /// Get all property terms.
    pub fn properties(&self) -> impl Iterator<Item = &VocabTerm> {
        self.terms.values().filter(|t| !t.is_class)
    }

    /// Get the number of terms in the index.
    pub fn len(&self) -> usize {
        self.terms.len()
    }

    /// Check if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ttl_path() -> camino::Utf8PathBuf {
        let manifest_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
        manifest_dir.join("../../ontologies/schema-org.ttl")
    }

    #[test]
    fn load_schema_org_ttl() {
        let mut index = VocabularyIndex::new();
        let count = index.load_ttl(&test_ttl_path(), "schema.org").unwrap();
        assert!(count > 10, "Should load many terms from schema-org.ttl");
    }

    #[test]
    fn lookup_by_label_exact() {
        let mut index = VocabularyIndex::new();
        index.load_ttl(&test_ttl_path(), "schema.org").unwrap();

        let results = index.lookup_by_label("Blog Post");
        assert_eq!(results.len(), 1);
        assert!(results[0].iri.contains("BlogPosting"));
    }

    #[test]
    fn lookup_by_alt_label() {
        let mut index = VocabularyIndex::new();
        index.load_ttl(&test_ttl_path(), "schema.org").unwrap();

        // "post" is an altLabel for BlogPosting
        let results = index.lookup_by_label("post");
        assert!(
            results.iter().any(|t| t.iri.contains("BlogPosting")),
            "Should find BlogPosting via alt label 'post'"
        );
    }

    #[test]
    fn classes_and_properties_separated() {
        let mut index = VocabularyIndex::new();
        index.load_ttl(&test_ttl_path(), "schema.org").unwrap();

        let class_count = index.classes().count();
        let prop_count = index.properties().count();
        assert!(class_count > 0);
        assert!(prop_count > 0);
        assert_eq!(class_count + prop_count, index.len());
    }
}

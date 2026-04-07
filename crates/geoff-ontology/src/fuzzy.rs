use crate::vocabulary::{VocabTerm, VocabularyIndex};

/// A match candidate with its similarity score.
#[derive(Debug, Clone)]
pub struct FuzzyMatch<'a> {
    pub term: &'a VocabTerm,
    pub score: f64,
    /// Which label matched (the primary or an alt).
    pub matched_label: String,
}

/// Fuzzy matcher that ranks vocabulary terms against a query string.
pub struct FuzzyMatcher {
    /// Minimum similarity score to consider a match (0.0–1.0).
    pub threshold: f64,
    /// Maximum number of results to return.
    pub max_results: usize,
}

impl Default for FuzzyMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl FuzzyMatcher {
    pub fn new() -> Self {
        Self {
            threshold: 0.7,
            max_results: 5,
        }
    }

    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold;
        self
    }

    pub fn with_max_results(mut self, max_results: usize) -> Self {
        self.max_results = max_results;
        self
    }

    /// Find the best matching terms for a query string.
    /// Checks the primary label and all alt labels, returning the best score per term.
    pub fn find_matches<'a>(&self, query: &str, index: &'a VocabularyIndex) -> Vec<FuzzyMatch<'a>> {
        let query_lower = query.to_lowercase();
        let mut matches: Vec<FuzzyMatch<'a>> = Vec::new();

        for term in index.all_terms() {
            let mut best_score = 0.0_f64;
            let mut best_label = String::new();

            // Check primary label
            let label_lower = term.label.to_lowercase();
            let score = strsim::jaro_winkler(&query_lower, &label_lower);
            if score > best_score {
                best_score = score;
                best_label = term.label.clone();
            }

            // Check alt labels
            for alt in &term.alt_labels {
                let alt_lower = alt.to_lowercase();
                let score = strsim::jaro_winkler(&query_lower, &alt_lower);
                if score > best_score {
                    best_score = score;
                    best_label = alt.clone();
                }
            }

            if best_score >= self.threshold {
                matches.push(FuzzyMatch {
                    term,
                    score: best_score,
                    matched_label: best_label,
                });
            }
        }

        // Sort by score descending
        matches.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        matches.truncate(self.max_results);
        matches
    }

    /// Find the best matching terms among classes only.
    pub fn find_class_matches<'a>(
        &self,
        query: &str,
        index: &'a VocabularyIndex,
    ) -> Vec<FuzzyMatch<'a>> {
        let all = self.find_matches(query, index);
        all.into_iter().filter(|m| m.term.is_class).collect()
    }

    /// Find the best matching terms among properties only.
    pub fn find_property_matches<'a>(
        &self,
        query: &str,
        index: &'a VocabularyIndex,
    ) -> Vec<FuzzyMatch<'a>> {
        let all = self.find_matches(query, index);
        all.into_iter().filter(|m| !m.term.is_class).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8Path;

    fn load_test_index() -> VocabularyIndex {
        let mut index = VocabularyIndex::new();
        let manifest_dir = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
        let ttl_path = manifest_dir.join("../../ontologies/schema-org.ttl");
        index.load_ttl(&ttl_path, "schema.org").unwrap();
        index
    }

    #[test]
    fn exact_label_scores_high() {
        let index = load_test_index();
        let matcher = FuzzyMatcher::new();
        let matches = matcher.find_matches("Blog Post", &index);
        assert!(!matches.is_empty());
        assert!(
            matches[0].term.iri.contains("BlogPosting"),
            "Expected BlogPosting as top match, got: {}",
            matches[0].term.iri
        );
        assert!(matches[0].score > 0.9);
    }

    #[test]
    fn fuzzy_finds_close_match() {
        let index = load_test_index();
        let matcher = FuzzyMatcher::new().with_threshold(0.6);
        let matches = matcher.find_matches("blog", &index);
        assert!(
            matches.iter().any(|m| m.term.iri.contains("BlogPosting")),
            "Should find BlogPosting for 'blog'"
        );
    }

    #[test]
    fn class_filter_works() {
        let index = load_test_index();
        let matcher = FuzzyMatcher::new().with_threshold(0.6);
        let matches = matcher.find_class_matches("article", &index);
        assert!(!matches.is_empty());
        for m in &matches {
            assert!(m.term.is_class, "All results should be classes");
        }
    }

    #[test]
    fn threshold_filters_low_scores() {
        let index = load_test_index();
        let matcher = FuzzyMatcher::new().with_threshold(0.95);
        let matches = matcher.find_matches("xyzzyplugh", &index);
        assert!(
            matches.is_empty(),
            "Nonsense query should return no matches"
        );
    }

    #[test]
    fn max_results_limits_output() {
        let index = load_test_index();
        let matcher = FuzzyMatcher::new().with_threshold(0.5).with_max_results(3);
        let matches = matcher.find_matches("page", &index);
        assert!(matches.len() <= 3);
    }
}

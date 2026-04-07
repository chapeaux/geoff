/// A page URI in the form `urn:geoff:content:{path}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PageUri(pub String);

impl PageUri {
    /// Create a page URI from a content-relative path.
    pub fn from_path(path: &str) -> Self {
        Self(format!("urn:geoff:content:{}", iri_escape(path)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PageUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A predicate IRI (e.g. `schema:name`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PredicateIri(pub String);

impl PredicateIri {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PredicateIri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Well-known XSD datatype IRIs for typed literals.
pub mod xsd {
    pub const STRING: &str = "http://www.w3.org/2001/XMLSchema#string";
    pub const DATE: &str = "http://www.w3.org/2001/XMLSchema#date";
    pub const DATE_TIME: &str = "http://www.w3.org/2001/XMLSchema#dateTime";
    pub const INTEGER: &str = "http://www.w3.org/2001/XMLSchema#integer";
    pub const BOOLEAN: &str = "http://www.w3.org/2001/XMLSchema#boolean";
    pub const DECIMAL: &str = "http://www.w3.org/2001/XMLSchema#decimal";
    pub const DOUBLE: &str = "http://www.w3.org/2001/XMLSchema#double";
}

/// An RDF object value — either a named node (IRI), a plain literal, or a typed literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectValue {
    Iri(String),
    Literal(String),
    /// A typed literal with value and XSD datatype IRI.
    TypedLiteral {
        value: String,
        datatype: String,
    },
}

/// Well-known named graph IRIs.
pub const GRAPH_ONTOLOGY: &str = "urn:geoff:ontology";
pub const GRAPH_SITE: &str = "urn:geoff:site";

/// Percent-encode characters that are invalid in IRIs.
pub fn iri_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || "-._~:@!$&'()*+,;=/".contains(c) {
            out.push(c);
        } else {
            for b in c.to_string().as_bytes() {
                use std::fmt::Write;
                let _ = write!(out, "%{b:02X}");
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_uri_from_path() {
        let uri = PageUri::from_path("blog/hello-world.md");
        assert_eq!(uri.as_str(), "urn:geoff:content:blog/hello-world.md");
    }

    #[test]
    fn iri_escape_spaces() {
        assert_eq!(iri_escape("hello world"), "hello%20world");
    }

    #[test]
    fn iri_escape_passthrough() {
        assert_eq!(iri_escape("simple-path.md"), "simple-path.md");
    }
}

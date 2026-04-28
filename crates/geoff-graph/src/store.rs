use camino::Utf8Path;
use geoff_core::types::ObjectValue;
use oxigraph::io::{RdfFormat, RdfParser};
use oxigraph::model::{GraphNameRef, Literal, NamedNodeRef, QuadRef, Term};
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use serde_json::{Map, Value};

/// Wraps Oxigraph `Store`, providing named-graph-aware RDF operations.
/// No other crate should import oxigraph directly.
#[derive(Clone)]
pub struct ContentStore {
    store: Store,
}

impl ContentStore {
    /// Create a new in-memory content store.
    pub fn new() -> std::result::Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            store: Store::new()?,
        })
    }

    /// Insert a triple into the specified named graph.
    pub fn insert_triple_into(
        &self,
        subject: &str,
        predicate: &str,
        object: &ObjectValue,
        graph: &str,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let s_node = NamedNodeRef::new(subject)?;
        let p_node = NamedNodeRef::new(predicate)?;
        let g_node = NamedNodeRef::new(graph)?;

        match object {
            ObjectValue::Iri(iri) => {
                let o_node = NamedNodeRef::new(iri)?;
                self.store.insert(QuadRef::new(
                    s_node,
                    p_node,
                    o_node,
                    GraphNameRef::NamedNode(g_node),
                ))?;
            }
            ObjectValue::Literal(value) => {
                let lit = Literal::new_simple_literal(value);
                self.store.insert(QuadRef::new(
                    s_node,
                    p_node,
                    lit.as_ref(),
                    GraphNameRef::NamedNode(g_node),
                ))?;
            }
            ObjectValue::TypedLiteral { value, datatype } => {
                let dt_node = NamedNodeRef::new(datatype)?;
                let lit = Literal::new_typed_literal(value, dt_node);
                self.store.insert(QuadRef::new(
                    s_node,
                    p_node,
                    lit.as_ref(),
                    GraphNameRef::NamedNode(g_node),
                ))?;
            }
        }
        Ok(())
    }

    /// Execute a SPARQL SELECT or ASK query and return results as JSON.
    pub fn query_to_json(
        &self,
        sparql: &str,
    ) -> std::result::Result<Value, Box<dyn std::error::Error>> {
        let results = SparqlEvaluator::new()
            .parse_query(sparql)?
            .on_store(&self.store)
            .execute()?;

        match results {
            QueryResults::Solutions(solutions) => {
                let variables: Vec<String> = solutions
                    .variables()
                    .iter()
                    .map(|v| v.as_str().to_owned())
                    .collect();

                let mut rows = Vec::new();
                for solution in solutions {
                    let solution = solution?;
                    let mut row = Map::new();
                    for var in &variables {
                        let value =
                            solution
                                .get(var.as_str())
                                .map_or(Value::Null, |term| match term {
                                    Term::Literal(lit) => Value::String(lit.value().to_string()),
                                    other => Value::String(other.to_string()),
                                });
                        row.insert(var.clone(), value);
                    }
                    rows.push(Value::Object(row));
                }
                Ok(Value::Array(rows))
            }
            QueryResults::Boolean(b) => Ok(Value::Bool(b)),
            QueryResults::Graph(_) => Err("CONSTRUCT/DESCRIBE queries not supported".into()),
        }
    }

    /// Load a Turtle (.ttl) file into the default graph.
    pub fn load_turtle(
        &self,
        path: &Utf8Path,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let file = std::fs::File::open(path.as_std_path())?;
        let reader = std::io::BufReader::new(file);
        self.store.load_from_reader(RdfFormat::Turtle, reader)?;
        Ok(())
    }

    /// Load a Turtle (.ttl) file into a specific named graph.
    pub fn load_turtle_into(
        &self,
        path: &Utf8Path,
        graph: &str,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let g_node = NamedNodeRef::new(graph)?;
        self.store.load_from_reader(
            RdfParser::from_format(RdfFormat::Turtle)
                .without_named_graphs()
                .with_default_graph(g_node),
            content.as_bytes(),
        )?;
        Ok(())
    }

    /// Clear all data from the store.
    pub fn clear(&self) -> std::result::Result<(), Box<dyn std::error::Error>> {
        self.store.clear()?;
        Ok(())
    }

    /// Export all triples as N-Triples for client-side SPARQL search.
    ///
    /// Includes every triple from every named graph so that custom RDF
    /// properties (from `[rdf.custom]` frontmatter) are queryable in the
    /// browser alongside standard schema.org fields.
    pub fn export_search_ntriples(
        &self,
    ) -> std::result::Result<String, Box<dyn std::error::Error>> {
        use std::fmt::Write;
        let mut out = String::new();
        for quad in self.store.iter() {
            let quad = quad?;
            writeln!(out, "{} {} {} .", quad.subject, quad.predicate, quad.object)?;
        }
        Ok(out)
    }

    /// Export all triples (flattened from all named graphs) as NTriples.
    ///
    /// This is useful for SHACL validation, which operates on a flat graph.
    pub fn export_turtle(&self) -> std::result::Result<String, Box<dyn std::error::Error>> {
        let mut out = String::new();
        for quad in self.store.iter() {
            let quad = quad?;
            use std::fmt::Write;
            writeln!(out, "{} {} {} .", quad.subject, quad.predicate, quad.object)?;
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_query_named_graph() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let store = ContentStore::new()?;
        store.insert_triple_into(
            "urn:geoff:content:blog/hello.md",
            "http://schema.org/name",
            &ObjectValue::Literal("Hello World".into()),
            "urn:geoff:content:blog/hello.md",
        )?;

        let json = store.query_to_json(
            "SELECT ?name WHERE { GRAPH <urn:geoff:content:blog/hello.md> { ?s <http://schema.org/name> ?name } }",
        )?;

        let rows = json.as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["name"], "Hello World");
        Ok(())
    }

    #[test]
    fn insert_iri_object() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let store = ContentStore::new()?;
        store.insert_triple_into(
            "urn:geoff:content:blog/hello.md",
            "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
            &ObjectValue::Iri("http://schema.org/BlogPosting".into()),
            "urn:geoff:content:blog/hello.md",
        )?;

        let json = store.query_to_json(
            "ASK { GRAPH <urn:geoff:content:blog/hello.md> { <urn:geoff:content:blog/hello.md> a <http://schema.org/BlogPosting> } }",
        )?;
        assert_eq!(json, Value::Bool(true));
        Ok(())
    }

    #[test]
    fn insert_typed_literal() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let store = ContentStore::new()?;
        store.insert_triple_into(
            "urn:geoff:content:blog/hello.md",
            "http://schema.org/datePublished",
            &ObjectValue::TypedLiteral {
                value: "2026-04-01".into(),
                datatype: "http://www.w3.org/2001/XMLSchema#date".into(),
            },
            "urn:geoff:content:blog/hello.md",
        )?;

        // xsd:date typed literals are queryable with SPARQL date functions
        let json = store.query_to_json(
            "SELECT ?d WHERE { GRAPH <urn:geoff:content:blog/hello.md> { ?s <http://schema.org/datePublished> ?d } }",
        )?;
        let rows = json.as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["d"], "2026-04-01");
        Ok(())
    }

    #[test]
    fn clear_empties_store() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let store = ContentStore::new()?;
        store.insert_triple_into(
            "urn:geoff:content:a",
            "http://example.org/p",
            &ObjectValue::Literal("v".into()),
            "urn:geoff:site",
        )?;
        store.clear()?;

        let json = store.query_to_json("SELECT ?s ?p ?o WHERE { GRAPH ?g { ?s ?p ?o } }")?;
        let rows = json.as_array().unwrap();
        assert!(rows.is_empty());
        Ok(())
    }
}

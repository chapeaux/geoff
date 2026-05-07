use wasm_bindgen::prelude::*;

use oxigraph::io::RdfFormat;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;

/// WASM SPARQL engine for Geoff sites.
///
/// Loads N-Triples data and executes SPARQL SELECT/ASK queries,
/// returning results as JSON. Used by both browser search components
/// and AI agents via the MCP manifest.
#[wasm_bindgen]
pub struct GeoffSparql {
    store: Store,
}

#[wasm_bindgen]
impl GeoffSparql {
    /// Create a new empty SPARQL engine.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<GeoffSparql, JsValue> {
        Store::new()
            .map(|store| GeoffSparql { store })
            .map_err(|e| JsValue::from_str(&format!("Failed to create store: {e}")))
    }

    /// Load N-Triples data into the store.
    /// Can be called multiple times to load additional data.
    pub fn load(&self, ntriples: &str) -> Result<(), JsValue> {
        self.store
            .load_from_reader(RdfFormat::NTriples, ntriples.as_bytes())
            .map_err(|e| JsValue::from_str(&format!("Failed to load data: {e}")))
    }

    /// Execute a SPARQL SELECT or ASK query.
    ///
    /// Returns a JSON string:
    /// - SELECT: `[{"var1": "val1", "var2": "val2"}, ...]`
    /// - ASK: `true` or `false`
    pub fn query(&self, sparql: &str) -> Result<String, JsValue> {
        let results = SparqlEvaluator::new()
            .parse_query(sparql)
            .map_err(|e| JsValue::from_str(&format!("SPARQL parse error: {e}")))?
            .on_store(&self.store)
            .execute()
            .map_err(|e| JsValue::from_str(&format!("SPARQL execution error: {e}")))?;

        match results {
            QueryResults::Solutions(solutions) => {
                let variables: Vec<String> = solutions
                    .variables()
                    .iter()
                    .map(|v| v.as_str().to_owned())
                    .collect();

                let mut rows = Vec::new();
                for solution in solutions {
                    let solution =
                        solution.map_err(|e| JsValue::from_str(&format!("Solution error: {e}")))?;
                    let mut row = serde_json::Map::new();
                    for var in &variables {
                        if let Some(term) = solution.get(var.as_str()) {
                            let value = match term {
                                oxigraph::model::Term::Literal(lit) => {
                                    serde_json::Value::String(lit.value().to_string())
                                }
                                other => serde_json::Value::String(other.to_string()),
                            };
                            row.insert(var.clone(), value);
                        }
                    }
                    rows.push(serde_json::Value::Object(row));
                }
                serde_json::to_string(&rows)
                    .map_err(|e| JsValue::from_str(&format!("JSON error: {e}")))
            }
            QueryResults::Boolean(b) => Ok(b.to_string()),
            QueryResults::Graph(_) => Err(JsValue::from_str(
                "CONSTRUCT/DESCRIBE queries not supported",
            )),
        }
    }

    /// Load N-Triples data and execute a query in one call.
    /// Convenience method for agents that provide data inline.
    pub fn query_with_data(&self, sparql: &str, ntriples: &str) -> Result<String, JsValue> {
        self.load(ntriples)?;
        self.query(sparql)
    }
}

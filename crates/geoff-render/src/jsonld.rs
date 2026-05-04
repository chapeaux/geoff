use geoff_graph::store::ContentStore;
use geoff_ontology::mappings::MappingRegistry;
use serde_json::{Map, Value, json};

/// Build a JSON-LD object from the page's RDF graph, including all triples.
/// Properties from the default vocabulary use short names; others use prefixed names.
/// Internal geoff predicates (urn:geoff:*) are filtered out.
pub fn build_jsonld_from_graph(
    store: &ContentStore,
    page_uri: &str,
    base_url: &str,
    page_url: &str,
    default_vocab: &str,
    registry: &MappingRegistry,
) -> Value {
    let query = format!("SELECT ?p ?o WHERE {{ GRAPH <{page_uri}> {{ <{page_uri}> ?p ?o }} }}");

    let mut obj = Map::new();
    let mut used_prefixes: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();

    let page_id = format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        page_url.trim_start_matches('/')
    );
    obj.insert("@id".into(), json!(page_id));

    if let Ok(results) = store.query_to_json(&query)
        && let Some(rows) = results.as_array()
    {
        for row in rows {
                let raw_pred = match row.get("p").and_then(|v| v.as_str()) {
                    Some(p) => p,
                    None => continue,
                };
                let pred = raw_pred
                    .strip_prefix('<')
                    .and_then(|s| s.strip_suffix('>'))
                    .unwrap_or(raw_pred);
                let raw_obj = match row.get("o").and_then(|v| v.as_str()) {
                    Some(o) => o,
                    None => continue,
                };
                // Strip angle brackets from IRI serialization (e.g. "<https://...>" → "https://...")
                let obj_val = raw_obj
                    .strip_prefix('<')
                    .and_then(|s| s.strip_suffix('>'))
                    .unwrap_or(raw_obj);

                // Skip internal geoff predicates
                if pred.starts_with("urn:geoff:") {
                    continue;
                }

                // Handle rdf:type specially
                if pred == "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"
                    || pred == "https://www.w3.org/1999/02/22-rdf-syntax-ns#type"
                {
                    let type_name =
                        compact_for_jsonld(obj_val, default_vocab, registry, &mut used_prefixes);
                    obj.insert("@type".into(), json!(type_name));
                    continue;
                }

                let key = compact_for_jsonld(pred, default_vocab, registry, &mut used_prefixes);

                // Special handling for author → Person wrapper
                if key == "author" || key == "schema:author" {
                    obj.insert(key, json!({ "@type": "Person", "name": obj_val }));
                    continue;
                }

                // Try to parse as number or bool for typed output
                let value = if let Ok(n) = obj_val.parse::<i64>() {
                    json!(n)
                } else if let Ok(n) = obj_val.parse::<f64>() {
                    json!(n)
                } else if obj_val == "true" || obj_val == "false" {
                    json!(obj_val == "true")
                } else {
                    json!(obj_val)
                };

                obj.insert(key, value);
        }
    }

    // Build @context with default vocab and any used prefixes
    if used_prefixes.is_empty() {
        obj.insert("@context".into(), json!(default_vocab));
    } else {
        let mut context = Map::new();
        context.insert("@vocab".into(), json!(default_vocab));
        for (prefix, namespace) in &used_prefixes {
            context.insert(prefix.clone(), json!(namespace));
        }
        obj.insert("@context".into(), Value::Object(context));
    }

    Value::Object(obj)
}

/// Compact an IRI for JSON-LD output.
/// If the IRI is in the default vocabulary, return just the local name.
/// Otherwise, return the prefixed form and track the used prefix.
fn compact_for_jsonld(
    iri: &str,
    default_vocab: &str,
    registry: &MappingRegistry,
    used_prefixes: &mut std::collections::BTreeMap<String, String>,
) -> String {
    // Check if it's in the default vocabulary → use short name
    if let Some(local) = iri.strip_prefix(default_vocab) {
        return local.to_string();
    }

    // Compact via registry (checks built-in + extra prefixes)
    let compacted = registry.compact_iri(iri);
    if compacted != iri {
        // Track the prefix for @context
        if let Some((prefix, _)) = compacted.split_once(':') {
            for (p, ns) in registry.all_prefixes() {
                if p == prefix {
                    used_prefixes.insert(prefix.to_string(), ns.to_string());
                    break;
                }
            }
        }
        return compacted;
    }

    iri.to_string()
}

/// Build a JSON-LD object for a page from its frontmatter fields.
pub fn build_jsonld(
    base_url: &str,
    page_path: &str,
    title: Option<&str>,
    date: Option<&str>,
    author: Option<&str>,
    content_type: Option<&str>,
) -> Value {
    let mut obj = Map::new();

    obj.insert("@context".into(), json!("https://schema.org/"));

    let schema_type = match content_type {
        Some("Blog Post") | Some("BlogPosting") => "BlogPosting",
        Some("Article") => "Article",
        Some("How-To Guide") | Some("HowTo") => "HowTo",
        Some("FAQ Page") | Some("FAQPage") => "FAQPage",
        Some("Event") => "Event",
        Some("Web Page") | Some("WebPage") => "WebPage",
        Some(other) => other,
        None => "WebPage",
    };
    obj.insert("@type".into(), json!(schema_type));

    let page_url = format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        page_path.trim_start_matches('/')
    );
    obj.insert("@id".into(), json!(page_url));

    if let Some(title) = title {
        obj.insert("name".into(), json!(title));
    }

    if let Some(date) = date {
        obj.insert("datePublished".into(), json!(date));
    }

    if let Some(author) = author {
        obj.insert(
            "author".into(),
            json!({
                "@type": "Person",
                "name": author
            }),
        );
    }

    Value::Object(obj)
}

/// Render a JSON-LD value as an HTML `<script>` block.
pub fn jsonld_script_tag(jsonld: &Value) -> String {
    let json_str = serde_json::to_string_pretty(jsonld).unwrap_or_default();
    format!("<script type=\"application/ld+json\">\n{json_str}\n</script>")
}

#[cfg(test)]
mod tests {
    use super::*;
    use geoff_core::types::ObjectValue;

    #[test]
    fn build_jsonld_from_graph_includes_all_triples() {
        let store = ContentStore::new().unwrap();
        let page_uri = "urn:geoff:content:blog/hello.md";
        let graph = page_uri;

        store
            .insert_triple_into(
                page_uri,
                "https://schema.org/name",
                &ObjectValue::Literal("Hello World".into()),
                graph,
            )
            .unwrap();
        store
            .insert_triple_into(
                page_uri,
                "https://schema.org/datePublished",
                &ObjectValue::Literal("2026-04-10".into()),
                graph,
            )
            .unwrap();
        store
            .insert_triple_into(
                page_uri,
                "https://schema.org/author",
                &ObjectValue::Literal("ldary".into()),
                graph,
            )
            .unwrap();
        store
            .insert_triple_into(
                page_uri,
                "https://schema.org/wordCount",
                &ObjectValue::Literal("1500".into()),
                graph,
            )
            .unwrap();
        store
            .insert_triple_into(
                page_uri,
                "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
                &ObjectValue::Iri("https://schema.org/BlogPosting".into()),
                graph,
            )
            .unwrap();

        let registry = MappingRegistry::new();
        let ld = build_jsonld_from_graph(
            &store,
            page_uri,
            "https://example.com",
            "/blog/hello/",
            "https://schema.org/",
            &registry,
        );

        assert_eq!(ld["@type"], "BlogPosting");
        assert_eq!(ld["name"], "Hello World");
        assert_eq!(ld["datePublished"], "2026-04-10");
        assert_eq!(ld["author"]["name"], "ldary");
        assert_eq!(ld["wordCount"], 1500);
        assert_eq!(ld["@id"], "https://example.com/blog/hello/");
    }

    #[test]
    fn build_jsonld_from_graph_multi_vocab() {
        let store = ContentStore::new().unwrap();
        let page_uri = "urn:geoff:content:test.md";

        store
            .insert_triple_into(
                page_uri,
                "https://schema.org/name",
                &ObjectValue::Literal("Test".into()),
                page_uri,
            )
            .unwrap();
        store
            .insert_triple_into(
                page_uri,
                "http://purl.org/dc/terms/language",
                &ObjectValue::Literal("en".into()),
                page_uri,
            )
            .unwrap();

        let registry = MappingRegistry::new();
        let ld = build_jsonld_from_graph(
            &store,
            page_uri,
            "https://example.com",
            "/test/",
            "https://schema.org/",
            &registry,
        );

        assert_eq!(ld["name"], "Test");
        assert_eq!(ld["dc:language"], "en");
        let ctx = &ld["@context"];
        assert_eq!(ctx["@vocab"], "https://schema.org/");
        assert_eq!(ctx["dc"], "http://purl.org/dc/terms/");
    }

    #[test]
    fn build_jsonld_from_graph_filters_geoff_internals() {
        let store = ContentStore::new().unwrap();
        let page_uri = "urn:geoff:content:test.md";

        store
            .insert_triple_into(
                page_uri,
                "https://schema.org/name",
                &ObjectValue::Literal("Test".into()),
                page_uri,
            )
            .unwrap();
        store
            .insert_triple_into(
                page_uri,
                "urn:geoff:meta:template",
                &ObjectValue::Literal("page.html".into()),
                page_uri,
            )
            .unwrap();

        let registry = MappingRegistry::new();
        let ld = build_jsonld_from_graph(
            &store,
            page_uri,
            "https://example.com",
            "/test/",
            "https://schema.org/",
            &registry,
        );

        assert_eq!(ld["name"], "Test");
        assert!(ld.get("urn:geoff:meta:template").is_none());
    }

    #[test]
    fn build_blog_jsonld() {
        let ld = build_jsonld(
            "https://example.com",
            "blog/hello",
            Some("Hello World"),
            Some("2026-04-10"),
            Some("ldary"),
            Some("Blog Post"),
        );
        assert_eq!(ld["@type"], "BlogPosting");
        assert_eq!(ld["name"], "Hello World");
        assert_eq!(ld["datePublished"], "2026-04-10");
        assert_eq!(ld["author"]["name"], "ldary");
    }

    #[test]
    fn default_type_is_webpage() {
        let ld = build_jsonld(
            "https://example.com",
            "/about",
            Some("About"),
            None,
            None,
            None,
        );
        assert_eq!(ld["@type"], "WebPage");
    }

    #[test]
    fn script_tag_format() {
        let ld = json!({"@context": "https://schema.org/", "@type": "WebPage"});
        let tag = jsonld_script_tag(&ld);
        assert!(tag.starts_with("<script type=\"application/ld+json\">"));
        assert!(tag.ends_with("</script>"));
    }
}

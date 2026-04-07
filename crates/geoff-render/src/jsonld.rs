use serde_json::{Map, Value, json};

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

use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// Parsed content from a Markdown file with TOML frontmatter.
#[derive(Debug, Clone)]
pub struct ParsedContent {
    /// Standard frontmatter fields (title, date, etc.)
    pub frontmatter: toml::Value,
    /// Explicit RDF fields from `[rdf.custom]` table
    pub rdf_fields: HashMap<String, JsonValue>,
    /// Rendered HTML from Markdown body
    pub html: String,
    /// Raw Markdown source (without frontmatter)
    pub raw_markdown: String,
}

/// Split a file's content into TOML frontmatter and Markdown body.
/// Frontmatter is delimited by `+++` lines.
pub fn split_frontmatter(
    content: &str,
) -> std::result::Result<(&str, &str), Box<dyn std::error::Error>> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("+++") {
        return Err("Content does not start with +++ frontmatter delimiter".into());
    }

    let after_first = &trimmed[3..];
    let after_first = after_first
        .strip_prefix("\r\n")
        .or_else(|| after_first.strip_prefix('\n'))
        .unwrap_or(after_first);

    // Handle both \n+++ and \r\n+++ (Windows line endings)
    let (end, delim_len) = after_first
        .find("\r\n+++")
        .map(|i| (i, 5))
        .or_else(|| after_first.find("\n+++").map(|i| (i, 4)))
        .ok_or("Missing closing +++ frontmatter delimiter")?;

    let frontmatter = &after_first[..end];
    let body = &after_first[end + delim_len..];
    let body = body
        .strip_prefix("\r\n")
        .or_else(|| body.strip_prefix('\n'))
        .unwrap_or(body);

    Ok((frontmatter, body))
}

/// Parsed frontmatter: (full TOML value, [rdf.custom] fields, [data] fields).
pub type FrontmatterResult = (
    toml::Value,
    HashMap<String, JsonValue>,
    HashMap<String, JsonValue>,
);

/// Parse TOML frontmatter, extracting standard fields, `[rdf.custom]` entries,
/// and `[data]` entries for linked data with friendly names.
pub fn parse_frontmatter(
    toml_str: &str,
) -> std::result::Result<FrontmatterResult, Box<dyn std::error::Error>> {
    let value: toml::Value = toml::from_str(toml_str)?;

    let mut rdf_fields = HashMap::new();

    if let Some(rdf_table) = value
        .get("rdf")
        .and_then(|r| r.get("custom"))
        .and_then(|c| c.as_table())
    {
        for (k, v) in rdf_table {
            rdf_fields.insert(k.clone(), toml_to_json(v));
        }
    }

    let mut data_fields = HashMap::new();
    if let Some(data_table) = value.get("data").and_then(|d| d.as_table()) {
        for (k, v) in data_table {
            data_fields.insert(k.clone(), toml_to_json(v));
        }
    }

    Ok((value, rdf_fields, data_fields))
}

pub fn toml_to_json(v: &toml::Value) -> JsonValue {
    match v {
        toml::Value::String(s) => JsonValue::String(s.clone()),
        toml::Value::Integer(i) => JsonValue::Number((*i).into()),
        toml::Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null),
        toml::Value::Boolean(b) => JsonValue::Bool(*b),
        toml::Value::Array(arr) => JsonValue::Array(arr.iter().map(toml_to_json).collect()),
        toml::Value::Table(t) => {
            let map: serde_json::Map<String, JsonValue> = t
                .iter()
                .map(|(k, v)| (k.clone(), toml_to_json(v)))
                .collect();
            JsonValue::Object(map)
        }
        toml::Value::Datetime(d) => JsonValue::String(d.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_basic_frontmatter() {
        let input = "+++\ntitle = \"Hello\"\n+++\n# Hello\nBody text";
        let (fm, body) = split_frontmatter(input).unwrap();
        assert_eq!(fm, "title = \"Hello\"");
        assert_eq!(body, "# Hello\nBody text");
    }

    #[test]
    fn split_windows_line_endings() {
        let input = "+++\r\ntitle = \"Hello\"\r\n+++\r\n# Hello\r\nBody text";
        let (fm, body) = split_frontmatter(input).unwrap();
        assert_eq!(fm, "title = \"Hello\"");
        assert_eq!(body, "# Hello\r\nBody text");
    }

    #[test]
    fn split_no_frontmatter_errors() {
        let input = "# Just Markdown";
        assert!(split_frontmatter(input).is_err());
    }

    #[test]
    fn parse_frontmatter_with_rdf_custom() {
        let toml_str = r#"
            title = "Test"
            [rdf.custom]
            "schema:wordCount" = 1500
        "#;
        let (value, rdf, data) = parse_frontmatter(toml_str).unwrap();
        assert_eq!(value.get("title").unwrap().as_str(), Some("Test"));
        assert_eq!(rdf["schema:wordCount"], JsonValue::Number(1500.into()));
        assert!(data.is_empty());
    }

    #[test]
    fn parse_frontmatter_with_data_section() {
        let toml_str = r#"
            title = "Test"
            [data]
            wordCount = 1500
            category = "tutorial"
            featured = true
        "#;
        let (value, rdf, data) = parse_frontmatter(toml_str).unwrap();
        assert_eq!(value.get("title").unwrap().as_str(), Some("Test"));
        assert!(rdf.is_empty());
        assert_eq!(data["wordCount"], JsonValue::Number(1500.into()));
        assert_eq!(data["category"], JsonValue::String("tutorial".into()));
        assert_eq!(data["featured"], JsonValue::Bool(true));
    }
}

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// A parsed DTCG design token file. Groups and tokens are interleaved
/// in the same tree — a node with `$value` is a token, without is a group.
#[derive(Debug, Clone, Default)]
pub struct DesignTokens {
    pub entries: BTreeMap<String, TokenNode>,
}

/// A node in the token tree: either a group (contains children) or a token (has $value).
#[derive(Debug, Clone)]
pub enum TokenNode {
    Token(Token),
    Group(TokenGroup),
}

/// A single design token with a value and optional metadata.
#[derive(Debug, Clone)]
pub struct Token {
    pub value: TokenValue,
    pub token_type: Option<String>,
    pub description: Option<String>,
    pub extensions: Option<Value>,
}

/// A group of tokens or sub-groups, optionally with a shared type.
#[derive(Debug, Clone)]
pub struct TokenGroup {
    pub group_type: Option<String>,
    pub description: Option<String>,
    pub children: BTreeMap<String, TokenNode>,
}

/// The value of a design token — can be a primitive, a reference, or a composite.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TokenValue {
    String(String),
    Number(f64),
    Bool(bool),
    Array(Vec<Value>),
    Object(CompositeValue),
}

/// A composite token value (dimension, typography, shadow, border, etc.).
pub type CompositeValue = BTreeMap<String, Value>;

fn parse_node(value: &Value) -> Option<TokenNode> {
    let obj = value.as_object()?;

    if obj.contains_key("$value") {
        let token_value: TokenValue = serde_json::from_value(obj["$value"].clone()).ok()?;
        Some(TokenNode::Token(Token {
            value: token_value,
            token_type: obj.get("$type").and_then(|v| v.as_str()).map(String::from),
            description: obj
                .get("$description")
                .and_then(|v| v.as_str())
                .map(String::from),
            extensions: obj.get("$extensions").cloned(),
        }))
    } else {
        let mut children = BTreeMap::new();
        for (key, val) in obj {
            if key.starts_with('$') {
                continue;
            }
            if let Some(node) = parse_node(val) {
                children.insert(key.clone(), node);
            }
        }
        Some(TokenNode::Group(TokenGroup {
            group_type: obj.get("$type").and_then(|v| v.as_str()).map(String::from),
            description: obj
                .get("$description")
                .and_then(|v| v.as_str())
                .map(String::from),
            children,
        }))
    }
}

impl DesignTokens {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let raw: Value = serde_json::from_str(json)?;
        let obj = raw.as_object().cloned().unwrap_or_default();

        let mut entries = BTreeMap::new();
        for (key, val) in &obj {
            if key.starts_with('$') {
                continue;
            }
            if let Some(node) = parse_node(val) {
                entries.insert(key.clone(), node);
            }
        }

        Ok(Self { entries })
    }

    pub fn from_file(path: &camino::Utf8Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::from_json(&content)?)
    }

    /// Flatten the token tree into a map of dot-separated paths to resolved tokens.
    /// Group `$type` is inherited by children that don't specify their own.
    pub fn flatten(&self) -> BTreeMap<String, FlatToken> {
        let mut result = BTreeMap::new();
        flatten_nodes(&self.entries, "", None, &mut result);
        result
    }
}

/// A flattened token with its full path and resolved type.
#[derive(Debug, Clone)]
pub struct FlatToken {
    pub path: String,
    pub value: TokenValue,
    pub token_type: Option<String>,
    pub description: Option<String>,
}

fn flatten_nodes(
    entries: &BTreeMap<String, TokenNode>,
    prefix: &str,
    inherited_type: Option<&str>,
    out: &mut BTreeMap<String, FlatToken>,
) {
    for (key, node) in entries {
        if key.starts_with('$') {
            continue;
        }
        let raw_path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        // Strip trailing "._" (Style Dictionary root token convention where "_" represents the group-level value)
        let path = if raw_path.ends_with("._") {
            raw_path[..raw_path.len() - 2].to_string()
        } else if key == "_" && !prefix.is_empty() {
            prefix.to_string()
        } else {
            raw_path
        };

        match node {
            TokenNode::Token(token) => {
                let token_type = token
                    .token_type
                    .as_deref()
                    .or(inherited_type)
                    .map(|s| s.to_string());
                out.insert(
                    path.clone(),
                    FlatToken {
                        path,
                        value: token.value.clone(),
                        token_type,
                        description: token.description.clone(),
                    },
                );
            }
            TokenNode::Group(group) => {
                let group_type = group.group_type.as_deref().or(inherited_type);
                flatten_nodes(&group.children, &path, group_type, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_tokens() {
        let json = r##"{
            "color": {
                "$type": "color",
                "primary": { "$value": "#0066cc", "$description": "Brand color" },
                "text": { "$value": "#1a1a1a" }
            }
        }"##;

        let tokens = DesignTokens::from_json(json).unwrap();
        let flat = tokens.flatten();

        assert_eq!(flat.len(), 2);
        assert!(flat.contains_key("color.primary"));
        assert!(flat.contains_key("color.text"));
        assert_eq!(flat["color.primary"].token_type.as_deref(), Some("color"));
        assert_eq!(flat["color.text"].token_type.as_deref(), Some("color"));
    }

    #[test]
    fn parse_composite_tokens() {
        let json = r##"{
            "spacing": {
                "$type": "dimension",
                "md": { "$value": { "value": 16, "unit": "px" } }
            }
        }"##;

        let tokens = DesignTokens::from_json(json).unwrap();
        let flat = tokens.flatten();

        assert_eq!(flat.len(), 1);
        assert_eq!(flat["spacing.md"].token_type.as_deref(), Some("dimension"));
    }

    #[test]
    fn type_inheritance_from_group() {
        let json = r##"{
            "color": {
                "$type": "color",
                "primary": { "$value": "#0066cc" },
                "secondary": { "$value": "#ff6b35", "$type": "color" }
            }
        }"##;

        let tokens = DesignTokens::from_json(json).unwrap();
        let flat = tokens.flatten();

        assert_eq!(flat["color.primary"].token_type.as_deref(), Some("color"));
        assert_eq!(flat["color.secondary"].token_type.as_deref(), Some("color"));
    }

    #[test]
    fn nested_groups() {
        let json = r##"{
            "color": {
                "$type": "color",
                "neutral": {
                    "100": { "$value": "#f5f5f5" },
                    "900": { "$value": "#1a1a1a" }
                }
            }
        }"##;

        let tokens = DesignTokens::from_json(json).unwrap();
        let flat = tokens.flatten();

        assert!(flat.contains_key("color.neutral.100"));
        assert!(flat.contains_key("color.neutral.900"));
    }

    #[test]
    fn description_preserved() {
        let json = r##"{
            "color": {
                "primary": { "$value": "#0066cc", "$description": "Main brand color" }
            }
        }"##;

        let tokens = DesignTokens::from_json(json).unwrap();
        let flat = tokens.flatten();

        assert_eq!(
            flat["color.primary"].description.as_deref(),
            Some("Main brand color")
        );
    }

    #[test]
    fn tokens_with_extra_fields() {
        let json = r##"{
            "border": {
                "width": {
                    "$type": "dimension",
                    "sm": {
                        "$value": "1px",
                        "$type": "dimension",
                        "original": { "$value": "1px", "attributes": { "category": "border" } },
                        "attributes": { "category": "border", "type": "width" },
                        "name": "rh-border-width-sm",
                        "path": ["border", "width", "sm"]
                    }
                }
            }
        }"##;

        let tokens = DesignTokens::from_json(json).unwrap();
        let flat = tokens.flatten();

        assert!(flat.contains_key("border.width.sm"));
        match &flat["border.width.sm"].value {
            TokenValue::String(s) => assert_eq!(s, "1px"),
            _ => panic!("expected string"),
        }
    }
}

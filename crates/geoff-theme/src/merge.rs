use serde_json::Value;

/// Deep-merge child token JSON over parent token JSON.
/// Child values override parent values at the same path.
/// Parent values not present in child are preserved.
pub fn merge_tokens(parent: &Value, child: &Value) -> Value {
    match (parent, child) {
        (Value::Object(p), Value::Object(c)) => {
            let mut merged = p.clone();
            for (key, child_val) in c {
                let merged_val = if let Some(parent_val) = p.get(key) {
                    merge_tokens(parent_val, child_val)
                } else {
                    child_val.clone()
                };
                merged.insert(key.clone(), merged_val);
            }
            Value::Object(merged)
        }
        (_, child) => child.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn child_overrides_value() {
        let parent = json!({
            "color": {
                "$type": "color",
                "primary": { "$value": "#0066cc" },
                "text": { "$value": "#1a1a1a" }
            }
        });

        let child = json!({
            "color": {
                "primary": { "$value": "#ee0000" }
            }
        });

        let merged = merge_tokens(&parent, &child);

        assert_eq!(merged["color"]["primary"]["$value"], "#ee0000");
        assert_eq!(merged["color"]["text"]["$value"], "#1a1a1a");
        assert_eq!(merged["color"]["$type"], "color");
    }

    #[test]
    fn child_adds_new_token() {
        let parent = json!({
            "color": {
                "primary": { "$value": "#0066cc" }
            }
        });

        let child = json!({
            "color": {
                "accent": { "$value": "#ff6b35" }
            }
        });

        let merged = merge_tokens(&parent, &child);

        assert_eq!(merged["color"]["primary"]["$value"], "#0066cc");
        assert_eq!(merged["color"]["accent"]["$value"], "#ff6b35");
    }

    #[test]
    fn child_adds_new_group() {
        let parent = json!({
            "color": { "primary": { "$value": "#0066cc" } }
        });

        let child = json!({
            "spacing": { "md": { "$value": { "value": 16, "unit": "px" } } }
        });

        let merged = merge_tokens(&parent, &child);

        assert_eq!(merged["color"]["primary"]["$value"], "#0066cc");
        assert_eq!(merged["spacing"]["md"]["$value"]["value"], 16);
    }

    #[test]
    fn three_level_inheritance() {
        let grandparent = json!({
            "color": {
                "primary": { "$value": "#111" },
                "secondary": { "$value": "#222" },
                "accent": { "$value": "#333" }
            }
        });

        let parent = json!({
            "color": {
                "primary": { "$value": "#aaa" }
            }
        });

        let child = json!({
            "color": {
                "secondary": { "$value": "#bbb" }
            }
        });

        let merged = merge_tokens(&merge_tokens(&grandparent, &parent), &child);

        assert_eq!(merged["color"]["primary"]["$value"], "#aaa");
        assert_eq!(merged["color"]["secondary"]["$value"], "#bbb");
        assert_eq!(merged["color"]["accent"]["$value"], "#333");
    }

    #[test]
    fn empty_child_preserves_parent() {
        let parent = json!({
            "color": { "primary": { "$value": "#0066cc" } }
        });

        let child = json!({});

        let merged = merge_tokens(&parent, &child);
        assert_eq!(merged["color"]["primary"]["$value"], "#0066cc");
    }
}

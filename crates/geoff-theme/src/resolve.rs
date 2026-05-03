use std::collections::BTreeMap;

use crate::tokens::{FlatToken, TokenValue};

/// Resolve `{reference}` strings in token values to their target values.
/// References use curly-brace syntax: `"{color.primary}"` resolves to the
/// value of the token at path `color.primary`.
pub fn resolve_references(tokens: &mut BTreeMap<String, FlatToken>) {
    let snapshot: BTreeMap<String, TokenValue> = tokens
        .iter()
        .map(|(k, t)| (k.clone(), t.value.clone()))
        .collect();

    for token in tokens.values_mut() {
        token.value = resolve_value(&token.value, &snapshot, 0);
    }
}

fn resolve_value(
    value: &TokenValue,
    all: &BTreeMap<String, TokenValue>,
    depth: usize,
) -> TokenValue {
    if depth > 20 {
        return value.clone();
    }

    match value {
        TokenValue::String(s) if is_reference(s) => {
            let ref_path = &s[1..s.len() - 1];
            if let Some(target) = all.get(ref_path) {
                resolve_value(target, all, depth + 1)
            } else {
                value.clone()
            }
        }
        _ => value.clone(),
    }
}

fn is_reference(s: &str) -> bool {
    s.starts_with('{') && s.ends_with('}') && s.len() > 2 && !s[1..s.len() - 1].contains('{')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::DesignTokens;

    #[test]
    fn resolve_simple_reference() {
        let json = r##"{
            "blue": { "$value": "#0066cc", "$type": "color" },
            "primary": { "$value": "{blue}", "$type": "color" }
        }"##;

        let tokens = DesignTokens::from_json(json).unwrap();
        let mut flat = tokens.flatten();
        resolve_references(&mut flat);

        match &flat["primary"].value {
            TokenValue::String(s) => assert_eq!(s, "#0066cc"),
            _ => panic!("expected string"),
        }
    }

    #[test]
    fn resolve_nested_reference() {
        let json = r##"{
            "blue": { "$value": "#0066cc", "$type": "color" },
            "primary": { "$value": "{blue}", "$type": "color" },
            "button-bg": { "$value": "{primary}", "$type": "color" }
        }"##;

        let tokens = DesignTokens::from_json(json).unwrap();
        let mut flat = tokens.flatten();
        resolve_references(&mut flat);

        match &flat["button-bg"].value {
            TokenValue::String(s) => assert_eq!(s, "#0066cc"),
            _ => panic!("expected string"),
        }
    }

    #[test]
    fn unresolvable_reference_kept_as_is() {
        let json = r##"{
            "primary": { "$value": "{nonexistent}", "$type": "color" }
        }"##;

        let tokens = DesignTokens::from_json(json).unwrap();
        let mut flat = tokens.flatten();
        resolve_references(&mut flat);

        match &flat["primary"].value {
            TokenValue::String(s) => assert_eq!(s, "{nonexistent}"),
            _ => panic!("expected string"),
        }
    }

    #[test]
    fn non_reference_string_unchanged() {
        let json = r##"{
            "color": { "$value": "#ff0000", "$type": "color" }
        }"##;

        let tokens = DesignTokens::from_json(json).unwrap();
        let mut flat = tokens.flatten();
        resolve_references(&mut flat);

        match &flat["color"].value {
            TokenValue::String(s) => assert_eq!(s, "#ff0000"),
            _ => panic!("expected string"),
        }
    }

    #[test]
    fn circular_reference_terminates() {
        let json = r##"{
            "a": { "$value": "{b}", "$type": "color" },
            "b": { "$value": "{a}", "$type": "color" }
        }"##;

        let tokens = DesignTokens::from_json(json).unwrap();
        let mut flat = tokens.flatten();
        resolve_references(&mut flat);
        // Should not infinite loop — depth limit stops it
    }

    #[test]
    fn reference_in_group_path() {
        let json = r##"{
            "color": {
                "$type": "color",
                "base": { "$value": "#0066cc" },
                "primary": { "$value": "{color.base}" }
            }
        }"##;

        let tokens = DesignTokens::from_json(json).unwrap();
        let mut flat = tokens.flatten();
        resolve_references(&mut flat);

        match &flat["color.primary"].value {
            TokenValue::String(s) => assert_eq!(s, "#0066cc"),
            _ => panic!("expected string"),
        }
    }
}

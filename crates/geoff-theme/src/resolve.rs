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

/// Resolve `{reference}` strings in token values, checking a base token set first.
/// This lets theme tokens reference design system tokens.
pub fn resolve_references_with_base(
    tokens: &mut BTreeMap<String, FlatToken>,
    base: &BTreeMap<String, FlatToken>,
) {
    // Build a combined lookup: base values + local values (local overrides base)
    let mut all: BTreeMap<String, TokenValue> = base
        .iter()
        .map(|(k, t)| (k.clone(), t.value.clone()))
        .collect();
    for (k, t) in tokens.iter() {
        all.insert(k.clone(), t.value.clone());
    }

    for token in tokens.values_mut() {
        token.value = resolve_value(&token.value, &all, 0);
    }
}

pub(crate) fn is_reference(s: &str) -> bool {
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

    #[test]
    fn resolve_with_base_tokens() {
        let base_json = r##"{
            "color": {
                "$type": "color",
                "red": { "$value": "#e00" },
                "blue": { "$value": "#06c" }
            }
        }"##;
        let theme_json = r##"{
            "color": {
                "$type": "color",
                "primary": { "$value": "{color.red}" },
                "secondary": { "$value": "{color.blue}" }
            }
        }"##;

        let base = DesignTokens::from_json(base_json).unwrap();
        let base_flat = base.flatten();
        let theme = DesignTokens::from_json(theme_json).unwrap();
        let mut theme_flat = theme.flatten();

        resolve_references_with_base(&mut theme_flat, &base_flat);

        match &theme_flat["color.primary"].value {
            TokenValue::String(s) => assert_eq!(s, "#e00"),
            _ => panic!("expected string"),
        }
        match &theme_flat["color.secondary"].value {
            TokenValue::String(s) => assert_eq!(s, "#06c"),
            _ => panic!("expected string"),
        }
    }
}

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
            // Standalone reference: resolve to the target value entirely
            let ref_path = &s[1..s.len() - 1];
            if let Some(target) = all.get(ref_path) {
                resolve_value(target, all, depth + 1)
            } else {
                value.clone()
            }
        }
        TokenValue::String(s) if contains_reference(s) => {
            // Inline references within a larger string (e.g. light-dark(), color-mix(), calc())
            TokenValue::String(resolve_inline_refs(s, all, depth))
        }
        _ => value.clone(),
    }
}

/// Replace all `{reference}` occurrences within a string with their resolved values.
fn resolve_inline_refs(s: &str, all: &BTreeMap<String, TokenValue>, depth: usize) -> String {
    let mut result = String::with_capacity(s.len());
    let mut pos = 0;
    let bytes = s.as_bytes();

    while pos < s.len() {
        if bytes[pos] == b'{'
            && let Some(end) = s[pos + 1..].find('}')
            && !s[pos + 1..pos + 1 + end].contains('{')
            && !s[pos + 1..pos + 1 + end].is_empty()
        {
            let ref_path = &s[pos + 1..pos + 1 + end];
            if let Some(target) = all.get(ref_path) {
                let resolved = resolve_value(target, all, depth + 1);
                match resolved {
                    TokenValue::String(v) => result.push_str(&v),
                    TokenValue::Number(n) => {
                        if n.fract() == 0.0 {
                            result.push_str(&format!("{}", n as i64));
                        } else {
                            result.push_str(&n.to_string());
                        }
                    }
                    _ => {
                        result.push('{');
                        result.push_str(ref_path);
                        result.push('}');
                    }
                }
                pos = pos + 1 + end + 1;
                continue;
            }
        }
        result.push(bytes[pos] as char);
        pos += 1;
    }
    result
}

/// Check if a string contains any `{reference}` pattern (but is not itself a standalone reference).
fn contains_reference(s: &str) -> bool {
    !is_reference(s) && s.contains('{') && s.contains('}')
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

/// An unresolved token reference found after resolution.
#[derive(Debug, Clone)]
pub struct UnresolvedRef {
    /// The token path that contains the broken reference.
    pub token_path: String,
    /// The reference string that couldn't be resolved (e.g. `{color.nonexistent}`).
    pub reference: String,
}

/// Scan resolved tokens for any remaining `{reference}` strings that weren't resolved.
/// Checks both standalone references and inline references within larger strings.
pub fn find_unresolved(tokens: &BTreeMap<String, FlatToken>) -> Vec<UnresolvedRef> {
    let mut errors = Vec::new();
    for (path, token) in tokens {
        if let TokenValue::String(s) = &token.value {
            if is_reference(s) {
                errors.push(UnresolvedRef {
                    token_path: path.clone(),
                    reference: s.clone(),
                });
            } else {
                // Scan for inline {reference} patterns that survived resolution
                let mut pos = 0;
                let bytes = s.as_bytes();
                while pos < s.len() {
                    if bytes[pos] == b'{'
                        && let Some(end) = s[pos + 1..].find('}')
                    {
                        let ref_path = &s[pos + 1..pos + 1 + end];
                        if !ref_path.contains('{') && !ref_path.is_empty() {
                            errors.push(UnresolvedRef {
                                token_path: path.clone(),
                                reference: format!("{{{ref_path}}}"),
                            });
                            pos = pos + 1 + end + 1;
                            continue;
                        }
                    }
                    pos += 1;
                }
            }
        }
    }
    errors
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
    fn find_unresolved_detects_broken_refs() {
        let json = r##"{
            "color": {
                "$type": "color",
                "primary": { "$value": "{color.nonexistent}" },
                "valid": { "$value": "#ff0000" }
            }
        }"##;
        let tokens = DesignTokens::from_json(json).unwrap();
        let mut flat = tokens.flatten();
        resolve_references(&mut flat);

        let errors = find_unresolved(&flat);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].token_path, "color.primary");
        assert_eq!(errors[0].reference, "{color.nonexistent}");
    }

    #[test]
    fn find_unresolved_empty_when_all_resolved() {
        let json = r##"{
            "blue": { "$value": "#0066cc", "$type": "color" },
            "primary": { "$value": "{blue}", "$type": "color" }
        }"##;
        let tokens = DesignTokens::from_json(json).unwrap();
        let mut flat = tokens.flatten();
        resolve_references(&mut flat);

        let errors = find_unresolved(&flat);
        assert!(errors.is_empty());
    }

    #[test]
    fn find_unresolved_cross_file() {
        let base_json = r##"{
            "color": { "$type": "color", "red": { "$value": "#e00" } }
        }"##;
        let theme_json = r##"{
            "brand": { "$type": "color", "primary": { "$value": "{color.missing}" } }
        }"##;

        let base = DesignTokens::from_json(base_json).unwrap();
        let base_flat = base.flatten();
        let theme = DesignTokens::from_json(theme_json).unwrap();
        let mut theme_flat = theme.flatten();
        resolve_references_with_base(&mut theme_flat, &base_flat);

        let errors = find_unresolved(&theme_flat);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].token_path, "brand.primary");
        assert_eq!(errors[0].reference, "{color.missing}");
    }

    #[test]
    fn resolve_light_dark_refs_with_base() {
        let base_json = r##"{
            "surface-critical": {
                "background": {
                    "on": {
                        "$type": "color",
                        "light": { "$value": "#ffffff" },
                        "dark": { "$value": "#1a1a1a" }
                    }
                }
            }
        }"##;
        let theme_json = r##"{
            "surface-critical": {
                "background": {
                    "$type": "color",
                    "$value": "light-dark({surface-critical.background.on.light}, {surface-critical.background.on.dark})"
                }
            }
        }"##;

        let base = DesignTokens::from_json(base_json).unwrap();
        let base_flat = base.flatten();
        let theme = DesignTokens::from_json(theme_json).unwrap();
        let mut theme_flat = theme.flatten();

        resolve_references_with_base(&mut theme_flat, &base_flat);

        match &theme_flat["surface-critical.background"].value {
            TokenValue::String(s) => {
                assert_eq!(s, "light-dark(#ffffff, #1a1a1a)");
            }
            other => panic!("expected string, got {other:?}"),
        }

        let errors = find_unresolved(&theme_flat);
        assert!(
            errors.is_empty(),
            "should have no unresolved refs: {errors:?}"
        );
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

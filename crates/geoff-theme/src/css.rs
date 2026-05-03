use std::collections::BTreeMap;
use std::fmt::Write;

use serde_json::Value;

use crate::tokens::{FlatToken, TokenValue};

/// Generate CSS custom properties from a flat token map.
/// If `dark_tokens` is provided, tokens differing between light and dark
/// use the `light-dark()` function with `-on-light` / `-on-dark` primitives.
///
/// When `critical_only` is true, only tokens whose path contains "-critical" are included.
pub fn generate_css(
    tokens: &BTreeMap<String, FlatToken>,
    dark_tokens: Option<&BTreeMap<String, FlatToken>>,
    critical_only: bool,
) -> String {
    generate_css_with_prefix(tokens, dark_tokens, critical_only, None)
}

pub fn generate_css_with_prefix(
    tokens: &BTreeMap<String, FlatToken>,
    dark_tokens: Option<&BTreeMap<String, FlatToken>>,
    critical_only: bool,
    prefix: Option<&str>,
) -> String {
    let mut css = String::new();

    if let Some(dark) = dark_tokens {
        generate_light_dark(&mut css, tokens, dark, critical_only, prefix);
    } else {
        for (path, token) in tokens {
            if critical_only && !path.contains("-critical") {
                continue;
            }
            if !critical_only && path.contains("-critical") {
                continue;
            }
            write_token_vars(
                &mut css,
                path,
                &token.value,
                token.token_type.as_deref(),
                prefix,
            );
        }
    }

    css
}

fn generate_light_dark(
    css: &mut String,
    light: &BTreeMap<String, FlatToken>,
    dark: &BTreeMap<String, FlatToken>,
    critical_only: bool,
    prefix: Option<&str>,
) {
    for (path, light_token) in light {
        if critical_only && !path.contains("-critical") {
            continue;
        }
        if !critical_only && path.contains("-critical") {
            continue;
        }

        if let Some(dark_token) = dark.get(path) {
            let light_val =
                token_to_css_value(&light_token.value, light_token.token_type.as_deref());
            let dark_val = token_to_css_value(&dark_token.value, dark_token.token_type.as_deref());

            if light_val == dark_val {
                write_token_vars(
                    css,
                    path,
                    &light_token.value,
                    light_token.token_type.as_deref(),
                    prefix,
                );
            } else {
                let var_name = path_to_var_name_with_prefix(path, prefix);
                let _ = writeln!(css, "  {var_name}-on-light: {light_val};");
                let _ = writeln!(css, "  {var_name}-on-dark: {dark_val};");
                let _ = writeln!(
                    css,
                    "  {var_name}: light-dark(var({var_name}-on-light), var({var_name}-on-dark));"
                );
            }
        } else {
            write_token_vars(
                css,
                path,
                &light_token.value,
                light_token.token_type.as_deref(),
                prefix,
            );
        }
    }

    for (path, dark_token) in dark {
        if !light.contains_key(path) {
            if critical_only && !path.contains("-critical") {
                continue;
            }
            if !critical_only && path.contains("-critical") {
                continue;
            }
            let var_name = path_to_var_name_with_prefix(path, prefix);
            let val = token_to_css_value(&dark_token.value, dark_token.token_type.as_deref());
            let _ = writeln!(css, "  {var_name}-on-dark: {val};");
        }
    }
}

fn write_token_vars(
    css: &mut String,
    path: &str,
    value: &TokenValue,
    token_type: Option<&str>,
    prefix: Option<&str>,
) {
    match (value, token_type) {
        (TokenValue::Object(obj), Some("typography")) => {
            write_composite_vars(css, path, obj, prefix);
        }
        (TokenValue::Object(obj), Some("shadow")) => {
            let shorthand = shadow_to_shorthand(obj);
            let var_name = path_to_var_name_with_prefix(path, prefix);
            let _ = writeln!(css, "  {var_name}: {shorthand};");
        }
        (TokenValue::Object(obj), Some("border")) => {
            let shorthand = border_to_shorthand(obj);
            let var_name = path_to_var_name_with_prefix(path, prefix);
            let _ = writeln!(css, "  {var_name}: {shorthand};");
        }
        (TokenValue::Array(arr), Some("shadow")) => {
            let parts: Vec<String> = arr
                .iter()
                .filter_map(|v| {
                    v.as_object().map(|o| {
                        let map: BTreeMap<String, Value> =
                            o.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                        shadow_to_shorthand(&map)
                    })
                })
                .collect();
            let var_name = path_to_var_name_with_prefix(path, prefix);
            let _ = writeln!(css, "  {var_name}: {};", parts.join(", "));
        }
        _ => {
            let var_name = path_to_var_name_with_prefix(path, prefix);
            let val = token_to_css_value(value, token_type);
            let _ = writeln!(css, "  {var_name}: {val};");
        }
    }
}

fn write_composite_vars(
    css: &mut String,
    path: &str,
    obj: &BTreeMap<String, Value>,
    prefix: Option<&str>,
) {
    for (key, val) in obj {
        let sub_path = format!("{path}.{key}");
        let var_name = path_to_var_name_with_prefix(&sub_path, prefix);
        let css_val = json_to_css_value(val);
        let _ = writeln!(css, "  {var_name}: {css_val};");
    }
}

fn shadow_to_shorthand(obj: &BTreeMap<String, Value>) -> String {
    let offset_x = dimension_val(obj.get("offsetX"));
    let offset_y = dimension_val(obj.get("offsetY"));
    let blur = dimension_val(obj.get("blur"));
    let spread = dimension_val(obj.get("spread"));
    let color = obj
        .get("color")
        .and_then(|v| v.as_str())
        .unwrap_or("transparent");
    format!("{offset_x} {offset_y} {blur} {spread} {color}")
}

fn border_to_shorthand(obj: &BTreeMap<String, Value>) -> String {
    let width = dimension_val(obj.get("width"));
    let style = obj.get("style").and_then(|v| v.as_str()).unwrap_or("solid");
    let color = obj
        .get("color")
        .and_then(|v| v.as_str())
        .unwrap_or("currentColor");
    format!("{width} {style} {color}")
}

fn dimension_val(val: Option<&Value>) -> String {
    match val {
        Some(Value::Object(obj)) => {
            let v = obj.get("value").and_then(|n| n.as_f64()).unwrap_or(0.0);
            let unit = obj.get("unit").and_then(|u| u.as_str()).unwrap_or("px");
            if v == 0.0 {
                "0".to_string()
            } else if v.fract() == 0.0 {
                format!("{}{unit}", v as i64)
            } else {
                format!("{v}{unit}")
            }
        }
        Some(Value::Number(n)) => {
            let v = n.as_f64().unwrap_or(0.0);
            if v == 0.0 {
                "0".to_string()
            } else {
                format!("{v}px")
            }
        }
        _ => "0".to_string(),
    }
}

fn token_to_css_value(value: &TokenValue, token_type: Option<&str>) -> String {
    match value {
        TokenValue::String(s) => s.clone(),
        TokenValue::Number(n) => {
            if n.fract() == 0.0 {
                format!("{}", *n as i64)
            } else {
                format!("{n}")
            }
        }
        TokenValue::Bool(b) => b.to_string(),
        TokenValue::Object(obj) => match token_type {
            Some("dimension") => dimension_val(Some(&Value::Object(
                obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            ))),
            Some("duration") => {
                let v = obj.get("value").and_then(|n| n.as_f64()).unwrap_or(0.0);
                let unit = obj.get("unit").and_then(|u| u.as_str()).unwrap_or("ms");
                format!("{v}{unit}")
            }
            Some("shadow") => shadow_to_shorthand(obj),
            Some("border") => border_to_shorthand(obj),
            _ => serde_json::to_string(obj).unwrap_or_default(),
        },
        TokenValue::Array(arr) => match token_type {
            Some("fontFamily") => arr
                .iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect::<Vec<_>>()
                .join(", "),
            Some("cubicBezier") => {
                let nums: Vec<String> = arr
                    .iter()
                    .filter_map(|v| v.as_f64().map(|n| format!("{n}")))
                    .collect();
                format!("cubic-bezier({})", nums.join(", "))
            }
            _ => serde_json::to_string(arr).unwrap_or_default(),
        },
    }
}

fn json_to_css_value(val: &Value) -> String {
    match val {
        Value::String(s) => s.clone(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                format!("{i}")
            } else {
                format!("{}", n.as_f64().unwrap_or(0.0))
            }
        }
        Value::Object(obj) => dimension_val(Some(&Value::Object(obj.clone()))),
        Value::Array(arr) => arr
            .iter()
            .map(json_to_css_value)
            .collect::<Vec<_>>()
            .join(", "),
        Value::Bool(b) => b.to_string(),
        Value::Null => "none".to_string(),
    }
}

fn path_to_var_name(raw_path: &str) -> String {
    path_to_var_name_with_prefix(raw_path, None)
}

fn path_to_var_name_with_prefix(raw_path: &str, prefix: Option<&str>) -> String {
    // Strip trailing "._" (Style Dictionary root token convention)
    let path = raw_path.strip_suffix("._").unwrap_or(raw_path);
    let kebab = path
        .chars()
        .map(|c| match c {
            '.' => '-',
            c if c.is_uppercase() => {
                // camelCase to kebab-case
                format!("-{}", c.to_lowercase())
                    .chars()
                    .collect::<String>()
                    .chars()
                    .next()
                    .unwrap_or(c)
            }
            _ => c,
        })
        .collect::<String>();

    // Handle camelCase properly
    let mut result = String::with_capacity(kebab.len() + 10);
    result.push_str("--");
    if let Some(pfx) = prefix {
        result.push_str(pfx);
        result.push('-');
    }
    let chars: Vec<char> = path.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c == '.' {
            result.push('-');
        } else if c.is_uppercase() && i > 0 && chars[i - 1] != '.' {
            result.push('-');
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c.to_ascii_lowercase());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::DesignTokens;

    #[test]
    fn simple_color_tokens() {
        let json = r##"{
            "color": {
                "$type": "color",
                "primary": { "$value": "#0066cc" },
                "text": { "$value": "#1a1a1a" }
            }
        }"##;

        let tokens = DesignTokens::from_json(json).unwrap();
        let flat = tokens.flatten();
        let css = generate_css(&flat, None, false);

        assert!(css.contains("--color-primary: #0066cc;"));
        assert!(css.contains("--color-text: #1a1a1a;"));
    }

    #[test]
    fn dimension_tokens() {
        let json = r##"{
            "spacing": {
                "$type": "dimension",
                "md": { "$value": { "value": 16, "unit": "px" } },
                "lg": { "$value": { "value": 1.5, "unit": "rem" } }
            }
        }"##;

        let tokens = DesignTokens::from_json(json).unwrap();
        let flat = tokens.flatten();
        let css = generate_css(&flat, None, false);

        assert!(css.contains("--spacing-md: 16px;"));
        assert!(css.contains("--spacing-lg: 1.5rem;"));
    }

    #[test]
    fn typography_composite_expands() {
        let json = r##"{
            "typography": {
                "body": {
                    "$type": "typography",
                    "$value": {
                        "fontFamily": "system-ui",
                        "fontSize": { "value": 16, "unit": "px" },
                        "fontWeight": 400,
                        "lineHeight": 1.5
                    }
                }
            }
        }"##;

        let tokens = DesignTokens::from_json(json).unwrap();
        let flat = tokens.flatten();
        let css = generate_css(&flat, None, false);

        assert!(css.contains("--typography-body-font-family: system-ui;"));
        assert!(css.contains("--typography-body-font-size: 16px;"));
        assert!(css.contains("--typography-body-font-weight: 400;"));
        assert!(css.contains("--typography-body-line-height: 1.5;"));
    }

    #[test]
    fn shadow_shorthand() {
        let json = r##"{
            "shadow": {
                "default": {
                    "$type": "shadow",
                    "$value": {
                        "color": "#00000014",
                        "offsetX": { "value": 0, "unit": "px" },
                        "offsetY": { "value": 2, "unit": "px" },
                        "blur": { "value": 4, "unit": "px" },
                        "spread": { "value": 0, "unit": "px" }
                    }
                }
            }
        }"##;

        let tokens = DesignTokens::from_json(json).unwrap();
        let flat = tokens.flatten();
        let css = generate_css(&flat, None, false);

        assert!(css.contains("--shadow-default: 0 2px 4px 0 #00000014;"));
    }

    #[test]
    fn light_dark_mode() {
        let light_json = r##"{
            "color": {
                "$type": "color",
                "bg": { "$value": "#ffffff" },
                "text": { "$value": "#1a1a1a" }
            },
            "spacing": {
                "$type": "dimension",
                "md": { "$value": { "value": 16, "unit": "px" } }
            }
        }"##;

        let dark_json = r##"{
            "color": {
                "$type": "color",
                "bg": { "$value": "#1a1a1a" },
                "text": { "$value": "#f5f5f5" }
            }
        }"##;

        let light = DesignTokens::from_json(light_json).unwrap().flatten();
        let dark = DesignTokens::from_json(dark_json).unwrap().flatten();
        let css = generate_css(&light, Some(&dark), false);

        assert!(css.contains("--color-bg-on-light: #ffffff;"));
        assert!(css.contains("--color-bg-on-dark: #1a1a1a;"));
        assert!(css.contains(
            "--color-bg: light-dark(var(--color-bg-on-light), var(--color-bg-on-dark));"
        ));
        assert!(css.contains("--spacing-md: 16px;"));
        assert!(!css.contains("--spacing-md-on-light"));
    }

    #[test]
    fn critical_filter() {
        let json = r##"{
            "color-critical": {
                "$type": "color",
                "bg": { "$value": "#ffffff" }
            },
            "color": {
                "$type": "color",
                "accent": { "$value": "#ff6b35" }
            }
        }"##;

        let tokens = DesignTokens::from_json(json).unwrap();
        let flat = tokens.flatten();

        let critical = generate_css(&flat, None, true);
        let deferred = generate_css(&flat, None, false);

        assert!(critical.contains("--color-critical-bg: #ffffff;"));
        assert!(!critical.contains("--color-accent"));

        assert!(deferred.contains("--color-accent: #ff6b35;"));
        assert!(!deferred.contains("--color-critical-bg"));
    }

    #[test]
    fn camel_case_to_kebab() {
        assert_eq!(
            path_to_var_name("typography.body.fontSize"),
            "--typography-body-font-size"
        );
        assert_eq!(path_to_var_name("color.primary"), "--color-primary");
        assert_eq!(path_to_var_name("spacing.md"), "--spacing-md");
    }
}

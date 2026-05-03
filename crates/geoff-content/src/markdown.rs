use geoff_ontology::mappings::MappingRegistry;
use pulldown_cmark::{Options, Parser, html};

/// Render Markdown source to HTML using pulldown-cmark with GFM extensions.
pub fn render_markdown(source: &str) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;

    let parser = Parser::new_ext(source, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

/// Rewrite Markdown-generated `<a href="rdfa:...">text</a>` links into
/// `<span property="...">text</span>` for RDFa Lite 1.1 inline annotations.
///
/// Skips content inside `<code>` and `<pre>` tags.
pub fn rewrite_rdfa_links(html: &str, registry: &MappingRegistry) -> String {
    const MARKER: &str = "<a href=\"rdfa:";

    let mut result = String::with_capacity(html.len());
    let mut pos = 0;
    let mut in_pre = false;
    let mut in_code = false;

    while pos < html.len() {
        // Look for the next HTML tag from the current position.
        let remaining = &html[pos..];
        let next_tag = match remaining.find('<') {
            Some(offset) => offset,
            None => {
                // No more tags — copy rest and we're done.
                result.push_str(remaining);
                break;
            }
        };

        // Copy everything before the tag.
        result.push_str(&remaining[..next_tag]);
        pos += next_tag;
        let tag_start = &html[pos..];

        // Track <pre> and <code> state.
        if tag_start.starts_with("<pre") {
            in_pre = true;
        } else if tag_start.starts_with("</pre>") {
            in_pre = false;
        } else if tag_start.starts_with("<code") {
            in_code = true;
        } else if tag_start.starts_with("</code>") {
            in_code = false;
        }

        // Try to match our marker outside of code/pre blocks.
        if !in_pre && !in_code && tag_start.starts_with(MARKER) {
            let after_prefix = pos + MARKER.len();

            // Find the closing quote after the property name.
            if let Some(quote_offset) = html[after_prefix..].find('"') {
                let property_name = &html[after_prefix..after_prefix + quote_offset];

                // Find the '>' that ends the opening <a> tag.
                let after_quote = after_prefix + quote_offset;
                if let Some(gt_offset) = html[after_quote..].find('>') {
                    let content_start = after_quote + gt_offset + 1;

                    // Find the closing </a> tag.
                    if let Some(close_offset) = html[content_start..].find("</a>") {
                        let text = &html[content_start..content_start + close_offset];
                        let end_pos = content_start + close_offset + "</a>".len();

                        let resolved = resolve_rdfa_property(property_name, registry);
                        result.push_str(&format!(
                            "<span property=\"{resolved}\">{text}</span>"
                        ));
                        pos = end_pos;
                        continue;
                    }
                }
            }
        }

        // Not a match (or inside code/pre) — copy one '<' and advance past it so
        // the next iteration can find the next tag.
        result.push('<');
        pos += 1;
    }

    result
}

/// Resolve an RDFa property name to its compact prefixed form.
///
/// Resolution order:
/// 1. Try the registry's friendly-name property map.
/// 2. Try expanding as a prefixed IRI (e.g. `skos:broader`).
/// 3. Fall back to the raw name.
fn resolve_rdfa_property(name: &str, registry: &MappingRegistry) -> String {
    // Try friendly name resolution first.
    if let Some(iri) = registry.resolve_property(name) {
        return registry.compact_iri(iri);
    }
    // Try as a prefixed IRI (e.g. "skos:broader").
    if let Some(expanded) = registry.expand_iri(name) {
        return registry.compact_iri(&expanded);
    }
    // Fallback: use as-is.
    name.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_basic_markdown() {
        let html = render_markdown("# Hello\n\nA paragraph.");
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(html.contains("<p>A paragraph.</p>"));
    }

    #[test]
    fn render_gfm_table() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |";
        let html = render_markdown(md);
        assert!(html.contains("<table>"));
    }

    #[test]
    fn render_strikethrough() {
        let html = render_markdown("~~deleted~~");
        assert!(html.contains("<del>deleted</del>"));
    }

    #[test]
    fn rewrite_rdfa_with_mapping() {
        let mut registry = MappingRegistry::new();
        registry.add_property("author", "https://schema.org/author");
        let html = r#"<p>By <a href="rdfa:author">John</a></p>"#;
        let result = rewrite_rdfa_links(html, &registry);
        assert!(
            result.contains(r#"<span property="schema:author">John</span>"#),
            "expected schema:author span, got: {result}"
        );
    }

    #[test]
    fn rewrite_rdfa_skips_code_blocks() {
        let registry = MappingRegistry::new();
        let html = r#"<code><a href="rdfa:author">John</a></code>"#;
        let result = rewrite_rdfa_links(html, &registry);
        assert!(
            result.contains(r#"<a href="rdfa:author">"#),
            "should not rewrite inside code, got: {result}"
        );
    }

    #[test]
    fn rewrite_rdfa_skips_pre_blocks() {
        let registry = MappingRegistry::new();
        let html = r#"<pre><a href="rdfa:author">John</a></pre>"#;
        let result = rewrite_rdfa_links(html, &registry);
        assert!(
            result.contains(r#"<a href="rdfa:author">"#),
            "should not rewrite inside pre, got: {result}"
        );
    }

    #[test]
    fn rewrite_rdfa_no_match_passthrough() {
        let registry = MappingRegistry::new();
        let html = r#"<p>Normal <a href="https://example.com">link</a></p>"#;
        let result = rewrite_rdfa_links(html, &registry);
        assert_eq!(result, html);
    }

    #[test]
    fn rewrite_rdfa_unmapped_property_passthrough() {
        let registry = MappingRegistry::new();
        let html = r#"<p><a href="rdfa:customProp">value</a></p>"#;
        let result = rewrite_rdfa_links(html, &registry);
        assert!(
            result.contains(r#"<span property="customProp">value</span>"#),
            "unmapped property should pass through as-is, got: {result}"
        );
    }

    #[test]
    fn rewrite_rdfa_prefixed_iri() {
        let registry = MappingRegistry::new();
        let html = r#"<p><a href="rdfa:schema:name">Alice</a></p>"#;
        let result = rewrite_rdfa_links(html, &registry);
        assert!(
            result.contains(r#"<span property="schema:name">Alice</span>"#),
            "prefixed IRI should resolve and re-compact, got: {result}"
        );
    }

    #[test]
    fn rewrite_rdfa_preserves_surrounding_html() {
        let mut registry = MappingRegistry::new();
        registry.add_property("author", "https://schema.org/author");
        let html = r#"<p>Written by <a href="rdfa:author">Jane</a> on Monday.</p>"#;
        let result = rewrite_rdfa_links(html, &registry);
        assert_eq!(
            result,
            r#"<p>Written by <span property="schema:author">Jane</span> on Monday.</p>"#
        );
    }

    #[test]
    fn rewrite_rdfa_utf8_content() {
        let mut registry = MappingRegistry::new();
        registry.add_property("author", "https://schema.org/author");
        let html = r#"<p><a href="rdfa:author">Ólafur Árnalds</a></p>"#;
        let result = rewrite_rdfa_links(html, &registry);
        assert!(
            result.contains(r#"<span property="schema:author">Ólafur Árnalds</span>"#),
            "UTF-8 content should be preserved, got: {result}"
        );
    }
}

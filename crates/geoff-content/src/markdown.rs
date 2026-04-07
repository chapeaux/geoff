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
}

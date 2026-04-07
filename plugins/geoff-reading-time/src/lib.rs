//! Reading time plugin for Geoff.
//!
//! Calculates estimated reading time from Markdown content and injects
//! it as a template variable (`reading_time_minutes`) during page render.

use async_trait::async_trait;
use geoff_plugin::context::{ContentContext, RenderContext};
use geoff_plugin::traits::Plugin;

/// Average words per minute for reading time calculation.
const WORDS_PER_MINUTE: usize = 200;

/// A plugin that calculates reading time from page content.
struct ReadingTimePlugin;

impl ReadingTimePlugin {
    fn estimate_minutes(text: &str) -> u64 {
        let word_count = text.split_whitespace().count();
        let minutes = (word_count as f64 / WORDS_PER_MINUTE as f64).ceil() as u64;
        minutes.max(1)
    }
}

#[async_trait]
impl Plugin for ReadingTimePlugin {
    fn name(&self) -> &str {
        "reading-time"
    }

    async fn on_content_parsed(
        &self,
        ctx: &mut ContentContext<'_>,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let minutes = Self::estimate_minutes(&ctx.page.raw_body);
        ctx.page
            .frontmatter
            .insert("reading_time_minutes".to_string(), minutes.into());
        Ok(())
    }

    async fn on_page_render(
        &self,
        ctx: &mut RenderContext<'_>,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let minutes = ctx
            .page
            .frontmatter
            .get("reading_time_minutes")
            .and_then(|v| v.as_u64())
            .unwrap_or_else(|| Self::estimate_minutes(&ctx.page.raw_body));

        ctx.extra_vars.insert(
            "reading_time_minutes".to_string(),
            serde_json::Value::Number(minutes.into()),
        );
        Ok(())
    }
}

/// Entry point for dynamic loading. Returns a raw pointer to a boxed Plugin trait object.
///
/// # Safety
///
/// This function is called by the host via `libloading`. The returned pointer
/// must be passed to `Box::from_raw` by the caller.
#[unsafe(no_mangle)]
#[allow(improper_ctypes_definitions)]
pub extern "C" fn create_plugin() -> *mut dyn Plugin {
    Box::into_raw(Box::new(ReadingTimePlugin))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_short_text() {
        // 5 words -> ceil(5/200) = 1 minute minimum
        assert_eq!(
            ReadingTimePlugin::estimate_minutes("one two three four five"),
            1
        );
    }

    #[test]
    fn estimate_longer_text() {
        // 400 words -> ceil(400/200) = 2 minutes
        let text = "word ".repeat(400);
        assert_eq!(ReadingTimePlugin::estimate_minutes(&text), 2);
    }

    #[test]
    fn estimate_empty() {
        // 0 words -> max(0, 1) = 1 minute minimum
        assert_eq!(ReadingTimePlugin::estimate_minutes(""), 1);
    }

    #[test]
    fn estimate_exact_boundary() {
        // 200 words -> ceil(200/200) = 1 minute
        let text = "word ".repeat(200);
        assert_eq!(ReadingTimePlugin::estimate_minutes(&text), 1);
    }

    #[test]
    fn estimate_just_over_boundary() {
        // 201 words -> ceil(201/200) = 2 minutes
        let text = "word ".repeat(201);
        assert_eq!(ReadingTimePlugin::estimate_minutes(&text), 2);
    }
}

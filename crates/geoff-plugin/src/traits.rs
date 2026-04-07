use async_trait::async_trait;

use crate::context::{
    BuildContext, ContentContext, GraphContext, InitContext, OutputContext, RenderContext,
    ValidationContext, WatchContext,
};

/// The core plugin trait. All lifecycle hooks have default no-op implementations.
///
/// Plugins can be implemented in Rust (loaded as cdylib) or in TypeScript (proxied via Deno).
#[async_trait]
pub trait Plugin: Send + Sync {
    /// The plugin's unique name.
    fn name(&self) -> &str;

    /// Called once when the plugin is first loaded.
    async fn on_init(
        &self,
        _ctx: &mut InitContext<'_>,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Called at the start of each build.
    async fn on_build_start(
        &self,
        _ctx: &mut BuildContext<'_>,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Called after each content file is parsed.
    async fn on_content_parsed(
        &self,
        _ctx: &mut ContentContext<'_>,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Called after all content is ingested into the RDF graph.
    async fn on_graph_updated(
        &self,
        _ctx: &mut GraphContext<'_>,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Called after SHACL validation completes.
    async fn on_validation_complete(
        &self,
        _ctx: &mut ValidationContext<'_>,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Called before rendering each page.
    async fn on_page_render(
        &self,
        _ctx: &mut RenderContext<'_>,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Called after all output files are written.
    async fn on_build_complete(
        &self,
        _ctx: &mut OutputContext<'_>,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    /// Called when a file change is detected during dev server.
    async fn on_file_changed(
        &self,
        _ctx: &mut WatchContext<'_>,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

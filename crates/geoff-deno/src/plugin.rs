//! DenoPlugin: implements the Plugin trait by proxying lifecycle calls
//! to a Deno subprocess via JSON-RPC.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::bridge::DenoBridge;
use crate::protocol;
use geoff_plugin::context::{
    BuildContext, ContentContext, GraphContext, InitContext, OutputContext, RenderContext,
    ValidationContext, WatchContext,
};
use geoff_plugin::traits::Plugin;

/// A plugin backed by a Deno subprocess.
pub struct DenoPlugin {
    plugin_name: String,
    bridge: Arc<Mutex<DenoBridge>>,
}

impl DenoPlugin {
    /// Create a new DenoPlugin by spawning a Deno subprocess for the given script.
    pub async fn new(
        name: &str,
        script_path: &str,
    ) -> std::result::Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let bridge = DenoBridge::spawn(script_path).await?;
        tracing::info!(name, script_path, "spawned Deno plugin subprocess");
        Ok(Self {
            plugin_name: name.to_string(),
            bridge: Arc::new(Mutex::new(bridge)),
        })
    }

    /// Send a JSON-RPC call and check for errors in the response.
    async fn rpc_call(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> std::result::Result<Option<serde_json::Value>, Box<dyn std::error::Error + Send + Sync>>
    {
        let mut bridge = self.bridge.lock().await;
        let response = bridge.call(method, params).await?;

        if let Some(err) = response.error {
            return Err(Box::new(std::io::Error::other(format!(
                "Deno plugin '{}' error in {method}: {} (code {})",
                self.plugin_name, err.message, err.code
            ))));
        }

        Ok(response.result)
    }

    /// Shut down the Deno subprocess.
    pub async fn shutdown(
        &self,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut bridge = self.bridge.lock().await;
        bridge.shutdown().await
    }
}

#[async_trait]
impl Plugin for DenoPlugin {
    fn name(&self) -> &str {
        &self.plugin_name
    }

    async fn on_init(
        &self,
        ctx: &mut InitContext<'_>,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let params = protocol::InitParams {
            base_url: ctx.config.base_url.clone(),
            title: ctx.config.title.clone(),
            options: ctx.plugin_options.clone(),
        };
        self.rpc_call(protocol::METHOD_INIT, Some(serde_json::to_value(params)?))
            .await?;
        Ok(())
    }

    async fn on_build_start(
        &self,
        _ctx: &mut BuildContext<'_>,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.rpc_call(protocol::METHOD_BUILD_START, None).await?;
        Ok(())
    }

    async fn on_content_parsed(
        &self,
        ctx: &mut ContentContext<'_>,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let params = protocol::ContentParsedParams {
            page: ctx.page.clone(),
        };
        let result = self
            .rpc_call(
                protocol::METHOD_CONTENT_PARSED,
                Some(serde_json::to_value(params)?),
            )
            .await?;

        // If the plugin returned modified page data, apply it
        if let Some(val) = result {
            let modified: protocol::ContentParsedResult = serde_json::from_value(val)?;
            *ctx.page = modified.page;
        }

        Ok(())
    }

    async fn on_graph_updated(
        &self,
        _ctx: &mut GraphContext<'_>,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.rpc_call(protocol::METHOD_GRAPH_UPDATED, None).await?;
        Ok(())
    }

    async fn on_validation_complete(
        &self,
        ctx: &mut ValidationContext<'_>,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let params = protocol::ValidationParams {
            conforms: ctx.conforms,
            violations: ctx.violations,
        };
        self.rpc_call(
            protocol::METHOD_VALIDATION_COMPLETE,
            Some(serde_json::to_value(params)?),
        )
        .await?;
        Ok(())
    }

    async fn on_page_render(
        &self,
        ctx: &mut RenderContext<'_>,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let params = protocol::PageRenderParams {
            page: ctx.page.clone(),
            extra_vars: ctx.extra_vars.clone(),
        };
        let result = self
            .rpc_call(
                protocol::METHOD_PAGE_RENDER,
                Some(serde_json::to_value(params)?),
            )
            .await?;

        // If the plugin returned modified data, apply it
        if let Some(val) = result {
            let modified: protocol::PageRenderResult = serde_json::from_value(val)?;
            *ctx.page = modified.page;
            *ctx.extra_vars = modified.extra_vars;
        }

        Ok(())
    }

    async fn on_build_complete(
        &self,
        _ctx: &mut OutputContext<'_>,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.rpc_call(protocol::METHOD_BUILD_COMPLETE, None).await?;
        Ok(())
    }

    async fn on_file_changed(
        &self,
        ctx: &mut WatchContext<'_>,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let params = protocol::FileChangedParams {
            changed_path: ctx.changed_path.to_string(),
        };
        self.rpc_call(
            protocol::METHOD_FILE_CHANGED,
            Some(serde_json::to_value(params)?),
        )
        .await?;
        Ok(())
    }
}

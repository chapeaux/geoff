use std::collections::HashMap;

use crate::context::{
    BuildContext, ContentContext, GraphContext, InitContext, OutputContext, RenderContext,
    ValidationContext, WatchContext,
};
use crate::traits::Plugin;

/// Manages loaded plugins and dispatches lifecycle events in registration order.
pub struct PluginRegistry {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Register a plugin. Plugins are dispatched in registration order.
    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        tracing::info!(name = plugin.name(), "registered plugin");
        self.plugins.push(plugin);
    }

    /// Register multiple plugins at once, preserving order.
    pub fn register_all(&mut self, plugins: Vec<Box<dyn Plugin>>) {
        for plugin in plugins {
            self.register(plugin);
        }
    }

    /// Returns the number of registered plugins.
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Returns true if no plugins are registered.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Returns the names of all registered plugins.
    pub fn plugin_names(&self) -> Vec<&str> {
        self.plugins.iter().map(|p| p.name()).collect()
    }

    // ── Lifecycle dispatchers ────────────────────────────────────────

    /// Dispatch `on_init` to all plugins in order.
    pub async fn dispatch_init(
        &self,
        config: &geoff_core::config::SiteConfig,
        plugin_options: &HashMap<String, HashMap<String, toml::Value>>,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for plugin in &self.plugins {
            let empty = HashMap::new();
            let opts = plugin_options.get(plugin.name()).unwrap_or(&empty);
            let mut ctx = InitContext {
                config,
                plugin_options: opts,
            };
            plugin.on_init(&mut ctx).await?;
        }
        Ok(())
    }

    /// Dispatch `on_build_start` to all plugins in order.
    pub async fn dispatch_build_start(
        &self,
        config: &geoff_core::config::SiteConfig,
        store: &geoff_graph::store::ContentStore,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for plugin in &self.plugins {
            let mut ctx = BuildContext { config, store };
            plugin.on_build_start(&mut ctx).await?;
        }
        Ok(())
    }

    /// Dispatch `on_content_parsed` to all plugins in order for a single page.
    pub async fn dispatch_content_parsed(
        &self,
        config: &geoff_core::config::SiteConfig,
        page: &mut crate::context::PageData,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for plugin in &self.plugins {
            let mut ctx = ContentContext { config, page };
            plugin.on_content_parsed(&mut ctx).await?;
        }
        Ok(())
    }

    /// Dispatch `on_graph_updated` to all plugins in order.
    pub async fn dispatch_graph_updated(
        &self,
        config: &geoff_core::config::SiteConfig,
        store: &geoff_graph::store::ContentStore,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for plugin in &self.plugins {
            let mut ctx = GraphContext { config, store };
            plugin.on_graph_updated(&mut ctx).await?;
        }
        Ok(())
    }

    /// Dispatch `on_validation_complete` to all plugins in order.
    pub async fn dispatch_validation_complete(
        &self,
        config: &geoff_core::config::SiteConfig,
        store: &geoff_graph::store::ContentStore,
        conforms: bool,
        violations: usize,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for plugin in &self.plugins {
            let mut ctx = ValidationContext {
                config,
                store,
                conforms,
                violations,
            };
            plugin.on_validation_complete(&mut ctx).await?;
        }
        Ok(())
    }

    /// Dispatch `on_page_render` to all plugins in order for a single page.
    pub async fn dispatch_page_render(
        &self,
        config: &geoff_core::config::SiteConfig,
        store: &geoff_graph::store::ContentStore,
        page: &mut crate::context::PageData,
        extra_vars: &mut HashMap<String, serde_json::Value>,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for plugin in &self.plugins {
            let mut ctx = RenderContext {
                config,
                store,
                page,
                extra_vars,
            };
            plugin.on_page_render(&mut ctx).await?;
        }
        Ok(())
    }

    /// Dispatch `on_build_complete` to all plugins in order.
    pub async fn dispatch_build_complete(
        &self,
        config: &geoff_core::config::SiteConfig,
        store: &geoff_graph::store::ContentStore,
        outputs: &HashMap<String, String>,
        output_dir: &camino::Utf8Path,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for plugin in &self.plugins {
            let mut ctx = OutputContext {
                config,
                store,
                outputs,
                output_dir,
            };
            plugin.on_build_complete(&mut ctx).await?;
        }
        Ok(())
    }

    /// Dispatch `on_file_changed` to all plugins in order.
    pub async fn dispatch_file_changed(
        &self,
        config: &geoff_core::config::SiteConfig,
        changed_path: &str,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        for plugin in &self.plugins {
            let mut ctx = WatchContext {
                config,
                changed_path,
            };
            plugin.on_file_changed(&mut ctx).await?;
        }
        Ok(())
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::Plugin;
    use async_trait::async_trait;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestPlugin {
        name: String,
        init_count: Arc<AtomicUsize>,
        build_start_count: Arc<AtomicUsize>,
    }

    impl TestPlugin {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                init_count: Arc::new(AtomicUsize::new(0)),
                build_start_count: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    #[async_trait]
    impl Plugin for TestPlugin {
        fn name(&self) -> &str {
            &self.name
        }

        async fn on_init(
            &self,
            _ctx: &mut InitContext<'_>,
        ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.init_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn on_build_start(
            &self,
            _ctx: &mut BuildContext<'_>,
        ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.build_start_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn registry_register_and_names() {
        let mut registry = PluginRegistry::new();
        assert!(registry.is_empty());

        registry.register(Box::new(TestPlugin::new("alpha")));
        registry.register(Box::new(TestPlugin::new("beta")));

        assert_eq!(registry.len(), 2);
        assert!(!registry.is_empty());
        assert_eq!(registry.plugin_names(), vec!["alpha", "beta"]);
    }

    #[test]
    fn registry_default() {
        let registry = PluginRegistry::default();
        assert!(registry.is_empty());
    }

    #[tokio::test]
    async fn dispatch_init_calls_all_plugins() {
        let mut registry = PluginRegistry::new();

        let p1 = TestPlugin::new("p1");
        let p1_count = Arc::clone(&p1.init_count);
        let p2 = TestPlugin::new("p2");
        let p2_count = Arc::clone(&p2.init_count);

        registry.register(Box::new(p1));
        registry.register(Box::new(p2));

        let config = geoff_core::config::SiteConfig {
            base_url: "https://example.com".to_string(),
            title: "Test".to_string(),
            content_dir: "content".into(),
            output_dir: "dist".into(),
            template_dir: "templates".into(),
            plugins: vec![],
        };

        let opts = HashMap::new();
        registry.dispatch_init(&config, &opts).await.unwrap();

        assert_eq!(p1_count.load(Ordering::SeqCst), 1);
        assert_eq!(p2_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn dispatch_build_start_calls_all_plugins() {
        let mut registry = PluginRegistry::new();

        let p1 = TestPlugin::new("p1");
        let p1_count = Arc::clone(&p1.build_start_count);

        registry.register(Box::new(p1));

        let config = geoff_core::config::SiteConfig {
            base_url: "https://example.com".to_string(),
            title: "Test".to_string(),
            content_dir: "content".into(),
            output_dir: "dist".into(),
            template_dir: "templates".into(),
            plugins: vec![],
        };

        let store = geoff_graph::store::ContentStore::new().expect("failed to create store");

        registry
            .dispatch_build_start(&config, &store)
            .await
            .unwrap();

        assert_eq!(p1_count.load(Ordering::SeqCst), 1);
    }
}

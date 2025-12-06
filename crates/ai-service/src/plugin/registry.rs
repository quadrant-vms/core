use super::AiPlugin;
use anyhow::{anyhow, Result};
use common::ai_tasks::PluginInfo;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Registry for AI plugins
#[derive(Clone)]
pub struct PluginRegistry {
    plugins: Arc<RwLock<HashMap<String, Arc<RwLock<dyn AiPlugin>>>>>,
}

impl PluginRegistry {
    /// Create a new empty plugin registry
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a plugin
    pub async fn register(&self, plugin: Arc<RwLock<dyn AiPlugin>>) -> Result<()> {
        let mut plugins = self.plugins.write().await;
        let plugin_read = plugin.read().await;
        let id = plugin_read.id().to_string();
        drop(plugin_read);

        if plugins.contains_key(&id) {
            return Err(anyhow!("Plugin '{}' is already registered", id));
        }

        plugins.insert(id.clone(), plugin);
        tracing::info!("Registered AI plugin: {}", id);
        Ok(())
    }

    /// Get a plugin by ID
    pub async fn get(&self, plugin_id: &str) -> Result<Arc<RwLock<dyn AiPlugin>>> {
        let plugins = self.plugins.read().await;
        plugins
            .get(plugin_id)
            .cloned()
            .ok_or_else(|| anyhow!("Plugin '{}' not found", plugin_id))
    }

    /// List all registered plugins
    pub async fn list(&self) -> Vec<PluginInfo> {
        let plugins = self.plugins.read().await;
        let mut infos = Vec::new();

        for plugin in plugins.values() {
            let plugin_read = plugin.read().await;
            infos.push(plugin_read.info());
        }

        infos
    }

    /// Check if a plugin is registered
    pub async fn has_plugin(&self, plugin_id: &str) -> bool {
        let plugins = self.plugins.read().await;
        plugins.contains_key(plugin_id)
    }

    /// Get the number of registered plugins
    pub async fn count(&self) -> usize {
        let plugins = self.plugins.read().await;
        plugins.len()
    }

    /// Health check all plugins
    pub async fn health_check_all(&self) -> HashMap<String, bool> {
        let plugins = self.plugins.read().await;
        let mut results = HashMap::new();

        for (id, plugin) in plugins.iter() {
            let plugin_read = plugin.read().await;
            let healthy = plugin_read.health_check().await.unwrap_or(false);
            results.insert(id.clone(), healthy);
        }

        results
    }

    /// Shutdown all plugins
    pub async fn shutdown_all(&self) -> Result<()> {
        let plugins = self.plugins.read().await;

        for (id, plugin) in plugins.iter() {
            let mut plugin_write = plugin.write().await;
            if let Err(e) = plugin_write.shutdown().await {
                tracing::error!("Error shutting down plugin '{}': {}", id, e);
            }
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
    use async_trait::async_trait;
    use common::ai_tasks::{AiResult, VideoFrame};

    struct MockPlugin {
        id: String,
    }

    #[async_trait]
    impl AiPlugin for MockPlugin {
        fn id(&self) -> &'static str {
            "mock_plugin"
        }

        fn name(&self) -> &'static str {
            "Mock Plugin"
        }

        fn description(&self) -> &'static str {
            "A mock plugin for testing"
        }

        fn version(&self) -> &'static str {
            "1.0.0"
        }

        async fn init(&mut self, _config: serde_json::Value) -> Result<()> {
            Ok(())
        }

        async fn process_frame(&self, _frame: &VideoFrame) -> Result<AiResult> {
            Ok(AiResult {
                task_id: "test".to_string(),
                timestamp: 0,
                plugin_type: self.id().to_string(),
                detections: vec![],
                confidence: Some(1.0),
                processing_time_ms: Some(10),
                metadata: None,
            })
        }
    }

    #[tokio::test]
    async fn test_plugin_registration() {
        let registry = PluginRegistry::new();
        let plugin = Arc::new(RwLock::new(MockPlugin {
            id: "test".to_string(),
        }));

        registry.register(plugin.clone()).await.unwrap();
        assert_eq!(registry.count().await, 1);
        assert!(registry.has_plugin("mock_plugin").await);
    }

    #[tokio::test]
    async fn test_plugin_get() {
        let registry = PluginRegistry::new();
        let plugin = Arc::new(RwLock::new(MockPlugin {
            id: "test".to_string(),
        }));

        registry.register(plugin.clone()).await.unwrap();
        let retrieved = registry.get("mock_plugin").await.unwrap();
        let retrieved_read = retrieved.read().await;
        assert_eq!(retrieved_read.id(), "mock_plugin");
    }

    #[tokio::test]
    async fn test_duplicate_registration() {
        let registry = PluginRegistry::new();
        let plugin1 = Arc::new(RwLock::new(MockPlugin {
            id: "test1".to_string(),
        }));
        let plugin2 = Arc::new(RwLock::new(MockPlugin {
            id: "test2".to_string(),
        }));

        registry.register(plugin1).await.unwrap();
        let result = registry.register(plugin2).await;
        assert!(result.is_err());
    }
}

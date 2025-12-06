pub mod mock_detector;
pub mod registry;
pub mod yolov8_detector;

use anyhow::Result;
use async_trait::async_trait;
use common::ai_tasks::{AiResult, PluginInfo, VideoFrame};

/// Core trait that all AI plugins must implement
#[async_trait]
pub trait AiPlugin: Send + Sync {
    /// Get the unique plugin identifier (e.g., "yolov8_detector")
    fn id(&self) -> &'static str;

    /// Get human-readable plugin name
    fn name(&self) -> &'static str;

    /// Get plugin description
    fn description(&self) -> &'static str;

    /// Get plugin version
    fn version(&self) -> &'static str;

    /// Get plugin metadata
    fn info(&self) -> PluginInfo {
        PluginInfo {
            id: self.id().to_string(),
            name: self.name().to_string(),
            description: self.description().to_string(),
            version: self.version().to_string(),
            config_schema: self.config_schema(),
            supported_formats: self.supported_formats(),
            requires_gpu: self.requires_gpu(),
        }
    }

    /// Get plugin-specific configuration schema (JSON Schema)
    fn config_schema(&self) -> Option<serde_json::Value> {
        None
    }

    /// Get supported input formats (e.g., ["jpeg", "png"])
    fn supported_formats(&self) -> Vec<String> {
        vec!["jpeg".to_string()]
    }

    /// Whether the plugin requires GPU acceleration
    fn requires_gpu(&self) -> bool {
        false
    }

    /// Initialize plugin with configuration
    async fn init(&mut self, config: serde_json::Value) -> Result<()>;

    /// Process a video frame and return detection results
    async fn process_frame(&self, frame: &VideoFrame) -> Result<AiResult>;

    /// Health check - verify the plugin is operational
    async fn health_check(&self) -> Result<bool> {
        Ok(true)
    }

    /// Shutdown the plugin gracefully
    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

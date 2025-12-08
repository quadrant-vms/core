//! AI Task contracts for the Quadrant VMS AI plugin system.
//!
//! This module defines the contracts for AI task lifecycle, plugin configuration,
//! and result delivery.

use serde::{Deserialize, Serialize};

/// Configuration for frame capture and processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiFrameConfig {
    /// Process every Nth frame (default: 1)
    #[serde(default = "default_frame_interval")]
    pub frame_interval: u32,

    /// Maximum FPS to process (default: no limit)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_fps: Option<u32>,

    /// Skip first N seconds of stream (default: 0)
    #[serde(default)]
    pub skip_seconds: u32,
}

impl Default for AiFrameConfig {
    fn default() -> Self {
        Self {
            frame_interval: 1,
            max_fps: None,
            skip_seconds: 0,
        }
    }
}

fn default_frame_interval() -> u32 {
    1
}

/// Configuration for an AI task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiTaskConfig {
    /// Unique task identifier
    pub id: String,

    /// Plugin type identifier (e.g., "object_detection", "pose_estimation")
    pub plugin_type: String,

    /// Source stream ID to process (if using existing stream)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_stream_id: Option<String>,

    /// Source recording ID to process (if using existing recording)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_recording_id: Option<String>,

    /// Plugin-specific configuration (JSON object)
    #[serde(default)]
    pub model_config: serde_json::Value,

    /// Frame capture and processing configuration
    #[serde(default)]
    pub frame_config: AiFrameConfig,

    /// Output format configuration
    pub output: AiOutputConfig,
}

/// Output configuration for AI task results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiOutputConfig {
    /// Output type (webhook, mqtt, rabbitmq, file)
    #[serde(rename = "type")]
    pub output_type: String,

    /// Output-specific configuration (JSON object)
    #[serde(default)]
    pub config: serde_json::Value,
}

/// Request to start an AI task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiTaskStartRequest {
    /// Task configuration
    pub config: AiTaskConfig,

    /// Lease TTL in seconds (default: 300)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lease_ttl_secs: Option<u64>,
}

/// Response to AI task start request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiTaskStartResponse {
    /// Whether the task was accepted
    pub accepted: bool,

    /// Lease ID if task was accepted
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lease_id: Option<String>,

    /// Human-readable message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Request to stop an AI task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiTaskStopRequest {
    /// Task ID to stop
    pub task_id: String,
}

/// Response to AI task stop request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiTaskStopResponse {
    /// Whether the stop was successful
    pub success: bool,

    /// Human-readable message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// AI task state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiTaskState {
    /// Task is queued but not started
    Pending,

    /// Plugin is being initialized
    Initializing,

    /// Task is actively processing frames
    Processing,

    /// Task is paused (can be resumed)
    Paused,

    /// Task is being stopped
    Stopping,

    /// Task has stopped normally
    Stopped,

    /// Task encountered an error
    Error,
}

/// AI task information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiTaskInfo {
    /// Task configuration
    pub config: AiTaskConfig,

    /// Current state
    pub state: AiTaskState,

    /// Node ID running this task
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,

    /// Lease ID if acquired
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lease_id: Option<String>,

    /// Last error message if in error state
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,

    /// Timestamp when task started (Unix timestamp in milliseconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<u64>,

    /// Timestamp when task stopped (Unix timestamp in milliseconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stopped_at: Option<u64>,

    /// Timestamp of last processed frame (Unix timestamp in milliseconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_processed_frame: Option<u64>,

    /// Total frames processed
    pub frames_processed: u64,

    /// Total detections made
    pub detections_made: u64,
}

/// Video frame metadata for AI processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoFrame {
    /// Source task or stream ID
    pub source_id: String,

    /// Frame timestamp (Unix timestamp in milliseconds)
    pub timestamp: u64,

    /// Frame sequence number
    pub sequence: u64,

    /// Frame width in pixels
    pub width: u32,

    /// Frame height in pixels
    pub height: u32,

    /// Image format (e.g., "jpeg", "png", "raw")
    pub format: String,

    /// Frame data (base64 encoded for JSON transport)
    pub data: String,
}

/// Detection result from AI plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Detection {
    /// Object class/label
    pub class: String,

    /// Detection confidence (0.0 to 1.0)
    pub confidence: f32,

    /// Bounding box (x, y, width, height)
    pub bbox: BoundingBox,

    /// Additional metadata (plugin-specific)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Bounding box coordinates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// AI processing result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiResult {
    /// Task ID that produced this result
    pub task_id: String,

    /// Frame timestamp
    pub timestamp: u64,

    /// Plugin type that produced the result
    pub plugin_type: String,

    /// Detected objects/entities
    pub detections: Vec<Detection>,

    /// Overall confidence score
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,

    /// Processing latency in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processing_time_ms: Option<u64>,

    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Plugin metadata and capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    /// Plugin unique identifier
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Plugin description
    pub description: String,

    /// Plugin version
    pub version: String,

    /// Configuration schema (JSON Schema)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_schema: Option<serde_json::Value>,

    /// Supported input formats
    pub supported_formats: Vec<String>,

    /// Whether the plugin requires GPU
    pub requires_gpu: bool,
}

/// List of available plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginListResponse {
    pub plugins: Vec<PluginInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_task_config_serialization() {
        let config = AiTaskConfig {
            id: "task-1".to_string(),
            plugin_type: "object_detection".to_string(),
            source_stream_id: Some("stream-123".to_string()),
            source_recording_id: None,
            model_config: serde_json::json!({
                "model": "yolov8",
                "confidence_threshold": 0.5
            }),
            frame_config: AiFrameConfig {
                frame_interval: 5,
                max_fps: Some(10),
                skip_seconds: 0,
            },
            output: AiOutputConfig {
                output_type: "webhook".to_string(),
                config: serde_json::json!({
                    "url": "http://localhost:8080/detections",
                    "headers": {}
                }),
            },
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AiTaskConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, config.id);
        assert_eq!(deserialized.plugin_type, config.plugin_type);
    }

    #[test]
    fn test_ai_task_state_transitions() {
        let states = vec![
            AiTaskState::Pending,
            AiTaskState::Initializing,
            AiTaskState::Processing,
            AiTaskState::Stopping,
            AiTaskState::Stopped,
        ];

        for state in states {
            let json = serde_json::to_string(&state).unwrap();
            let deserialized: AiTaskState = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, state);
        }
    }

    #[test]
    fn test_detection_serialization() {
        let detection = Detection {
            class: "person".to_string(),
            confidence: 0.95,
            bbox: BoundingBox {
                x: 100,
                y: 200,
                width: 50,
                height: 100,
            },
            metadata: Some(serde_json::json!({
                "age_estimate": "25-35",
                "pose": "standing"
            })),
        };

        let json = serde_json::to_string(&detection).unwrap();
        let deserialized: Detection = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.class, detection.class);
        assert_eq!(deserialized.confidence, detection.confidence);
    }
}

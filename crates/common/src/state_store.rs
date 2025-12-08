use anyhow::Result;
use async_trait::async_trait;

use crate::ai_tasks::AiTaskInfo;
use crate::recordings::RecordingInfo;
use crate::streams::StreamInfo;

/// Trait for persistent state storage
#[async_trait]
pub trait StateStore: Send + Sync {
    // Stream state operations
    async fn save_stream(&self, info: &StreamInfo) -> Result<()>;
    async fn get_stream(&self, stream_id: &str) -> Result<Option<StreamInfo>>;
    async fn list_streams(&self, node_id: Option<&str>) -> Result<Vec<StreamInfo>>;
    async fn delete_stream(&self, stream_id: &str) -> Result<()>;
    async fn update_stream_state(&self, stream_id: &str, state: &str, error: Option<&str>) -> Result<()>;

    // Recording state operations
    async fn save_recording(&self, info: &RecordingInfo) -> Result<()>;
    async fn get_recording(&self, recording_id: &str) -> Result<Option<RecordingInfo>>;
    async fn list_recordings(&self, node_id: Option<&str>) -> Result<Vec<RecordingInfo>>;
    async fn delete_recording(&self, recording_id: &str) -> Result<()>;
    async fn update_recording_state(&self, recording_id: &str, state: &str, error: Option<&str>) -> Result<()>;

    // AI task state operations
    async fn save_ai_task(&self, info: &AiTaskInfo) -> Result<()>;
    async fn get_ai_task(&self, task_id: &str) -> Result<Option<AiTaskInfo>>;
    async fn list_ai_tasks(&self, node_id: Option<&str>) -> Result<Vec<AiTaskInfo>>;
    async fn delete_ai_task(&self, task_id: &str) -> Result<()>;
    async fn update_ai_task_state(&self, task_id: &str, state: &str, error: Option<&str>) -> Result<()>;
    async fn update_ai_task_stats(&self, task_id: &str, frames_delta: u64, detections_delta: u64) -> Result<()>;

    // Health check
    async fn health_check(&self) -> Result<bool>;
}

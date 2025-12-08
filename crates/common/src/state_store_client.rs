use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;

use crate::ai_tasks::AiTaskInfo;
use crate::recordings::RecordingInfo;
use crate::state_store::StateStore;
use crate::streams::StreamInfo;

/// HTTP client for StateStore API
#[derive(Clone)]
pub struct StateStoreClient {
    base_url: String,
    client: Client,
}

impl StateStoreClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: Client::new(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

#[derive(Serialize)]
struct UpdateStateRequest {
    state: String,
    error: Option<String>,
}

#[derive(Serialize)]
struct UpdateStatsRequest {
    frames_delta: u64,
    detections_delta: u64,
}

#[async_trait]
impl StateStore for StateStoreClient {
    async fn save_stream(&self, info: &StreamInfo) -> Result<()> {
        self.client
            .post(self.url("/v1/state/streams"))
            .json(info)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn get_stream(&self, stream_id: &str) -> Result<Option<StreamInfo>> {
        let response = self.client
            .get(self.url(&format!("/v1/state/streams/{}", stream_id)))
            .send()
            .await?
            .error_for_status()?;

        let stream = response.json::<Option<StreamInfo>>().await?;
        Ok(stream)
    }

    async fn list_streams(&self, node_id: Option<&str>) -> Result<Vec<StreamInfo>> {
        let mut url = self.url("/v1/state/streams");
        if let Some(node_id) = node_id {
            url = format!("{}?node_id={}", url, node_id);
        }

        let response = self.client
            .get(&url)
            .send()
            .await?
            .error_for_status()?;

        let streams = response.json::<Vec<StreamInfo>>().await?;
        Ok(streams)
    }

    async fn delete_stream(&self, stream_id: &str) -> Result<()> {
        self.client
            .delete(self.url(&format!("/v1/state/streams/{}", stream_id)))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn update_stream_state(&self, stream_id: &str, state: &str, error: Option<&str>) -> Result<()> {
        let req = UpdateStateRequest {
            state: state.to_string(),
            error: error.map(|s| s.to_string()),
        };

        self.client
            .put(self.url(&format!("/v1/state/streams/{}/state", stream_id)))
            .json(&req)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn save_recording(&self, info: &RecordingInfo) -> Result<()> {
        self.client
            .post(self.url("/v1/state/recordings"))
            .json(info)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn get_recording(&self, recording_id: &str) -> Result<Option<RecordingInfo>> {
        let response = self.client
            .get(self.url(&format!("/v1/state/recordings/{}", recording_id)))
            .send()
            .await?
            .error_for_status()?;

        let recording = response.json::<Option<RecordingInfo>>().await?;
        Ok(recording)
    }

    async fn list_recordings(&self, node_id: Option<&str>) -> Result<Vec<RecordingInfo>> {
        let mut url = self.url("/v1/state/recordings");
        if let Some(node_id) = node_id {
            url = format!("{}?node_id={}", url, node_id);
        }

        let response = self.client
            .get(&url)
            .send()
            .await?
            .error_for_status()?;

        let recordings = response.json::<Vec<RecordingInfo>>().await?;
        Ok(recordings)
    }

    async fn delete_recording(&self, recording_id: &str) -> Result<()> {
        self.client
            .delete(self.url(&format!("/v1/state/recordings/{}", recording_id)))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn update_recording_state(&self, recording_id: &str, state: &str, error: Option<&str>) -> Result<()> {
        let req = UpdateStateRequest {
            state: state.to_string(),
            error: error.map(|s| s.to_string()),
        };

        self.client
            .put(self.url(&format!("/v1/state/recordings/{}/state", recording_id)))
            .json(&req)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn save_ai_task(&self, info: &AiTaskInfo) -> Result<()> {
        self.client
            .post(self.url("/v1/state/ai-tasks"))
            .json(info)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn get_ai_task(&self, task_id: &str) -> Result<Option<AiTaskInfo>> {
        let response = self.client
            .get(self.url(&format!("/v1/state/ai-tasks/{}", task_id)))
            .send()
            .await?
            .error_for_status()?;

        let task = response.json::<Option<AiTaskInfo>>().await?;
        Ok(task)
    }

    async fn list_ai_tasks(&self, node_id: Option<&str>) -> Result<Vec<AiTaskInfo>> {
        let mut url = self.url("/v1/state/ai-tasks");
        if let Some(node_id) = node_id {
            url = format!("{}?node_id={}", url, node_id);
        }

        let response = self.client
            .get(&url)
            .send()
            .await?
            .error_for_status()?;

        let tasks = response.json::<Vec<AiTaskInfo>>().await?;
        Ok(tasks)
    }

    async fn delete_ai_task(&self, task_id: &str) -> Result<()> {
        self.client
            .delete(self.url(&format!("/v1/state/ai-tasks/{}", task_id)))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn update_ai_task_state(&self, task_id: &str, state: &str, error: Option<&str>) -> Result<()> {
        let req = UpdateStateRequest {
            state: state.to_string(),
            error: error.map(|s| s.to_string()),
        };

        self.client
            .put(self.url(&format!("/v1/state/ai-tasks/{}/state", task_id)))
            .json(&req)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn update_ai_task_stats(&self, task_id: &str, frames_delta: u64, detections_delta: u64) -> Result<()> {
        let req = UpdateStatsRequest {
            frames_delta,
            detections_delta,
        };

        self.client
            .put(self.url(&format!("/v1/state/ai-tasks/{}/stats", task_id)))
            .json(&req)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn health_check(&self) -> Result<bool> {
        // Use coordinator health check endpoint
        let response = self.client
            .get(self.url("/readyz"))
            .send()
            .await;

        match response {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
    }
}

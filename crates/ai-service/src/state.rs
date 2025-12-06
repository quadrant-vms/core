use crate::coordinator::CoordinatorClient;
use crate::plugin::registry::PluginRegistry;
use anyhow::{anyhow, Context, Result};
use common::ai_tasks::{AiResult, AiTaskConfig, AiTaskInfo, AiTaskState, VideoFrame};
use common::leases::{LeaseAcquireRequest, LeaseKind, LeaseReleaseRequest, LeaseRenewRequest};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

const MAX_RENEWAL_RETRIES: u32 = 3;
const RENEWAL_BACKOFF_BASE_MS: u64 = 100;

#[derive(Clone)]
pub struct AiServiceState {
    inner: Arc<AiServiceStateInner>,
}

struct AiServiceStateInner {
    node_id: String,
    coordinator: Option<Arc<dyn CoordinatorClient>>,
    plugins: PluginRegistry,
    tasks: RwLock<HashMap<String, AiTaskInfo>>,
    renewals: RwLock<HashMap<String, CancellationToken>>,
}

impl AiServiceState {
    pub fn new(node_id: String, plugins: PluginRegistry) -> Self {
        Self {
            inner: Arc::new(AiServiceStateInner {
                node_id,
                coordinator: None,
                plugins,
                tasks: RwLock::new(HashMap::new()),
                renewals: RwLock::new(HashMap::new()),
            }),
        }
    }

    pub fn with_coordinator(
        node_id: String,
        coordinator: Arc<dyn CoordinatorClient>,
        plugins: PluginRegistry,
    ) -> Self {
        Self {
            inner: Arc::new(AiServiceStateInner {
                node_id,
                coordinator: Some(coordinator),
                plugins,
                tasks: RwLock::new(HashMap::new()),
                renewals: RwLock::new(HashMap::new()),
            }),
        }
    }

    pub fn node_id(&self) -> &str {
        &self.inner.node_id
    }

    pub fn plugins(&self) -> &PluginRegistry {
        &self.inner.plugins
    }

    pub async fn get_task(&self, task_id: &str) -> Option<AiTaskInfo> {
        let tasks = self.inner.tasks.read().await;
        tasks.get(task_id).cloned()
    }

    pub async fn list_tasks(&self) -> Vec<AiTaskInfo> {
        let tasks = self.inner.tasks.read().await;
        tasks.values().cloned().collect()
    }

    pub async fn start_task(
        &self,
        config: AiTaskConfig,
        lease_ttl_secs: Option<u64>,
    ) -> Result<String> {
        let task_id = config.id.clone();

        // Check if task already exists
        {
            let tasks = self.inner.tasks.read().await;
            if tasks.contains_key(&task_id) {
                return Err(anyhow!("Task '{}' already exists", task_id));
            }
        }

        // Verify plugin exists
        if !self.inner.plugins.has_plugin(&config.plugin_type).await {
            return Err(anyhow!("Plugin '{}' not found", config.plugin_type));
        }

        // Acquire lease from coordinator if available
        let lease_id = if let Some(coordinator) = &self.inner.coordinator {
            let ttl = lease_ttl_secs.unwrap_or(300);
            let request = LeaseAcquireRequest {
                resource_id: task_id.clone(),
                holder_id: self.inner.node_id.clone(),
                kind: LeaseKind::Ai,
                ttl_secs: ttl,
            };

            let response = coordinator
                .acquire(&request)
                .await
                .context("Failed to acquire lease for AI task")?;

            response.record.map(|r| r.lease_id)
        } else {
            None
        };

        // Create task info
        let task_info = AiTaskInfo {
            config: config.clone(),
            state: AiTaskState::Initializing,
            lease_id: lease_id.clone(),
            last_error: None,
            started_at: Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
            ),
            last_processed_frame: None,
            frames_processed: 0,
            detections_made: 0,
        };

        // Store task
        {
            let mut tasks = self.inner.tasks.write().await;
            tasks.insert(task_id.clone(), task_info.clone());
        }

        // Start lease renewal if we have a lease
        if let (Some(lid), Some(coordinator)) = (lease_id, &self.inner.coordinator) {
            self.start_renewal_loop(task_id.clone(), lid, coordinator.clone(), lease_ttl_secs)
                .await;
        }

        // Update state to Processing
        self.update_task_state(&task_id, AiTaskState::Processing)
            .await?;

        info!("Started AI task: {} with plugin: {}", task_id, config.plugin_type);

        Ok(task_id)
    }

    pub async fn stop_task(&self, task_id: &str) -> Result<()> {
        // Cancel renewal loop
        {
            let mut renewals = self.inner.renewals.write().await;
            if let Some(token) = renewals.remove(task_id) {
                token.cancel();
            }
        }

        // Get task info for lease release
        let task_info = {
            let tasks = self.inner.tasks.read().await;
            tasks.get(task_id).cloned()
        };

        if let Some(info) = task_info {
            // Release lease if exists
            if let (Some(lease_id), Some(coordinator)) =
                (info.lease_id, &self.inner.coordinator)
            {
                let request = LeaseReleaseRequest { lease_id };

                if let Err(e) = coordinator.release(&request).await {
                    warn!("Failed to release lease for task {}: {}", task_id, e);
                }
            }

            // Update task state
            self.update_task_state(task_id, AiTaskState::Stopped)
                .await?;

            info!("Stopped AI task: {}", task_id);
            Ok(())
        } else {
            Err(anyhow!("Task '{}' not found", task_id))
        }
    }

    async fn update_task_state(&self, task_id: &str, new_state: AiTaskState) -> Result<()> {
        let mut tasks = self.inner.tasks.write().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.state = new_state;
            Ok(())
        } else {
            Err(anyhow!("Task '{}' not found", task_id))
        }
    }

    pub async fn update_task_stats(&self, task_id: &str, frames_delta: u64, detections_delta: u64) {
        let mut tasks = self.inner.tasks.write().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.frames_processed += frames_delta;
            task.detections_made += detections_delta;
            task.last_processed_frame = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64,
            );
        }
    }

    /// Process a video frame for a specific task
    pub async fn process_frame(&self, task_id: &str, frame: VideoFrame) -> Result<AiResult> {
        // Get task info
        let task_info = {
            let tasks = self.inner.tasks.read().await;
            tasks.get(task_id).cloned()
                .ok_or_else(|| anyhow!("Task '{}' not found", task_id))?
        };

        // Verify task is in processing state
        if task_info.state != AiTaskState::Processing {
            return Err(anyhow!("Task '{}' is not in processing state (current: {:?})", task_id, task_info.state));
        }

        // Get the plugin
        let plugin = self.inner.plugins.get(&task_info.config.plugin_type).await
            .context(format!("Plugin '{}' not found", task_info.config.plugin_type))?;

        // Process frame with plugin
        let plugin_read = plugin.read().await;
        let start_time = std::time::Instant::now();
        let mut result = plugin_read.process_frame(&frame).await
            .context("Failed to process frame with plugin")?;
        let processing_time = start_time.elapsed().as_millis() as u64;
        drop(plugin_read);

        // Override task_id to match the actual task (plugin may use frame.source_id)
        result.task_id = task_id.to_string();

        // Update task stats
        let detections_count = result.detections.len() as u64;
        self.update_task_stats(task_id, 1, detections_count).await;

        // Update metrics
        telemetry::metrics::AI_SERVICE_FRAMES_PROCESSED
            .with_label_values(&[&task_info.config.plugin_type, "success"])
            .inc();
        telemetry::metrics::AI_SERVICE_DETECTION_LATENCY
            .with_label_values(&[&task_info.config.plugin_type])
            .observe(processing_time as f64 / 1000.0);

        info!(
            task_id = %task_id,
            detections = detections_count,
            processing_time_ms = processing_time,
            "Processed frame"
        );

        Ok(result)
    }

    async fn start_renewal_loop(
        &self,
        task_id: String,
        lease_id: String,
        coordinator: Arc<dyn CoordinatorClient>,
        ttl_secs: Option<u64>,
    ) {
        let token = CancellationToken::new();
        let ttl = ttl_secs.unwrap_or(300);
        let renew_interval = Duration::from_secs(ttl / 2);

        // Store cancellation token
        {
            let mut renewals = self.inner.renewals.write().await;
            renewals.insert(task_id.clone(), token.clone());
        }

        // Spawn renewal loop
        let state = self.clone();
        tokio::spawn(async move {
            let mut consecutive_failures = 0;

            loop {
                tokio::select! {
                    _ = token.cancelled() => {
                        info!("Renewal loop cancelled for task: {}", task_id);
                        break;
                    }
                    _ = tokio::time::sleep(renew_interval) => {
                        let request = LeaseRenewRequest {
                            lease_id: lease_id.clone(),
                            ttl_secs: ttl,
                        };

                        match coordinator.renew(&request).await {
                            Ok(_) => {
                                consecutive_failures = 0;
                            }
                            Err(e) => {
                                consecutive_failures += 1;
                                error!(
                                    "Failed to renew lease for task {} (attempt {}/{}): {}",
                                    task_id, consecutive_failures, MAX_RENEWAL_RETRIES, e
                                );

                                if consecutive_failures >= MAX_RENEWAL_RETRIES {
                                    error!("Max renewal retries exceeded for task: {}", task_id);
                                    let _ = state.update_task_state(&task_id, AiTaskState::Error).await;
                                    break;
                                }

                                // Exponential backoff
                                let backoff = Duration::from_millis(
                                    RENEWAL_BACKOFF_BASE_MS * 2u64.pow(consecutive_failures - 1),
                                );
                                tokio::time::sleep(backoff).await;
                            }
                        }
                    }
                }
            }
        });
    }

    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down AI service...");

        // Cancel all renewal loops
        {
            let renewals = self.inner.renewals.read().await;
            for token in renewals.values() {
                token.cancel();
            }
        }

        // Release all leases and stop tasks
        let task_ids: Vec<String> = {
            let tasks = self.inner.tasks.read().await;
            tasks.keys().cloned().collect()
        };

        for task_id in task_ids {
            if let Err(e) = self.stop_task(&task_id).await {
                warn!("Error stopping task {} during shutdown: {}", task_id, e);
            }
        }

        // Shutdown all plugins
        self.inner.plugins.shutdown_all().await?;

        info!("AI service shutdown complete");
        Ok(())
    }
}

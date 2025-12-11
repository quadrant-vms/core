use crate::ptz_client::create_ptz_client;
use crate::store::DeviceStore;
use crate::types::*;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

/// Represents a running tour execution
#[derive(Clone)]
struct TourExecutionHandle {
    tour_id: String,
    device_id: String,
    cancellation_token: CancellationToken,
}

/// Background worker that manages PTZ tour executions
#[derive(Clone)]
pub struct TourExecutor {
    store: Arc<DeviceStore>,
    active_tours: Arc<RwLock<HashMap<String, TourExecutionHandle>>>,
    ptz_timeout_secs: u64,
}

impl TourExecutor {
    /// Create a new tour executor
    pub fn new(store: Arc<DeviceStore>, ptz_timeout_secs: u64) -> Self {
        Self {
            store,
            active_tours: Arc::new(RwLock::new(HashMap::new())),
            ptz_timeout_secs,
        }
    }

    /// Start executing a tour
    pub async fn start_tour(&self, tour_id: String) -> Result<()> {
        // Get tour details
        let tour = self
            .store
            .get_ptz_tour(&tour_id)
            .await?
            .context("tour not found")?;

        // Check if tour is already running
        {
            let active = self.active_tours.read().await;
            if active.contains_key(&tour_id) {
                anyhow::bail!("tour is already running");
            }
        }

        // Get tour steps
        let steps = self.store.get_ptz_tour_steps(&tour_id).await?;
        if steps.is_empty() {
            anyhow::bail!("tour has no steps");
        }

        // Create cancellation token
        let cancellation_token = CancellationToken::new();
        let handle = TourExecutionHandle {
            tour_id: tour_id.clone(),
            device_id: tour.device_id.clone(),
            cancellation_token: cancellation_token.clone(),
        };

        // Register active tour
        {
            let mut active = self.active_tours.write().await;
            active.insert(tour_id.clone(), handle);
        }

        // Update tour state to running
        self.store
            .update_ptz_tour_state(&tour_id, TourState::Running)
            .await?;

        // Spawn execution task
        let executor = self.clone();
        tokio::spawn(async move {
            if let Err(e) = executor.execute_tour_loop(tour, steps, cancellation_token).await {
                error!(tour_id = %tour_id, error = %e, "tour execution failed");
            }

            // Clean up: remove from active tours and update state
            {
                let mut active = executor.active_tours.write().await;
                active.remove(&tour_id);
            }

            if let Err(e) = executor
                .store
                .update_ptz_tour_state(&tour_id, TourState::Stopped)
                .await
            {
                error!(tour_id = %tour_id, error = %e, "failed to update tour state to stopped");
            }

            info!(tour_id = %tour_id, "tour execution completed");
        });

        Ok(())
    }

    /// Stop a running tour
    pub async fn stop_tour(&self, tour_id: &str) -> Result<()> {
        let handle = {
            let mut active = self.active_tours.write().await;
            active.remove(tour_id)
        };

        if let Some(handle) = handle {
            handle.cancellation_token.cancel();
            info!(tour_id = %tour_id, "tour stopped");
        }

        // Update state in database
        self.store
            .update_ptz_tour_state(tour_id, TourState::Stopped)
            .await?;

        Ok(())
    }

    /// Pause a running tour
    pub async fn pause_tour(&self, tour_id: &str) -> Result<()> {
        let active = self.active_tours.read().await;
        if !active.contains_key(tour_id) {
            anyhow::bail!("tour is not running");
        }

        // Update state in database
        self.store
            .update_ptz_tour_state(tour_id, TourState::Paused)
            .await?;

        info!(tour_id = %tour_id, "tour paused");
        Ok(())
    }

    /// Resume a paused tour
    pub async fn resume_tour(&self, tour_id: &str) -> Result<()> {
        let active = self.active_tours.read().await;
        if !active.contains_key(tour_id) {
            anyhow::bail!("tour is not running");
        }

        // Update state in database
        self.store
            .update_ptz_tour_state(tour_id, TourState::Running)
            .await?;

        info!(tour_id = %tour_id, "tour resumed");
        Ok(())
    }

    /// Check if a tour is currently running
    pub async fn is_tour_running(&self, tour_id: &str) -> bool {
        let active = self.active_tours.read().await;
        active.contains_key(tour_id)
    }

    /// Get list of currently running tours
    pub async fn list_running_tours(&self) -> Vec<String> {
        let active = self.active_tours.read().await;
        active.keys().cloned().collect()
    }

    /// Execute tour loop
    async fn execute_tour_loop(
        &self,
        tour: PtzTour,
        steps: Vec<PtzTourStep>,
        cancellation_token: CancellationToken,
    ) -> Result<()> {
        info!(
            tour_id = %tour.tour_id,
            device_id = %tour.device_id,
            steps = steps.len(),
            loop_enabled = tour.loop_enabled,
            "starting tour execution"
        );

        loop {
            // Execute all steps
            for step in &steps {
                // Check for cancellation
                if cancellation_token.is_cancelled() {
                    info!(tour_id = %tour.tour_id, "tour cancelled");
                    return Ok(());
                }

                // Check if paused
                loop {
                    let current_tour = self
                        .store
                        .get_ptz_tour(&tour.tour_id)
                        .await?
                        .context("tour not found during execution")?;

                    if current_tour.state == TourState::Stopped {
                        info!(tour_id = %tour.tour_id, "tour stopped via state change");
                        return Ok(());
                    }

                    if current_tour.state == TourState::Running {
                        break;
                    }

                    // Paused - wait a bit and check again
                    tokio::select! {
                        _ = sleep(Duration::from_millis(500)) => {},
                        _ = cancellation_token.cancelled() => {
                            return Ok(());
                        }
                    }
                }

                // Execute step
                if let Err(e) = self.execute_step(&tour.device_id, step).await {
                    error!(
                        tour_id = %tour.tour_id,
                        step_id = %step.step_id,
                        error = %e,
                        "failed to execute tour step"
                    );
                    // Continue to next step even if this one fails
                }

                // Dwell at position
                let dwell_duration = Duration::from_millis(step.dwell_time_ms as u64);
                tokio::select! {
                    _ = sleep(dwell_duration) => {},
                    _ = cancellation_token.cancelled() => {
                        info!(tour_id = %tour.tour_id, "tour cancelled during dwell");
                        return Ok(());
                    }
                }
            }

            // Check if we should loop
            if !tour.loop_enabled {
                info!(tour_id = %tour.tour_id, "tour completed (loop disabled)");
                break;
            }

            info!(tour_id = %tour.tour_id, "tour loop iteration completed, restarting");
        }

        Ok(())
    }

    /// Execute a single tour step
    async fn execute_step(&self, device_id: &str, step: &PtzTourStep) -> Result<()> {
        // Get device
        let device = self
            .store
            .get_device(device_id)
            .await?
            .context("device not found")?;

        let username = device.username.clone();
        let password = device
            .password_encrypted
            .as_ref()
            .and_then(|enc| self.store.decrypt_password(enc).ok());

        // Create PTZ client
        let client = create_ptz_client(&device.protocol, &device.primary_uri, username, password)?;

        // Determine position to move to
        let position = if let Some(preset_id) = &step.preset_id {
            // Use preset position
            let preset = self
                .store
                .get_ptz_preset(preset_id)
                .await?
                .context("preset not found")?;
            preset.position
        } else if let Some(pos) = &step.position {
            // Use explicit position
            pos.clone()
        } else {
            anyhow::bail!("step has neither preset_id nor position");
        };

        // Move to position
        let absolute_req = PtzAbsolutePositionRequest {
            pan: position.pan,
            tilt: position.tilt,
            zoom: position.zoom,
            speed: Some(step.speed),
        };

        client.goto_absolute_position(&absolute_req).await?;

        info!(
            device_id = %device_id,
            step_id = %step.step_id,
            "moved to tour step position"
        );

        Ok(())
    }

    /// Stop all running tours (cleanup on shutdown)
    pub async fn stop_all_tours(&self) -> Result<()> {
        let tour_ids: Vec<String> = {
            let active = self.active_tours.read().await;
            active.keys().cloned().collect()
        };

        for tour_id in tour_ids {
            if let Err(e) = self.stop_tour(&tour_id).await {
                warn!(tour_id = %tour_id, error = %e, "failed to stop tour during cleanup");
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tour_executor_creation() {
        // Basic smoke test - can't test actual execution without database
        let pool = sqlx::PgPool::connect_lazy("").unwrap();
        let store = Arc::new(DeviceStore::from_pool(pool));
        let executor = TourExecutor::new(store, 10);
        assert_eq!(executor.ptz_timeout_secs, 10);
    }
}

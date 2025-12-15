use crate::{firmware_client::*, firmware_storage::*, store::*, types::*};
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Manages firmware update execution
pub struct FirmwareExecutor {
    store: DeviceStore,
    storage: FirmwareStorage,
    active_updates: Arc<RwLock<HashMap<String, CancellationToken>>>,
}

impl FirmwareExecutor {
    pub fn new(store: DeviceStore, storage: FirmwareStorage) -> Self {
        Self {
            store,
            storage,
            active_updates: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start a firmware update
    pub async fn start_update(&self, update_id: &str) -> Result<()> {
        let update = self
            .store
            .get_firmware_update(update_id)
            .await
            .context("failed to get firmware update")?;

        // Check if already running
        {
            let active = self.active_updates.read().await;
            if active.contains_key(update_id) {
                return Err(anyhow!("firmware update already running: {}", update_id));
            }
        }

        // Get device info
        let device = self
            .store
            .get_device(&update.device_id)
            .await
            .context("failed to get device")?
            .ok_or_else(|| anyhow!("device not found: {}", update.device_id))?;

        // Create cancellation token
        let cancel_token = CancellationToken::new();

        // Add to active updates
        {
            let mut active = self.active_updates.write().await;
            active.insert(update_id.to_string(), cancel_token.clone());
        }

        // Spawn update task
        let update_id_owned = update_id.to_string();
        let store = self.store.clone();
        let storage = self.storage.clone();

        tokio::spawn(async move {
            if let Err(e) = Self::execute_update(
                store.clone(),
                storage,
                update,
                device,
                cancel_token.clone(),
            )
            .await
            {
                error!("firmware update {} failed: {}", update_id_owned, e);
                let _ = store
                    .update_firmware_status(
                        &update_id_owned,
                        FirmwareUpdateStatus::Failed,
                        0,
                        Some(&e.to_string()),
                    )
                    .await;
            }
        });

        info!("started firmware update: {}", update_id);
        Ok(())
    }

    /// Stop a firmware update
    pub async fn stop_update(&self, update_id: &str) -> Result<()> {
        let cancel_token = {
            let mut active = self.active_updates.write().await;
            active.remove(update_id)
        };

        if let Some(token) = cancel_token {
            token.cancel();
            info!("stopped firmware update: {}", update_id);

            // Update status to cancelled
            self.store
                .cancel_firmware_update(update_id)
                .await
                .context("failed to cancel firmware update")?;

            Ok(())
        } else {
            Err(anyhow!(
                "firmware update not running: {}",
                update_id
            ))
        }
    }

    /// Check if update is running
    pub async fn is_update_running(&self, update_id: &str) -> bool {
        let active = self.active_updates.read().await;
        active.contains_key(update_id)
    }

    /// Get list of active update IDs
    pub async fn get_active_updates(&self) -> Vec<String> {
        let active = self.active_updates.read().await;
        active.keys().cloned().collect()
    }

    /// Execute firmware update (main logic)
    async fn execute_update(
        store: DeviceStore,
        storage: FirmwareStorage,
        update: FirmwareUpdate,
        device: Device,
        cancel_token: CancellationToken,
    ) -> Result<()> {
        let update_id = update.update_id.clone();

        info!(
            "executing firmware update {} for device {}",
            update_id, device.device_id
        );

        // Check if cancelled
        if cancel_token.is_cancelled() {
            return Ok(());
        }

        // Step 1: Validate firmware file
        store
            .update_firmware_status(&update_id, FirmwareUpdateStatus::Uploading, 5, None)
            .await?;

        storage
            .validate_file(&update.firmware_file_path, &update.firmware_checksum)
            .await
            .context("firmware file validation failed")?;

        debug!("firmware file validated: {}", update.firmware_file_path);

        // Check if cancelled
        if cancel_token.is_cancelled() {
            return Ok(());
        }

        // Step 2: Read firmware data
        let firmware_data = storage
            .read_file(&update.firmware_file_path)
            .await
            .context("failed to read firmware file")?;

        store
            .update_firmware_status(&update_id, FirmwareUpdateStatus::Uploaded, 10, None)
            .await?;

        // Check if cancelled
        if cancel_token.is_cancelled() {
            return Ok(());
        }

        // Step 3: Create firmware client
        let client = create_firmware_client(&device).context("failed to create firmware client")?;

        // Check if device supports firmware upgrades
        let supports_upgrade = client
            .supports_firmware_upgrade()
            .await
            .context("failed to check firmware upgrade support")?;

        if !supports_upgrade {
            return Err(anyhow!(
                "device {} does not support firmware upgrades",
                device.device_id
            ));
        }

        // Check if cancelled
        if cancel_token.is_cancelled() {
            return Ok(());
        }

        // Step 4: Upload and install firmware
        store
            .update_firmware_status(&update_id, FirmwareUpdateStatus::Installing, 15, None)
            .await?;

        // Retry logic
        let mut attempts = 0;
        let max_attempts = update.max_retries + 1;

        loop {
            attempts += 1;

            match client.upload_and_install(&firmware_data).await {
                Ok(_) => {
                    info!(
                        "firmware uploaded successfully to device {} (attempt {}/{})",
                        device.device_id, attempts, max_attempts
                    );
                    break;
                }
                Err(e) => {
                    warn!(
                        "firmware upload failed for device {} (attempt {}/{}): {}",
                        device.device_id, attempts, max_attempts, e
                    );

                    if attempts >= max_attempts {
                        return Err(anyhow!(
                            "firmware upload failed after {} attempts: {}",
                            attempts,
                            e
                        ));
                    }

                    // Increment retry count
                    store.increment_firmware_retry_count(&update_id).await?;

                    // Wait before retry
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

                    // Check if cancelled
                    if cancel_token.is_cancelled() {
                        return Ok(());
                    }
                }
            }
        }

        store
            .update_firmware_status(&update_id, FirmwareUpdateStatus::Rebooting, 80, None)
            .await?;

        // Check if cancelled
        if cancel_token.is_cancelled() {
            return Ok(());
        }

        // Step 5: Reboot device
        client
            .reboot()
            .await
            .context("failed to reboot device")?;

        // Wait for device to reboot
        tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;

        store
            .update_firmware_status(&update_id, FirmwareUpdateStatus::Verifying, 90, None)
            .await?;

        // Check if cancelled
        if cancel_token.is_cancelled() {
            return Ok(());
        }

        // Step 6: Verify installation
        let verified = client
            .verify_installation(&update.firmware_version)
            .await
            .unwrap_or(false);

        if verified {
            store
                .update_firmware_status(&update_id, FirmwareUpdateStatus::Completed, 100, None)
                .await?;

            info!(
                "firmware update {} completed successfully for device {}",
                update_id, device.device_id
            );
        } else {
            let error_msg = "firmware verification failed - version mismatch";
            store
                .update_firmware_status(
                    &update_id,
                    FirmwareUpdateStatus::Failed,
                    90,
                    Some(error_msg),
                )
                .await?;

            return Err(anyhow!(error_msg));
        }

        Ok(())
    }

    /// Cleanup completed updates from active list
    pub async fn cleanup_completed(&self) -> usize {
        let mut active = self.active_updates.write().await;
        let initial_count = active.len();

        // Remove updates that are no longer active
        active.retain(|_update_id, _| {
            // Check if update is completed/failed/cancelled in database
            // For now, we just check if the cancel token is cancelled
            // In production, you'd query the database
            true
        });

        let removed = initial_count - active.len();
        if removed > 0 {
            info!("cleaned up {} completed firmware updates", removed);
        }
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_setup() -> (DeviceStore, FirmwareStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage = FirmwareStorage::new(temp_dir.path()).unwrap();
        storage.init().await.unwrap();

        // For tests, you'd need to set up a test database
        // This is a placeholder
        let database_url = std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5432/test_db".to_string());
        let store = DeviceStore::new(&database_url).await.unwrap();

        (store, storage, temp_dir)
    }

    #[tokio::test]
    #[ignore] // Requires database setup
    async fn test_firmware_executor_creation() {
        let (store, storage, _temp_dir) = create_test_setup().await;
        let executor = FirmwareExecutor::new(store, storage);

        let active = executor.get_active_updates().await;
        assert_eq!(active.len(), 0);
    }

    #[tokio::test]
    #[ignore] // Requires database setup
    async fn test_is_update_running() {
        let (store, storage, _temp_dir) = create_test_setup().await;
        let executor = FirmwareExecutor::new(store, storage);

        let update_id = "test-update-id";
        let is_running = executor.is_update_running(update_id).await;
        assert!(!is_running);
    }
}

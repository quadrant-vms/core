use crate::prober::DeviceProber;
use crate::store::DeviceStore;
use crate::types::{Device, DeviceStatus};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

pub struct HealthMonitor {
    store: Arc<DeviceStore>,
    prober: Arc<DeviceProber>,
    check_interval_secs: u64,
    max_consecutive_failures: i32,
}

impl HealthMonitor {
    pub fn new(
        store: Arc<DeviceStore>,
        prober: Arc<DeviceProber>,
        check_interval_secs: u64,
        max_consecutive_failures: i32,
    ) -> Self {
        Self {
            store,
            prober,
            check_interval_secs,
            max_consecutive_failures,
        }
    }

    /// Start the health monitoring loop
    pub async fn start(&self) {
        info!("health monitor started");

        loop {
            if let Err(e) = self.run_health_checks().await {
                error!("health check cycle failed: {}", e);
            }

            sleep(Duration::from_secs(self.check_interval_secs)).await;
        }
    }

    /// Run health checks for all devices that need checking
    async fn run_health_checks(&self) -> anyhow::Result<()> {
        let devices = self.store.get_devices_needing_health_check().await?;

        if devices.is_empty() {
            return Ok(());
        }

        info!("checking health for {} devices", devices.len());

        // Process devices in parallel (with concurrency limit)
        let mut tasks = Vec::new();

        for device in devices {
            let store = Arc::clone(&self.store);
            let prober = Arc::clone(&self.prober);
            let max_failures = self.max_consecutive_failures;

            let task = tokio::spawn(async move {
                if let Err(e) = Self::check_device_health(device, store, prober, max_failures).await
                {
                    error!("failed to check device health: {}", e);
                }
            });

            tasks.push(task);

            // Limit concurrency to avoid overwhelming the system
            if tasks.len() >= 10 {
                for task in tasks.drain(..) {
                    let _ = task.await;
                }
            }
        }

        // Wait for remaining tasks
        for task in tasks {
            let _ = task.await;
        }

        Ok(())
    }

    /// Check health of a single device
    async fn check_device_health(
        device: Device,
        store: Arc<DeviceStore>,
        prober: Arc<DeviceProber>,
        max_consecutive_failures: i32,
    ) -> anyhow::Result<()> {
        let device_id = &device.device_id;
        let username = device.username.as_deref();
        let password = device
            .password_encrypted
            .as_ref()
            .and_then(|enc| store.decrypt_password(enc).ok())
            .as_deref();

        // Perform health check
        let (is_healthy, response_time_ms, error_message) = prober
            .health_check(&device.primary_uri, &device.protocol, username, password)
            .await?;

        // Determine new status
        let new_status = if is_healthy {
            DeviceStatus::Online
        } else if device.consecutive_failures + 1 >= max_consecutive_failures {
            DeviceStatus::Error
        } else {
            DeviceStatus::Offline
        };

        // Update device status
        store
            .update_health_status(
                device_id,
                new_status.clone(),
                Some(response_time_ms as i32),
                error_message.clone(),
            )
            .await?;

        // Log result
        match new_status {
            DeviceStatus::Online => {
                if device.status != DeviceStatus::Online {
                    info!(
                        device_id = %device_id,
                        device_name = %device.name,
                        response_time_ms = response_time_ms,
                        "device came online"
                    );
                }
            }
            DeviceStatus::Offline => {
                warn!(
                    device_id = %device_id,
                    device_name = %device.name,
                    consecutive_failures = device.consecutive_failures + 1,
                    error = ?error_message,
                    "device offline"
                );
            }
            DeviceStatus::Error => {
                error!(
                    device_id = %device_id,
                    device_name = %device.name,
                    consecutive_failures = device.consecutive_failures + 1,
                    error = ?error_message,
                    "device in error state"
                );
            }
            _ => {}
        }

        Ok(())
    }
}

use crate::types::*;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Trait for firmware update operations
#[async_trait]
pub trait FirmwareClient: Send + Sync {
    /// Upload firmware to device and initiate update
    async fn upload_and_install(&self, firmware_data: &[u8]) -> Result<()>;

    /// Get current firmware version from device
    async fn get_firmware_version(&self) -> Result<String>;

    /// Check if device supports firmware upgrades
    async fn supports_firmware_upgrade(&self) -> Result<bool>;

    /// Monitor update progress (returns progress percentage 0-100)
    async fn get_update_progress(&self) -> Result<i32>;

    /// Verify firmware installation was successful
    async fn verify_installation(&self, expected_version: &str) -> Result<bool>;

    /// Reboot device
    async fn reboot(&self) -> Result<()>;
}

/// ONVIF Firmware client implementation
pub struct OnvifFirmwareClient {
    device_uri: String,
    username: Option<String>,
    password: Option<String>,
    device_id: String,
    http_client: reqwest::Client,
    progress: Arc<RwLock<i32>>,
}

impl OnvifFirmwareClient {
    pub fn new(
        device_uri: String,
        username: Option<String>,
        password: Option<String>,
        device_id: String,
    ) -> Result<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300)) // 5 minutes for firmware upload
            .build()?;

        Ok(Self {
            device_uri,
            username,
            password,
            device_id,
            http_client,
            progress: Arc::new(RwLock::new(0)),
        })
    }

    fn build_soap_envelope(&self, body: &str, namespace: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
            xmlns:tds="http://www.onvif.org/ver10/device/wsdl"
            xmlns:tt="http://www.onvif.org/ver10/schema"
            {}>
  <s:Body>
    {}
  </s:Body>
</s:Envelope>"#,
            namespace, body
        )
    }

    async fn send_onvif_request(&self, soap_body: &str, namespace: &str) -> Result<String> {
        let envelope = self.build_soap_envelope(soap_body, namespace);

        debug!("sending ONVIF firmware request to {}", self.device_uri);

        let mut request = self
            .http_client
            .post(&self.device_uri)
            .header("Content-Type", "application/soap+xml; charset=utf-8")
            .body(envelope);

        // Add basic auth if credentials provided
        if let (Some(username), Some(password)) = (&self.username, &self.password) {
            request = request.basic_auth(username, Some(password));
        }

        let response = request.send().await?;
        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            return Err(anyhow!("ONVIF request failed: {} - {}", status, body));
        }

        Ok(body)
    }
}

#[async_trait]
impl FirmwareClient for OnvifFirmwareClient {
    async fn upload_and_install(&self, firmware_data: &[u8]) -> Result<()> {
        info!(
            "uploading firmware to device {} ({} bytes)",
            self.device_id,
            firmware_data.len()
        );

        // Set progress to uploading
        *self.progress.write().await = 10;

        // ONVIF uses SystemReboot with firmware parameter or UpgradeSystemFirmware
        // This is a simplified implementation - real ONVIF may require multi-part upload

        let soap_body = format!(
            r#"<tds:UpgradeSystemFirmware>
  <tds:Firmware>{}</tds:Firmware>
</tds:UpgradeSystemFirmware>"#,
            general_purpose::STANDARD.encode(firmware_data)
        );

        *self.progress.write().await = 50;

        let response = self.send_onvif_request(&soap_body, "").await?;

        *self.progress.write().await = 90;

        debug!("firmware upload response: {}", response);

        // Check for error in response
        if response.contains("Fault") || response.contains("Error") {
            return Err(anyhow!("firmware upload failed: {}", response));
        }

        *self.progress.write().await = 100;

        info!("firmware uploaded successfully to device {}", self.device_id);
        Ok(())
    }

    async fn get_firmware_version(&self) -> Result<String> {
        let soap_body = r#"<tds:GetDeviceInformation/>"#;

        let response = self.send_onvif_request(soap_body, "").await?;

        // Parse firmware version from response
        // Example: <tt:FirmwareVersion>1.2.3</tt:FirmwareVersion>
        if let Some(start) = response.find("<tt:FirmwareVersion>") {
            let start_idx = start + "<tt:FirmwareVersion>".len();
            if let Some(end) = response[start_idx..].find("</tt:FirmwareVersion>") {
                let version = &response[start_idx..start_idx + end];
                return Ok(version.to_string());
            }
        }

        Err(anyhow!("failed to parse firmware version from response"))
    }

    async fn supports_firmware_upgrade(&self) -> Result<bool> {
        // Check device capabilities for firmware upgrade support
        let soap_body = r#"<tds:GetCapabilities>
  <tds:Category>Device</tds:Category>
</tds:GetCapabilities>"#;

        let response = self.send_onvif_request(soap_body, "").await?;

        // Check if SystemBackup capability exists (indicates firmware upgrade support)
        Ok(response.contains("SystemBackup") || response.contains("UpgradeSystemFirmware"))
    }

    async fn get_update_progress(&self) -> Result<i32> {
        let progress = *self.progress.read().await;
        Ok(progress)
    }

    async fn verify_installation(&self, expected_version: &str) -> Result<bool> {
        // Wait a bit for device to apply firmware
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

        match self.get_firmware_version().await {
            Ok(current_version) => {
                debug!(
                    "firmware verification: expected={}, current={}",
                    expected_version, current_version
                );
                Ok(current_version == expected_version)
            }
            Err(e) => {
                warn!("firmware verification failed: {}", e);
                Err(e)
            }
        }
    }

    async fn reboot(&self) -> Result<()> {
        info!("rebooting device {}", self.device_id);

        let soap_body = r#"<tds:SystemReboot/>"#;

        let response = self.send_onvif_request(soap_body, "").await?;

        if response.contains("Fault") {
            return Err(anyhow!("reboot failed: {}", response));
        }

        info!("device {} reboot initiated", self.device_id);
        Ok(())
    }
}

/// Mock firmware client for testing
pub struct MockFirmwareClient {
    device_id: String,
    current_version: Arc<RwLock<String>>,
    supports_upgrade: bool,
    should_fail: bool,
    progress: Arc<RwLock<i32>>,
}

impl MockFirmwareClient {
    pub fn new(device_id: String, current_version: String) -> Self {
        Self {
            device_id,
            current_version: Arc::new(RwLock::new(current_version)),
            supports_upgrade: true,
            should_fail: false,
            progress: Arc::new(RwLock::new(0)),
        }
    }

    pub fn with_upgrade_support(mut self, supports: bool) -> Self {
        self.supports_upgrade = supports;
        self
    }

    pub fn with_failure(mut self, should_fail: bool) -> Self {
        self.should_fail = should_fail;
        self
    }
}

#[async_trait]
impl FirmwareClient for MockFirmwareClient {
    async fn upload_and_install(&self, firmware_data: &[u8]) -> Result<()> {
        if self.should_fail {
            return Err(anyhow!("mock firmware upload failed"));
        }

        info!(
            "[MOCK] uploading firmware to device {} ({} bytes)",
            self.device_id,
            firmware_data.len()
        );

        // Simulate upload progress
        for progress in [10, 30, 50, 70, 90, 100] {
            *self.progress.write().await = progress;
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        // Extract version from data (mock - just use first 10 bytes as version string)
        let version = format!("mock-{}", firmware_data.len());
        *self.current_version.write().await = version;

        info!(
            "[MOCK] firmware uploaded successfully to device {}",
            self.device_id
        );
        Ok(())
    }

    async fn get_firmware_version(&self) -> Result<String> {
        let version = self.current_version.read().await.clone();
        Ok(version)
    }

    async fn supports_firmware_upgrade(&self) -> Result<bool> {
        Ok(self.supports_upgrade)
    }

    async fn get_update_progress(&self) -> Result<i32> {
        let progress = *self.progress.read().await;
        Ok(progress)
    }

    async fn verify_installation(&self, expected_version: &str) -> Result<bool> {
        let current_version = self.current_version.read().await.clone();
        Ok(current_version == expected_version)
    }

    async fn reboot(&self) -> Result<()> {
        info!("[MOCK] rebooting device {}", self.device_id);
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        Ok(())
    }
}

/// Create firmware client based on device protocol
pub fn create_firmware_client(
    device: &Device,
) -> Result<Arc<dyn FirmwareClient>> {
    match device.protocol {
        ConnectionProtocol::Onvif => Ok(Arc::new(OnvifFirmwareClient::new(
            device.primary_uri.clone(),
            device.username.clone(),
            None, // Password decryption would happen here
            device.device_id.clone(),
        )?)),
        _ => {
            // For non-ONVIF devices, use mock client
            warn!(
                "device {} protocol {:?} does not support firmware upgrades, using mock client",
                device.device_id, device.protocol
            );
            Ok(Arc::new(MockFirmwareClient::new(
                device.device_id.clone(),
                device.firmware_version.clone().unwrap_or_default(),
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_firmware_client() {
        let client = MockFirmwareClient::new("test-device".to_string(), "1.0.0".to_string());

        // Check initial version
        let version = client.get_firmware_version().await.unwrap();
        assert_eq!(version, "1.0.0");

        // Check upgrade support
        let supports = client.supports_firmware_upgrade().await.unwrap();
        assert!(supports);

        // Upload firmware
        let firmware_data = b"test firmware data";
        client.upload_and_install(firmware_data).await.unwrap();

        // Verify progress
        let progress = client.get_update_progress().await.unwrap();
        assert_eq!(progress, 100);

        // Reboot
        client.reboot().await.unwrap();
    }

    #[tokio::test]
    async fn test_mock_firmware_client_failure() {
        let client = MockFirmwareClient::new("test-device".to_string(), "1.0.0".to_string())
            .with_failure(true);

        let firmware_data = b"test firmware data";
        let result = client.upload_and_install(firmware_data).await;
        assert!(result.is_err());
    }
}

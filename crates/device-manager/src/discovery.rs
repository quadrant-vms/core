use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

const WS_DISCOVERY_MULTICAST_ADDR: &str = "239.255.255.250:3702";
const WS_DISCOVERY_TIMEOUT_SECS: u64 = 5;
const MAX_DISCOVERY_RESULTS: usize = 100;

/// WS-Discovery probe message for ONVIF devices
const WS_DISCOVERY_PROBE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" xmlns:a="http://schemas.xmlsoap.org/ws/2004/08/addressing">
  <s:Header>
    <a:Action s:mustUnderstand="1">http://schemas.xmlsoap.org/ws/2005/04/discovery/Probe</a:Action>
    <a:MessageID>uuid:{message_id}</a:MessageID>
    <a:ReplyTo>
      <a:Address>http://schemas.xmlsoap.org/ws/2004/08/addressing/role/anonymous</a:Address>
    </a:ReplyTo>
    <a:To s:mustUnderstand="1">urn:schemas-xmlsoap-org:ws:2005:04:discovery</a:To>
  </s:Header>
  <s:Body>
    <Probe xmlns="http://schemas.xmlsoap.org/ws/2005/04/discovery">
      <d:Types xmlns:d="http://schemas.xmlsoap.org/ws/2005/04/discovery" xmlns:dp0="http://www.onvif.org/ver10/network/wsdl">dp0:NetworkVideoTransmitter</d:Types>
    </Probe>
  </s:Body>
</s:Envelope>"#;

/// Result of a single discovered device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredDevice {
    pub device_service_url: String,
    pub scopes: Vec<String>,
    pub types: Vec<String>,
    pub xaddrs: Vec<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub hardware_id: Option<String>,
    pub name: Option<String>,
    pub location: Option<String>,
    pub discovered_at: chrono::DateTime<chrono::Utc>,
}

/// Discovery scan session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryScan {
    pub scan_id: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub devices_found: usize,
    pub status: DiscoveryScanStatus,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DiscoveryScanStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Discovery result containing all found devices
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryResult {
    pub scan_id: String,
    pub devices: Vec<DiscoveredDevice>,
    pub scan_duration_secs: u64,
    pub total_found: usize,
}

/// ONVIF device discovery client using WS-Discovery
pub struct OnvifDiscoveryClient {
    timeout_secs: u64,
    active_scans: Arc<RwLock<HashMap<String, DiscoveryScan>>>,
}

impl OnvifDiscoveryClient {
    pub fn new(timeout_secs: u64) -> Self {
        Self {
            timeout_secs,
            active_scans: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start a new discovery scan
    pub async fn start_scan(&self) -> Result<String> {
        let scan_id = Uuid::new_v4().to_string();
        let scan = DiscoveryScan {
            scan_id: scan_id.clone(),
            started_at: chrono::Utc::now(),
            completed_at: None,
            devices_found: 0,
            status: DiscoveryScanStatus::Running,
            error_message: None,
        };

        {
            let mut scans = self.active_scans.write().await;
            scans.insert(scan_id.clone(), scan);
        }

        info!(scan_id = %scan_id, "discovery scan started");
        Ok(scan_id)
    }

    /// Perform WS-Discovery to find ONVIF devices on the network
    pub async fn discover_devices(&self, scan_id: &str) -> Result<DiscoveryResult> {
        let start_time = std::time::Instant::now();

        info!(scan_id = %scan_id, "starting WS-Discovery probe");

        // Update scan status
        {
            let mut scans = self.active_scans.write().await;
            if let Some(scan) = scans.get_mut(scan_id) {
                scan.status = DiscoveryScanStatus::Running;
            }
        }

        // Perform discovery
        let devices = match self.send_ws_discovery_probe().await {
            Ok(devices) => devices,
            Err(e) => {
                error!(scan_id = %scan_id, error = %e, "discovery failed");
                self.update_scan_status(scan_id, DiscoveryScanStatus::Failed, Some(e.to_string()))
                    .await;
                return Err(e);
            }
        };

        let scan_duration = start_time.elapsed().as_secs();
        let total_found = devices.len();

        // Update scan status
        self.update_scan_status(scan_id, DiscoveryScanStatus::Completed, None)
            .await;

        {
            let mut scans = self.active_scans.write().await;
            if let Some(scan) = scans.get_mut(scan_id) {
                scan.devices_found = total_found;
                scan.completed_at = Some(chrono::Utc::now());
            }
        }

        info!(
            scan_id = %scan_id,
            devices_found = total_found,
            duration_secs = scan_duration,
            "discovery scan completed"
        );

        Ok(DiscoveryResult {
            scan_id: scan_id.to_string(),
            devices,
            scan_duration_secs: scan_duration,
            total_found,
        })
    }

    /// Send WS-Discovery probe message via UDP multicast
    async fn send_ws_discovery_probe(&self) -> Result<Vec<DiscoveredDevice>> {
        let message_id = Uuid::new_v4().to_string();
        let probe_message = WS_DISCOVERY_PROBE.replace("{message_id}", &message_id);

        // Create UDP socket
        let socket = UdpSocket::bind("0.0.0.0:0").context("failed to bind UDP socket")?;
        socket
            .set_read_timeout(Some(Duration::from_secs(self.timeout_secs)))
            .context("failed to set socket timeout")?;
        socket
            .set_broadcast(true)
            .context("failed to enable broadcast")?;

        // Send probe to multicast address
        let multicast_addr: SocketAddr = WS_DISCOVERY_MULTICAST_ADDR
            .parse()
            .context("invalid multicast address")?;

        debug!(
            multicast_addr = %multicast_addr,
            "sending WS-Discovery probe"
        );

        socket
            .send_to(probe_message.as_bytes(), multicast_addr)
            .context("failed to send probe message")?;

        // Collect responses
        let mut devices = Vec::new();
        let mut buffer = [0u8; 65535];
        let deadline = std::time::Instant::now() + Duration::from_secs(self.timeout_secs);

        while std::time::Instant::now() < deadline && devices.len() < MAX_DISCOVERY_RESULTS {
            match socket.recv_from(&mut buffer) {
                Ok((size, src_addr)) => {
                    let response = String::from_utf8_lossy(&buffer[..size]);
                    debug!(
                        src_addr = %src_addr,
                        response_size = size,
                        "received WS-Discovery response"
                    );

                    if let Some(device) = self.parse_probe_match(&response, src_addr) {
                        devices.push(device);
                        info!(
                            device_count = devices.len(),
                            "discovered ONVIF device"
                        );
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // Timeout reached, no more responses
                    break;
                }
                Err(e) => {
                    warn!(error = %e, "error receiving UDP response");
                    break;
                }
            }
        }

        info!(total_devices = devices.len(), "WS-Discovery probe completed");
        Ok(devices)
    }

    /// Parse WS-Discovery ProbeMatch response
    fn parse_probe_match(&self, xml: &str, _src_addr: SocketAddr) -> Option<DiscoveredDevice> {
        // Check if this is a ProbeMatch response
        if !xml.contains("ProbeMatch") {
            return None;
        }

        // Extract XAddrs (device service URLs)
        let xaddrs = self.extract_xml_content(xml, "XAddrs")?;
        let xaddr_list: Vec<String> = xaddrs
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        if xaddr_list.is_empty() {
            warn!("ProbeMatch without XAddrs");
            return None;
        }

        // Extract Scopes (contains device metadata)
        let scopes_raw = self.extract_xml_content(xml, "Scopes").unwrap_or_default();
        let scopes: Vec<String> = scopes_raw
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        // Extract Types
        let types_raw = self.extract_xml_content(xml, "Types").unwrap_or_default();
        let types: Vec<String> = types_raw
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        // Parse scopes for metadata
        let manufacturer = self.extract_scope_value(&scopes, "name");
        let model = self.extract_scope_value(&scopes, "hardware");
        let hardware_id = self.extract_scope_value(&scopes, "hardware");
        let name = self.extract_scope_value(&scopes, "name");
        let location = self.extract_scope_value(&scopes, "location");

        Some(DiscoveredDevice {
            device_service_url: xaddr_list.first().unwrap().clone(),
            scopes,
            types,
            xaddrs: xaddr_list,
            manufacturer,
            model,
            hardware_id,
            name,
            location,
            discovered_at: chrono::Utc::now(),
        })
    }

    /// Extract XML element content (simple parser, not full XML)
    fn extract_xml_content(&self, xml: &str, tag: &str) -> Option<String> {
        let start_tag = format!("<{}", tag);
        let end_tag = format!("</{}>", tag);

        let start_pos = xml.find(&start_tag)?;
        let content_start = xml[start_pos..].find('>')? + start_pos + 1;
        let content_end = xml[content_start..].find(&end_tag)? + content_start;

        Some(xml[content_start..content_end].trim().to_string())
    }

    /// Extract value from ONVIF scope URL
    fn extract_scope_value(&self, scopes: &[String], key: &str) -> Option<String> {
        for scope in scopes {
            if let Some(pos) = scope.find(&format!("/{}/", key)) {
                let value_start = pos + key.len() + 2;
                let value = scope[value_start..]
                    .split('/')
                    .next()
                    .unwrap_or("")
                    .to_string();
                if !value.is_empty() {
                    return Some(value);
                }
            }
        }
        None
    }

    /// Get scan status
    pub async fn get_scan_status(&self, scan_id: &str) -> Option<DiscoveryScan> {
        let scans = self.active_scans.read().await;
        scans.get(scan_id).cloned()
    }

    /// Cancel an active scan
    pub async fn cancel_scan(&self, scan_id: &str) -> Result<()> {
        let mut scans = self.active_scans.write().await;
        if let Some(scan) = scans.get_mut(scan_id) {
            if scan.status == DiscoveryScanStatus::Running {
                scan.status = DiscoveryScanStatus::Cancelled;
                scan.completed_at = Some(chrono::Utc::now());
                info!(scan_id = %scan_id, "discovery scan cancelled");
                return Ok(());
            }
        }
        anyhow::bail!("scan not found or not running")
    }

    /// List all scans
    pub async fn list_scans(&self) -> Vec<DiscoveryScan> {
        let scans = self.active_scans.read().await;
        scans.values().cloned().collect()
    }

    /// Update scan status
    async fn update_scan_status(
        &self,
        scan_id: &str,
        status: DiscoveryScanStatus,
        error_message: Option<String>,
    ) {
        let mut scans = self.active_scans.write().await;
        if let Some(scan) = scans.get_mut(scan_id) {
            scan.status = status;
            scan.error_message = error_message;
            if matches!(
                scan.status,
                DiscoveryScanStatus::Completed
                    | DiscoveryScanStatus::Failed
                    | DiscoveryScanStatus::Cancelled
            ) {
                scan.completed_at = Some(chrono::Utc::now());
            }
        }
    }

    /// Get device information via ONVIF GetDeviceInformation
    pub async fn get_device_info(&self, device_url: &str) -> Result<HashMap<String, String>> {
        let soap_body = r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">
  <s:Body xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
    <tds:GetDeviceInformation/>
  </s:Body>
</s:Envelope>"#;

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(self.timeout_secs))
            .build()
            .context("failed to create HTTP client")?;

        let response = client
            .post(device_url)
            .header("Content-Type", "application/soap+xml; charset=utf-8")
            .body(soap_body)
            .send()
            .await
            .context("failed to send GetDeviceInformation request")?;

        let response_text = response
            .text()
            .await
            .context("failed to read response body")?;

        // Parse device information
        let mut info = HashMap::new();
        if let Some(manufacturer) = self.extract_xml_content(&response_text, "Manufacturer") {
            info.insert("Manufacturer".to_string(), manufacturer);
        }
        if let Some(model) = self.extract_xml_content(&response_text, "Model") {
            info.insert("Model".to_string(), model);
        }
        if let Some(firmware) = self.extract_xml_content(&response_text, "FirmwareVersion") {
            info.insert("FirmwareVersion".to_string(), firmware);
        }
        if let Some(serial) = self.extract_xml_content(&response_text, "SerialNumber") {
            info.insert("SerialNumber".to_string(), serial);
        }
        if let Some(hardware) = self.extract_xml_content(&response_text, "HardwareId") {
            info.insert("HardwareId".to_string(), hardware);
        }

        Ok(info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_discovery_client_creation() {
        let client = OnvifDiscoveryClient::new(5);
        assert_eq!(client.timeout_secs, 5);
    }

    #[tokio::test]
    async fn test_start_scan() {
        let client = OnvifDiscoveryClient::new(5);
        let scan_id = client.start_scan().await.unwrap();
        assert!(!scan_id.is_empty());

        let scan = client.get_scan_status(&scan_id).await.unwrap();
        assert_eq!(scan.status, DiscoveryScanStatus::Running);
        assert_eq!(scan.devices_found, 0);
    }

    #[tokio::test]
    async fn test_cancel_scan() {
        let client = OnvifDiscoveryClient::new(5);
        let scan_id = client.start_scan().await.unwrap();

        client.cancel_scan(&scan_id).await.unwrap();

        let scan = client.get_scan_status(&scan_id).await.unwrap();
        assert_eq!(scan.status, DiscoveryScanStatus::Cancelled);
    }

    #[test]
    fn test_xml_content_extraction() {
        let client = OnvifDiscoveryClient::new(5);
        let xml = r#"<root><Name>TestDevice</Name><Model>Camera123</Model></root>"#;

        assert_eq!(
            client.extract_xml_content(xml, "Name"),
            Some("TestDevice".to_string())
        );
        assert_eq!(
            client.extract_xml_content(xml, "Model"),
            Some("Camera123".to_string())
        );
    }

    #[test]
    fn test_scope_value_extraction() {
        let client = OnvifDiscoveryClient::new(5);
        let scopes = vec![
            "onvif://www.onvif.org/name/TestCamera".to_string(),
            "onvif://www.onvif.org/hardware/Model123".to_string(),
            "onvif://www.onvif.org/location/Building1".to_string(),
        ];

        assert_eq!(
            client.extract_scope_value(&scopes, "name"),
            Some("TestCamera".to_string())
        );
        assert_eq!(
            client.extract_scope_value(&scopes, "hardware"),
            Some("Model123".to_string())
        );
        assert_eq!(
            client.extract_scope_value(&scopes, "location"),
            Some("Building1".to_string())
        );
    }
}

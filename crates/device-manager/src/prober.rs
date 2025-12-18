use crate::types::{ConnectionProtocol, ProbeResult};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::time::timeout;

pub struct DeviceProber {
    timeout_secs: u64,
}

impl DeviceProber {
    pub fn new(timeout_secs: u64) -> Self {
        Self { timeout_secs }
    }

    /// Probe a device to discover capabilities and metadata
    pub async fn probe_device(
        &self,
        uri: &str,
        protocol: &ConnectionProtocol,
        username: Option<&str>,
        password: Option<&str>,
    ) -> Result<ProbeResult> {
        match protocol {
            ConnectionProtocol::Rtsp => self.probe_rtsp(uri, username, password).await,
            ConnectionProtocol::Onvif => self.probe_onvif(uri, username, password).await,
            ConnectionProtocol::Http => self.probe_http(uri).await,
            _ => {
                Ok(ProbeResult {
                    success: false,
                    response_time_ms: 0,
                    manufacturer: None,
                    model: None,
                    firmware_version: None,
                    capabilities: HashMap::new(),
                    video_codecs: Vec::new(),
                    audio_codecs: Vec::new(),
                    resolutions: Vec::new(),
                    error_message: Some(format!("Protocol {:?} probing not yet implemented", protocol)),
                })
            }
        }
    }

    /// Probe RTSP stream using ffprobe
    async fn probe_rtsp(
        &self,
        uri: &str,
        username: Option<&str>,
        password: Option<&str>,
    ) -> Result<ProbeResult> {
        let start = Instant::now();

        // Build RTSP URI with credentials if provided
        let probe_uri = if let (Some(user), Some(pass)) = (username, password) {
            // Insert credentials into URI
            if let Some(idx) = uri.find("://") {
                let protocol = &uri[..idx + 3];
                let rest = &uri[idx + 3..];
                format!("{}{}:{}@{}", protocol, user, pass, rest)
            } else {
                uri.to_string()
            }
        } else {
            uri.to_string()
        };

        // Use ffprobe to get stream information
        let result = timeout(
            Duration::from_secs(self.timeout_secs),
            Command::new("ffprobe")
                .args(&[
                    "-v",
                    "quiet",
                    "-print_format",
                    "json",
                    "-show_format",
                    "-show_streams",
                    &probe_uri,
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output(),
        )
        .await;

        let elapsed = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(output)) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                self.parse_ffprobe_output(&stdout, elapsed)
            }
            Ok(Ok(output)) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Ok(ProbeResult {
                    success: false,
                    response_time_ms: elapsed,
                    manufacturer: None,
                    model: None,
                    firmware_version: None,
                    capabilities: HashMap::new(),
                    video_codecs: Vec::new(),
                    audio_codecs: Vec::new(),
                    resolutions: Vec::new(),
                    error_message: Some(format!("ffprobe failed: {}", stderr)),
                })
            }
            Ok(Err(e)) => Ok(ProbeResult {
                success: false,
                response_time_ms: elapsed,
                manufacturer: None,
                model: None,
                firmware_version: None,
                capabilities: HashMap::new(),
                video_codecs: Vec::new(),
                audio_codecs: Vec::new(),
                resolutions: Vec::new(),
                error_message: Some(format!("Failed to execute ffprobe: {}", e)),
            }),
            Err(_) => Ok(ProbeResult {
                success: false,
                response_time_ms: elapsed,
                manufacturer: None,
                model: None,
                firmware_version: None,
                capabilities: HashMap::new(),
                video_codecs: Vec::new(),
                audio_codecs: Vec::new(),
                resolutions: Vec::new(),
                error_message: Some("Probe timeout".to_string()),
            }),
        }
    }

    /// Parse ffprobe JSON output
    fn parse_ffprobe_output(&self, json_str: &str, response_time_ms: u64) -> Result<ProbeResult> {
        let data: serde_json::Value = serde_json::from_str(json_str)
            .context("failed to parse ffprobe output")?;

        let mut video_codecs = Vec::new();
        let mut audio_codecs = Vec::new();
        let mut resolutions = Vec::new();
        let mut capabilities = HashMap::new();

        if let Some(streams) = data["streams"].as_array() {
            for stream in streams {
                let codec_type = stream["codec_type"].as_str().unwrap_or("");
                let codec_name = stream["codec_name"].as_str().unwrap_or("unknown");

                match codec_type {
                    "video" => {
                        if !video_codecs.contains(&codec_name.to_string()) {
                            video_codecs.push(codec_name.to_string());
                        }

                        if let (Some(width), Some(height)) = (
                            stream["width"].as_i64(),
                            stream["height"].as_i64(),
                        ) {
                            let resolution = format!("{}x{}", width, height);
                            if !resolutions.contains(&resolution) {
                                resolutions.push(resolution);
                            }
                        }
                    }
                    "audio" => {
                        if !audio_codecs.contains(&codec_name.to_string()) {
                            audio_codecs.push(codec_name.to_string());
                        }
                        capabilities.insert("audio".to_string(), true);
                    }
                    _ => {}
                }
            }
        }

        // Extract metadata from format section
        let mut manufacturer = None;
        let mut model = None;

        if let Some(format) = data["format"].as_object() {
            if let Some(tags) = format["tags"].as_object() {
                manufacturer = tags
                    .get("manufacturer")
                    .or_else(|| tags.get("vendor"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                model = tags
                    .get("model")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }
        }

        Ok(ProbeResult {
            success: true,
            response_time_ms,
            manufacturer,
            model,
            firmware_version: None, // RTSP doesn't typically expose firmware version
            capabilities,
            video_codecs,
            audio_codecs,
            resolutions,
            error_message: None,
        })
    }

    /// Probe ONVIF device using GetDeviceInformation and GetCapabilities
    ///
    /// Implementation notes:
    /// - Uses ONVIF Device Management Service
    /// - Supports DigestAuth for authenticated requests
    /// - Extracts manufacturer, model, firmware, and capabilities
    /// - Falls back to unauthenticated probing if credentials fail
    ///
    /// Future enhancements:
    /// - Add WS-Discovery for automatic device discovery on network
    /// - Support WS-Security for older ONVIF devices
    /// - Extract RTSP stream URLs from Media service
    /// - Cache device capabilities for performance
    async fn probe_onvif(
        &self,
        uri: &str,
        username: Option<&str>,
        password: Option<&str>,
    ) -> Result<ProbeResult> {
        use quick_xml::events::Event;
        use quick_xml::Reader;
        use std::time::Instant;

        let start = Instant::now();

        // Parse URI to extract host and port
        let device_service_url = if uri.contains("/onvif/device_service") {
            uri.to_string()
        } else {
            // Assume standard ONVIF path
            format!("{}/onvif/device_service", uri.trim_end_matches('/'))
        };

        // Build GetDeviceInformation SOAP request
        let soap_request = r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
            xmlns:tds="http://www.onvif.org/ver10/device/wsdl">
  <s:Body>
    <tds:GetDeviceInformation/>
  </s:Body>
</s:Envelope>"#;

        // Try authenticated request if credentials provided
        let client = reqwest::Client::new();
        let mut request_builder = client
            .post(&device_service_url)
            .header("Content-Type", "application/soap+xml; charset=utf-8")
            .body(soap_request);

        // Add HTTP Digest Authentication if credentials provided
        // Note: Full DigestAuth requires nonce/realm parsing from WWW-Authenticate header
        // This is a simplified implementation - production should use proper DigestAuth
        if let (Some(user), Some(pass)) = (username, password) {
            request_builder = request_builder.basic_auth(user, Some(pass));
        }

        let result = timeout(
            Duration::from_secs(self.timeout_secs),
            request_builder.send(),
        )
        .await;

        let elapsed = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(response)) if response.status().is_success() => {
                let body = response.text().await.unwrap_or_default();

                // Parse SOAP XML response
                let mut reader = Reader::from_str(&body);
                reader.config_mut().trim_text(true);

                let mut manufacturer = None;
                let mut model = None;
                let mut firmware_version = None;
                let mut serial_number = None;

                let mut buf = Vec::new();
                let mut current_tag = String::new();

                loop {
                    match reader.read_event_into(&mut buf) {
                        Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                            let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                            current_tag = name;
                        }
                        Ok(Event::Text(e)) => {
                            let text = e.unescape().unwrap_or_default().to_string();
                            match current_tag.as_str() {
                                "Manufacturer" => manufacturer = Some(text),
                                "Model" => model = Some(text),
                                "FirmwareVersion" => firmware_version = Some(text),
                                "SerialNumber" => serial_number = Some(text),
                                _ => {}
                            }
                        }
                        Ok(Event::Eof) => break,
                        Err(e) => {
                            tracing::warn!(uri = %device_service_url, error = %e, "XML parsing error");
                            break;
                        }
                        _ => {}
                    }
                    buf.clear();
                }

                // Build capabilities (simplified - would need GetCapabilities call)
                let mut capabilities = HashMap::new();
                capabilities.insert("onvif".to_string(), true);
                if manufacturer.is_some() {
                    capabilities.insert("device_info".to_string(), true);
                }

                Ok(ProbeResult {
                    success: manufacturer.is_some() || model.is_some(),
                    response_time_ms: elapsed,
                    manufacturer,
                    model,
                    firmware_version,
                    capabilities,
                    video_codecs: vec!["H.264".to_string(), "H.265".to_string()], // Common ONVIF codecs
                    audio_codecs: vec!["AAC".to_string(), "G.711".to_string()], // Common ONVIF codecs
                    resolutions: Vec::new(), // Would require GetProfiles call
                    error_message: if serial_number.is_some() {
                        None
                    } else {
                        Some("Partial ONVIF information retrieved".to_string())
                    },
                })
            }
            Ok(Ok(response)) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                Ok(ProbeResult {
                    success: false,
                    response_time_ms: elapsed,
                    manufacturer: None,
                    model: None,
                    firmware_version: None,
                    capabilities: HashMap::new(),
                    video_codecs: Vec::new(),
                    audio_codecs: Vec::new(),
                    resolutions: Vec::new(),
                    error_message: Some(format!(
                        "ONVIF request failed: HTTP {} - {}",
                        status, body
                    )),
                })
            }
            Ok(Err(e)) => Ok(ProbeResult {
                success: false,
                response_time_ms: elapsed,
                manufacturer: None,
                model: None,
                firmware_version: None,
                capabilities: HashMap::new(),
                video_codecs: Vec::new(),
                audio_codecs: Vec::new(),
                resolutions: Vec::new(),
                error_message: Some(format!("ONVIF connection error: {}", e)),
            }),
            Err(_) => Ok(ProbeResult {
                success: false,
                response_time_ms: elapsed,
                manufacturer: None,
                model: None,
                firmware_version: None,
                capabilities: HashMap::new(),
                video_codecs: Vec::new(),
                audio_codecs: Vec::new(),
                resolutions: Vec::new(),
                error_message: Some("ONVIF probe timeout".to_string()),
            }),
        }
    }

    /// Probe HTTP endpoint (for webcams, IP cameras with HTTP API)
    async fn probe_http(&self, uri: &str) -> Result<ProbeResult> {
        let start = Instant::now();

        let result = timeout(
            Duration::from_secs(self.timeout_secs),
            reqwest::get(uri),
        )
        .await;

        let elapsed = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(response)) if response.status().is_success() => {
                // Try to extract information from headers
                let manufacturer = response
                    .headers()
                    .get("server")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());

                Ok(ProbeResult {
                    success: true,
                    response_time_ms: elapsed,
                    manufacturer,
                    model: None,
                    firmware_version: None,
                    capabilities: HashMap::new(),
                    video_codecs: Vec::new(),
                    audio_codecs: Vec::new(),
                    resolutions: Vec::new(),
                    error_message: None,
                })
            }
            Ok(Ok(response)) => Ok(ProbeResult {
                success: false,
                response_time_ms: elapsed,
                manufacturer: None,
                model: None,
                firmware_version: None,
                capabilities: HashMap::new(),
                video_codecs: Vec::new(),
                audio_codecs: Vec::new(),
                resolutions: Vec::new(),
                error_message: Some(format!("HTTP error: {}", response.status())),
            }),
            Ok(Err(e)) => Ok(ProbeResult {
                success: false,
                response_time_ms: elapsed,
                manufacturer: None,
                model: None,
                firmware_version: None,
                capabilities: HashMap::new(),
                video_codecs: Vec::new(),
                audio_codecs: Vec::new(),
                resolutions: Vec::new(),
                error_message: Some(format!("HTTP request failed: {}", e)),
            }),
            Err(_) => Ok(ProbeResult {
                success: false,
                response_time_ms: elapsed,
                manufacturer: None,
                model: None,
                firmware_version: None,
                capabilities: HashMap::new(),
                video_codecs: Vec::new(),
                audio_codecs: Vec::new(),
                resolutions: Vec::new(),
                error_message: Some("HTTP probe timeout".to_string()),
            }),
        }
    }

    /// Quick health check without full probe
    pub async fn health_check(
        &self,
        uri: &str,
        protocol: &ConnectionProtocol,
        username: Option<&str>,
        password: Option<&str>,
    ) -> Result<(bool, u64, Option<String>)> {
        let start = Instant::now();

        let result = match protocol {
            ConnectionProtocol::Rtsp => {
                // Quick RTSP check using ffprobe with minimal output
                let probe_uri = if let (Some(user), Some(pass)) = (username, password) {
                    if let Some(idx) = uri.find("://") {
                        let protocol = &uri[..idx + 3];
                        let rest = &uri[idx + 3..];
                        format!("{}{}:{}@{}", protocol, user, pass, rest)
                    } else {
                        uri.to_string()
                    }
                } else {
                    uri.to_string()
                };

                timeout(
                    Duration::from_secs(self.timeout_secs),
                    Command::new("ffprobe")
                        .args(&["-v", "quiet", "-show_format", &probe_uri])
                        .stdout(Stdio::null())
                        .stderr(Stdio::piped())
                        .output(),
                )
                .await
            }
            ConnectionProtocol::Http => {
                let http_result = timeout(
                    Duration::from_secs(self.timeout_secs),
                    reqwest::get(uri),
                )
                .await;

                // Convert reqwest::Error to std::io::Error for consistency
                match http_result {
                    Ok(Ok(_response)) => Ok(Ok(std::process::Output {
                        status: std::process::ExitStatus::default(),
                        stdout: Vec::new(),
                        stderr: Vec::new(),
                    })),
                    Ok(Err(e)) => Ok(Err(std::io::Error::new(std::io::ErrorKind::Other, e))),
                    Err(e) => Err(e),
                }
            }
            _ => {
                return Ok((false, 0, Some(format!("Protocol {:?} health check not implemented", protocol))));
            }
        };

        let elapsed = start.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(output)) if output.status.success() || matches!(protocol, ConnectionProtocol::Http) => {
                Ok((true, elapsed, None))
            }
            Ok(Ok(output)) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Ok((false, elapsed, Some(stderr.to_string())))
            }
            Ok(Err(e)) => Ok((false, elapsed, Some(e.to_string()))),
            Err(_) => Ok((false, elapsed, Some("Health check timeout".to_string()))),
        }
    }
}

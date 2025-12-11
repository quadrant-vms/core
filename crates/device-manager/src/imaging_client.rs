use crate::types::*;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, warn};

/// Trait for camera imaging and configuration operations
#[async_trait]
pub trait ImagingClient: Send + Sync {
    /// Configure camera settings
    async fn configure_camera(
        &self,
        config: &CameraConfigurationRequest,
    ) -> Result<CameraConfigurationResponse>;

    /// Get current camera configuration
    async fn get_camera_configuration(&self) -> Result<CameraConfigurationRequest>;
}

/// ONVIF Imaging client implementation
pub struct OnvifImagingClient {
    device_uri: String,
    username: Option<String>,
    password: Option<String>,
    device_id: String,
    http_client: reqwest::Client,
}

impl OnvifImagingClient {
    pub fn new(
        device_uri: String,
        username: Option<String>,
        password: Option<String>,
        device_id: String,
    ) -> Result<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;

        Ok(Self {
            device_uri,
            username,
            password,
            device_id,
            http_client,
        })
    }

    fn build_soap_envelope(&self, body: &str, namespace: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
            xmlns:timg="http://www.onvif.org/ver20/imaging/wsdl"
            xmlns:tt="http://www.onvif.org/ver10/schema"
            xmlns:trt="http://www.onvif.org/ver10/media/wsdl"
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

        debug!("sending ONVIF imaging request to {}", self.device_uri);

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

    async fn set_video_encoder_configuration(
        &self,
        config: &CameraConfigurationRequest,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let mut applied = HashMap::new();

        // Build video encoder configuration
        let mut config_parts = vec![
            r#"<trt:SetVideoEncoderConfiguration>
  <trt:Configuration token="video_encoder_config_1">
    <tt:Name>VideoEncoderConfig</tt:Name>
    <tt:UseCount>1</tt:UseCount>"#
                .to_string(),
        ];

        if let Some(codec) = &config.video_codec {
            let codec_upper = codec.to_uppercase();
            config_parts.push(format!("    <tt:Encoding>{}</tt:Encoding>", codec_upper));
            applied.insert("video_codec".to_string(), serde_json::json!(codec));
        }

        if let Some(resolution) = &config.resolution {
            let parts: Vec<&str> = resolution.split('x').collect();
            if parts.len() == 2 {
                if let (Ok(width), Ok(height)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>())
                {
                    config_parts.push(format!(
                        "    <tt:Resolution><tt:Width>{}</tt:Width><tt:Height>{}</tt:Height></tt:Resolution>",
                        width, height
                    ));
                    applied.insert("resolution".to_string(), serde_json::json!(resolution));
                }
            }
        }

        if let Some(framerate) = config.framerate {
            config_parts.push(format!(
                "    <tt:RateControl><tt:FrameRateLimit>{}</tt:FrameRateLimit>",
                framerate
            ));
            applied.insert("framerate".to_string(), serde_json::json!(framerate));

            if let Some(bitrate) = config.bitrate {
                config_parts.push(format!("<tt:BitrateLimit>{}</tt:BitrateLimit>", bitrate));
                applied.insert("bitrate".to_string(), serde_json::json!(bitrate));
            }

            config_parts.push("    </tt:RateControl>".to_string());
        }

        if let Some(gop_size) = config.gop_size {
            config_parts.push(format!(
                "    <tt:H264><tt:GovLength>{}</tt:GovLength></tt:H264>",
                gop_size
            ));
            applied.insert("gop_size".to_string(), serde_json::json!(gop_size));
        }

        if let Some(quality) = &config.quality {
            let quality_value = match quality.as_str() {
                "low" => 1,
                "medium" => 2,
                "high" => 3,
                _ => 2,
            };
            config_parts.push(format!("    <tt:Quality>{}</tt:Quality>", quality_value));
            applied.insert("quality".to_string(), serde_json::json!(quality));
        }

        config_parts.push(
            r#"  </trt:Configuration>
  <trt:ForcePersistence>true</trt:ForcePersistence>
</trt:SetVideoEncoderConfiguration>"#
                .to_string(),
        );

        let soap_body = config_parts.join("\n");

        self.send_onvif_request(&soap_body, "").await?;

        Ok(applied)
    }

    async fn set_imaging_settings(
        &self,
        config: &CameraConfigurationRequest,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let mut applied = HashMap::new();
        let mut has_imaging_settings = false;

        let mut config_parts = vec![
            r#"<timg:SetImagingSettings>
  <timg:VideoSourceToken>video_source_1</timg:VideoSourceToken>
  <timg:ImagingSettings>"#
                .to_string(),
        ];

        if let Some(brightness) = config.brightness {
            let brightness_value = brightness * 100.0; // Convert 0.0-1.0 to 0-100
            config_parts.push(format!(
                "    <tt:Brightness>{}</tt:Brightness>",
                brightness_value
            ));
            applied.insert("brightness".to_string(), serde_json::json!(brightness));
            has_imaging_settings = true;
        }

        if let Some(contrast) = config.contrast {
            let contrast_value = contrast * 100.0;
            config_parts.push(format!("    <tt:Contrast>{}</tt:Contrast>", contrast_value));
            applied.insert("contrast".to_string(), serde_json::json!(contrast));
            has_imaging_settings = true;
        }

        if let Some(saturation) = config.saturation {
            let saturation_value = saturation * 100.0;
            config_parts.push(format!(
                "    <tt:ColorSaturation>{}</tt:ColorSaturation>",
                saturation_value
            ));
            applied.insert("saturation".to_string(), serde_json::json!(saturation));
            has_imaging_settings = true;
        }

        if let Some(sharpness) = config.sharpness {
            let sharpness_value = sharpness * 100.0;
            config_parts.push(format!(
                "    <tt:Sharpness>{}</tt:Sharpness>",
                sharpness_value
            ));
            applied.insert("sharpness".to_string(), serde_json::json!(sharpness));
            has_imaging_settings = true;
        }

        if let Some(ir_mode) = &config.ir_mode {
            let ir_mode_upper = match ir_mode.as_str() {
                "on" => "ON",
                "off" => "OFF",
                _ => "AUTO",
            };
            config_parts.push(format!(
                "    <tt:IrCutFilter>{}</tt:IrCutFilter>",
                ir_mode_upper
            ));
            applied.insert("ir_mode".to_string(), serde_json::json!(ir_mode));
            has_imaging_settings = true;
        }

        if let Some(wdr_enabled) = config.wdr_enabled {
            let wdr_mode = if wdr_enabled { "ON" } else { "OFF" };
            config_parts.push(format!(
                "    <tt:WideDynamicRange><tt:Mode>{}</tt:Mode></tt:WideDynamicRange>",
                wdr_mode
            ));
            applied.insert("wdr_enabled".to_string(), serde_json::json!(wdr_enabled));
            has_imaging_settings = true;
        }

        config_parts.push(
            r#"  </timg:ImagingSettings>
  <timg:ForcePersistence>true</timg:ForcePersistence>
</timg:SetImagingSettings>"#
                .to_string(),
        );

        if !has_imaging_settings {
            return Ok(applied);
        }

        let soap_body = config_parts.join("\n");

        self.send_onvif_request(&soap_body, "").await?;

        Ok(applied)
    }
}

#[async_trait]
impl ImagingClient for OnvifImagingClient {
    async fn configure_camera(
        &self,
        config: &CameraConfigurationRequest,
    ) -> Result<CameraConfigurationResponse> {
        let config_id = uuid::Uuid::new_v4().to_string();
        let mut all_applied = HashMap::new();
        let mut failed_settings: HashMap<String, String> = HashMap::new();

        // Try to set video encoder configuration
        if config.video_codec.is_some()
            || config.resolution.is_some()
            || config.framerate.is_some()
            || config.bitrate.is_some()
            || config.gop_size.is_some()
            || config.quality.is_some()
        {
            match self.set_video_encoder_configuration(config).await {
                Ok(applied) => {
                    all_applied.extend(applied);
                }
                Err(e) => {
                    warn!("failed to set video encoder configuration: {}", e);
                    failed_settings.insert(
                        "video_encoder".to_string(),
                        format!("Failed to set video encoder configuration: {}", e),
                    );
                }
            }
        }

        // Try to set imaging settings
        if config.brightness.is_some()
            || config.contrast.is_some()
            || config.saturation.is_some()
            || config.sharpness.is_some()
            || config.ir_mode.is_some()
            || config.wdr_enabled.is_some()
        {
            match self.set_imaging_settings(config).await {
                Ok(applied) => {
                    all_applied.extend(applied);
                }
                Err(e) => {
                    warn!("failed to set imaging settings: {}", e);
                    failed_settings.insert(
                        "imaging_settings".to_string(),
                        format!("Failed to set imaging settings: {}", e),
                    );
                }
            }
        }

        // Determine overall status
        let status = if all_applied.is_empty() && !failed_settings.is_empty() {
            ConfigurationStatus::Failed
        } else if !failed_settings.is_empty() {
            ConfigurationStatus::PartiallyApplied
        } else {
            ConfigurationStatus::Applied
        };

        let error_message = if !failed_settings.is_empty() {
            Some(format!("Some settings failed: {:?}", failed_settings))
        } else {
            None
        };

        Ok(CameraConfigurationResponse {
            config_id,
            device_id: self.device_id.clone(),
            status,
            applied_settings: all_applied,
            failed_settings: if failed_settings.is_empty() {
                None
            } else {
                Some(failed_settings)
            },
            error_message,
            applied_at: Some(chrono::Utc::now()),
        })
    }

    async fn get_camera_configuration(&self) -> Result<CameraConfigurationRequest> {
        // This would query current configuration from the device
        // For now, return empty configuration
        warn!("get_camera_configuration not fully implemented");
        Ok(CameraConfigurationRequest {
            video_codec: None,
            resolution: None,
            framerate: None,
            bitrate: None,
            gop_size: None,
            quality: None,
            brightness: None,
            contrast: None,
            saturation: None,
            sharpness: None,
            hue: None,
            audio_enabled: None,
            audio_codec: None,
            audio_bitrate: None,
            multicast_enabled: None,
            multicast_address: None,
            rtsp_port: None,
            ir_mode: None,
            wdr_enabled: None,
            metadata: None,
        })
    }
}

/// Mock imaging client for testing
pub struct MockImagingClient {
    device_id: String,
}

impl MockImagingClient {
    pub fn new(device_id: String) -> Self {
        Self { device_id }
    }
}

#[async_trait]
impl ImagingClient for MockImagingClient {
    async fn configure_camera(
        &self,
        config: &CameraConfigurationRequest,
    ) -> Result<CameraConfigurationResponse> {
        debug!("mock: configure camera");

        let mut applied_settings = HashMap::new();

        if let Some(codec) = &config.video_codec {
            applied_settings.insert("video_codec".to_string(), serde_json::json!(codec));
        }
        if let Some(resolution) = &config.resolution {
            applied_settings.insert("resolution".to_string(), serde_json::json!(resolution));
        }
        if let Some(framerate) = config.framerate {
            applied_settings.insert("framerate".to_string(), serde_json::json!(framerate));
        }
        if let Some(brightness) = config.brightness {
            applied_settings.insert("brightness".to_string(), serde_json::json!(brightness));
        }

        Ok(CameraConfigurationResponse {
            config_id: uuid::Uuid::new_v4().to_string(),
            device_id: self.device_id.clone(),
            status: ConfigurationStatus::Applied,
            applied_settings,
            failed_settings: None,
            error_message: None,
            applied_at: Some(chrono::Utc::now()),
        })
    }

    async fn get_camera_configuration(&self) -> Result<CameraConfigurationRequest> {
        debug!("mock: get camera configuration");
        Ok(CameraConfigurationRequest {
            video_codec: Some("h264".to_string()),
            resolution: Some("1920x1080".to_string()),
            framerate: Some(30),
            bitrate: Some(4096),
            gop_size: Some(30),
            quality: Some("high".to_string()),
            brightness: Some(0.5),
            contrast: Some(0.5),
            saturation: Some(0.5),
            sharpness: Some(0.5),
            hue: Some(0.5),
            audio_enabled: Some(true),
            audio_codec: Some("aac".to_string()),
            audio_bitrate: Some(128),
            multicast_enabled: Some(false),
            multicast_address: None,
            rtsp_port: Some(554),
            ir_mode: Some("auto".to_string()),
            wdr_enabled: Some(false),
            metadata: None,
        })
    }
}

/// Factory for creating imaging clients based on device protocol
pub fn create_imaging_client(
    protocol: &ConnectionProtocol,
    device_uri: &str,
    username: Option<String>,
    password: Option<String>,
    device_id: &str,
) -> Result<Arc<dyn ImagingClient>> {
    match protocol {
        ConnectionProtocol::Onvif => {
            let client = OnvifImagingClient::new(
                device_uri.to_string(),
                username,
                password,
                device_id.to_string(),
            )?;
            Ok(Arc::new(client))
        }
        _ => {
            // For non-ONVIF protocols, use mock client
            warn!(
                "Camera configuration not natively supported for protocol {:?}, using mock client",
                protocol
            );
            Ok(Arc::new(MockImagingClient::new(device_id.to_string())))
        }
    }
}

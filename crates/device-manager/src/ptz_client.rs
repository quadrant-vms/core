use crate::types::*;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, warn};

/// Trait for PTZ control operations
#[async_trait]
pub trait PtzClient: Send + Sync {
    /// Move camera in specified direction
    async fn move_camera(&self, request: &PtzMoveRequest) -> Result<()>;

    /// Stop camera movement
    async fn stop(&self, request: &PtzStopRequest) -> Result<()>;

    /// Zoom camera
    async fn zoom(&self, request: &PtzZoomRequest) -> Result<()>;

    /// Move to absolute position
    async fn goto_absolute_position(&self, request: &PtzAbsolutePositionRequest) -> Result<()>;

    /// Move by relative offset
    async fn goto_relative_position(&self, request: &PtzRelativePositionRequest) -> Result<()>;

    /// Set focus mode and value
    async fn set_focus(&self, request: &PtzFocusRequest) -> Result<()>;

    /// Set iris value
    async fn set_iris(&self, request: &PtzIrisRequest) -> Result<()>;

    /// Go to home position
    async fn goto_home(&self) -> Result<()>;

    /// Get current PTZ status
    async fn get_status(&self) -> Result<PtzStatus>;

    /// Get PTZ capabilities
    async fn get_capabilities(&self) -> Result<PtzCapabilities>;
}

/// ONVIF PTZ client implementation
pub struct OnvifPtzClient {
    device_uri: String,
    username: Option<String>,
    password: Option<String>,
    http_client: reqwest::Client,
}

impl OnvifPtzClient {
    pub fn new(
        device_uri: String,
        username: Option<String>,
        password: Option<String>,
    ) -> Result<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;

        Ok(Self {
            device_uri,
            username,
            password,
            http_client,
        })
    }

    fn build_soap_envelope(&self, body: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
            xmlns:tptz="http://www.onvif.org/ver20/ptz/wsdl">
  <s:Body>
    {}
  </s:Body>
</s:Envelope>"#,
            body
        )
    }

    async fn send_onvif_request(&self, soap_body: &str) -> Result<String> {
        let envelope = self.build_soap_envelope(soap_body);

        debug!("sending ONVIF PTZ request to {}", self.device_uri);

        let mut request = self.http_client
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
impl PtzClient for OnvifPtzClient {
    async fn move_camera(&self, request: &PtzMoveRequest) -> Result<()> {
        let (pan_speed, tilt_speed) = match request.direction {
            PtzDirection::Up => (0.0, request.speed),
            PtzDirection::Down => (0.0, -request.speed),
            PtzDirection::Left => (-request.speed, 0.0),
            PtzDirection::Right => (request.speed, 0.0),
            PtzDirection::UpLeft => (-request.speed, request.speed),
            PtzDirection::UpRight => (request.speed, request.speed),
            PtzDirection::DownLeft => (-request.speed, -request.speed),
            PtzDirection::DownRight => (request.speed, -request.speed),
        };

        let soap_body = format!(
            r#"<tptz:ContinuousMove>
  <tptz:ProfileToken>profile_1</tptz:ProfileToken>
  <tptz:Velocity>
    <tt:PanTilt x="{}" y="{}" xmlns:tt="http://www.onvif.org/ver10/schema"/>
    <tt:Zoom x="0" xmlns:tt="http://www.onvif.org/ver10/schema"/>
  </tptz:Velocity>
</tptz:ContinuousMove>"#,
            pan_speed, tilt_speed
        );

        self.send_onvif_request(&soap_body).await?;

        // If duration is specified, schedule a stop
        if let Some(duration_ms) = request.duration_ms {
            let client = Arc::new(self.clone());
            tokio::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(duration_ms)).await;
                let _ = client.stop(&PtzStopRequest {
                    stop_pan_tilt: true,
                    stop_zoom: false,
                }).await;
            });
        }

        Ok(())
    }

    async fn stop(&self, request: &PtzStopRequest) -> Result<()> {
        let soap_body = format!(
            r#"<tptz:Stop>
  <tptz:ProfileToken>profile_1</tptz:ProfileToken>
  <tptz:PanTilt>{}</tptz:PanTilt>
  <tptz:Zoom>{}</tptz:Zoom>
</tptz:Stop>"#,
            request.stop_pan_tilt, request.stop_zoom
        );

        self.send_onvif_request(&soap_body).await?;
        Ok(())
    }

    async fn zoom(&self, request: &PtzZoomRequest) -> Result<()> {
        let zoom_speed = match request.direction {
            PtzZoomDirection::In => request.speed,
            PtzZoomDirection::Out => -request.speed,
        };

        let soap_body = format!(
            r#"<tptz:ContinuousMove>
  <tptz:ProfileToken>profile_1</tptz:ProfileToken>
  <tptz:Velocity>
    <tt:PanTilt x="0" y="0" xmlns:tt="http://www.onvif.org/ver10/schema"/>
    <tt:Zoom x="{}" xmlns:tt="http://www.onvif.org/ver10/schema"/>
  </tptz:Velocity>
</tptz:ContinuousMove>"#,
            zoom_speed
        );

        self.send_onvif_request(&soap_body).await?;

        // If duration is specified, schedule a stop
        if let Some(duration_ms) = request.duration_ms {
            let client = Arc::new(self.clone());
            tokio::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(duration_ms)).await;
                let _ = client.stop(&PtzStopRequest {
                    stop_pan_tilt: false,
                    stop_zoom: true,
                }).await;
            });
        }

        Ok(())
    }

    async fn goto_absolute_position(&self, request: &PtzAbsolutePositionRequest) -> Result<()> {
        let speed = request.speed.unwrap_or(0.5);

        let soap_body = format!(
            r#"<tptz:AbsoluteMove>
  <tptz:ProfileToken>profile_1</tptz:ProfileToken>
  <tptz:Position>
    <tt:PanTilt x="{}" y="{}" xmlns:tt="http://www.onvif.org/ver10/schema"/>
    <tt:Zoom x="{}" xmlns:tt="http://www.onvif.org/ver10/schema"/>
  </tptz:Position>
  <tptz:Speed>
    <tt:PanTilt x="{}" y="{}" xmlns:tt="http://www.onvif.org/ver10/schema"/>
    <tt:Zoom x="{}" xmlns:tt="http://www.onvif.org/ver10/schema"/>
  </tptz:Speed>
</tptz:AbsoluteMove>"#,
            request.pan, request.tilt, request.zoom, speed, speed, speed
        );

        self.send_onvif_request(&soap_body).await?;
        Ok(())
    }

    async fn goto_relative_position(&self, request: &PtzRelativePositionRequest) -> Result<()> {
        let speed = request.speed.unwrap_or(0.5);

        let soap_body = format!(
            r#"<tptz:RelativeMove>
  <tptz:ProfileToken>profile_1</tptz:ProfileToken>
  <tptz:Translation>
    <tt:PanTilt x="{}" y="{}" xmlns:tt="http://www.onvif.org/ver10/schema"/>
    <tt:Zoom x="{}" xmlns:tt="http://www.onvif.org/ver10/schema"/>
  </tptz:Translation>
  <tptz:Speed>
    <tt:PanTilt x="{}" y="{}" xmlns:tt="http://www.onvif.org/ver10/schema"/>
    <tt:Zoom x="{}" xmlns:tt="http://www.onvif.org/ver10/schema"/>
  </tptz:Speed>
</tptz:RelativeMove>"#,
            request.pan, request.tilt, request.zoom, speed, speed, speed
        );

        self.send_onvif_request(&soap_body).await?;
        Ok(())
    }

    async fn set_focus(&self, _request: &PtzFocusRequest) -> Result<()> {
        // ONVIF focus control requires imaging service
        // This is a simplified implementation
        warn!("focus control not fully implemented for ONVIF");
        Ok(())
    }

    async fn set_iris(&self, _request: &PtzIrisRequest) -> Result<()> {
        // ONVIF iris control requires imaging service
        // This is a simplified implementation
        warn!("iris control not fully implemented for ONVIF");
        Ok(())
    }

    async fn goto_home(&self) -> Result<()> {
        let soap_body = r#"<tptz:GotoHomePosition>
  <tptz:ProfileToken>profile_1</tptz:ProfileToken>
</tptz:GotoHomePosition>"#;

        self.send_onvif_request(soap_body).await?;
        Ok(())
    }

    async fn get_status(&self) -> Result<PtzStatus> {
        let soap_body = r#"<tptz:GetStatus>
  <tptz:ProfileToken>profile_1</tptz:ProfileToken>
</tptz:GetStatus>"#;

        let _response = self.send_onvif_request(soap_body).await?;

        // Parse SOAP response (simplified - real implementation would use proper XML parsing)
        // For now, return a placeholder
        warn!("PTZ status parsing not fully implemented");

        Ok(PtzStatus {
            device_id: "unknown".to_string(),
            position: PtzPosition {
                pan: 0.0,
                tilt: 0.0,
                zoom: 0.0,
            },
            is_moving: false,
            last_updated: chrono::Utc::now(),
        })
    }

    async fn get_capabilities(&self) -> Result<PtzCapabilities> {
        // Query ONVIF capabilities
        // For now, return default capabilities
        Ok(PtzCapabilities {
            pan_tilt: true,
            zoom: true,
            focus: true,
            iris: true,
            presets: true,
            tours: false, // ONVIF tours are different from our custom tours
            absolute_movement: true,
            relative_movement: true,
            continuous_movement: true,
            home_position: true,
            pan_range: Some((-180.0, 180.0)),
            tilt_range: Some((-90.0, 90.0)),
            zoom_range: Some((0.0, 1.0)),
            max_presets: Some(128),
        })
    }
}

// Clone implementation for OnvifPtzClient (needed for spawning stop tasks)
impl Clone for OnvifPtzClient {
    fn clone(&self) -> Self {
        Self {
            device_uri: self.device_uri.clone(),
            username: self.username.clone(),
            password: self.password.clone(),
            http_client: self.http_client.clone(),
        }
    }
}

/// Mock PTZ client for testing
pub struct MockPtzClient;

impl MockPtzClient {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl PtzClient for MockPtzClient {
    async fn move_camera(&self, _request: &PtzMoveRequest) -> Result<()> {
        debug!("mock: move camera");
        Ok(())
    }

    async fn stop(&self, _request: &PtzStopRequest) -> Result<()> {
        debug!("mock: stop");
        Ok(())
    }

    async fn zoom(&self, _request: &PtzZoomRequest) -> Result<()> {
        debug!("mock: zoom");
        Ok(())
    }

    async fn goto_absolute_position(&self, _request: &PtzAbsolutePositionRequest) -> Result<()> {
        debug!("mock: goto absolute position");
        Ok(())
    }

    async fn goto_relative_position(&self, _request: &PtzRelativePositionRequest) -> Result<()> {
        debug!("mock: goto relative position");
        Ok(())
    }

    async fn set_focus(&self, _request: &PtzFocusRequest) -> Result<()> {
        debug!("mock: set focus");
        Ok(())
    }

    async fn set_iris(&self, _request: &PtzIrisRequest) -> Result<()> {
        debug!("mock: set iris");
        Ok(())
    }

    async fn goto_home(&self) -> Result<()> {
        debug!("mock: goto home");
        Ok(())
    }

    async fn get_status(&self) -> Result<PtzStatus> {
        Ok(PtzStatus {
            device_id: "mock".to_string(),
            position: PtzPosition {
                pan: 0.0,
                tilt: 0.0,
                zoom: 0.5,
            },
            is_moving: false,
            last_updated: chrono::Utc::now(),
        })
    }

    async fn get_capabilities(&self) -> Result<PtzCapabilities> {
        Ok(PtzCapabilities {
            pan_tilt: true,
            zoom: true,
            focus: true,
            iris: true,
            presets: true,
            tours: true,
            absolute_movement: true,
            relative_movement: true,
            continuous_movement: true,
            home_position: true,
            pan_range: Some((-180.0, 180.0)),
            tilt_range: Some((-90.0, 90.0)),
            zoom_range: Some((0.0, 1.0)),
            max_presets: Some(256),
        })
    }
}

/// Factory for creating PTZ clients based on device protocol
pub fn create_ptz_client(
    protocol: &ConnectionProtocol,
    device_uri: &str,
    username: Option<String>,
    password: Option<String>,
) -> Result<Arc<dyn PtzClient>> {
    match protocol {
        ConnectionProtocol::Onvif => {
            let client = OnvifPtzClient::new(device_uri.to_string(), username, password)?;
            Ok(Arc::new(client))
        }
        _ => {
            // For non-ONVIF protocols, use mock client
            warn!("PTZ not natively supported for protocol {:?}, using mock client", protocol);
            Ok(Arc::new(MockPtzClient::new()))
        }
    }
}

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use interceptor::registry::Registry;

use super::peer::WebRtcPeerManager;

/// WHEP (WebRTC-HTTP Egress Protocol) handler
///
/// WHEP is a simple protocol for WebRTC playback that uses HTTP for signaling.
/// Client workflow:
/// 1. Client sends SDP offer via POST to /whep/{resource_id}
/// 2. Server responds with SDP answer and Location header (session URL)
/// 3. Client uses the session URL for ICE candidate exchange (PATCH requests)
/// 4. Client deletes session via DELETE to session URL
pub struct WhepHandler {
    peer_manager: Arc<WebRtcPeerManager>,
    ice_servers: Vec<RTCIceServer>,
}

/// WHEP offer request (client → server)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhepOffer {
    /// SDP offer from client
    pub sdp: String,
    /// Optional preferred codec
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codec: Option<String>,
}

/// WHEP answer response (server → client)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhepAnswer {
    /// SDP answer from server
    pub sdp: String,
    /// Session ID for subsequent operations
    pub session_id: String,
    /// Session URL for PATCH/DELETE operations
    pub session_url: String,
}

impl WhepHandler {
    pub fn new(peer_manager: Arc<WebRtcPeerManager>) -> Self {
        // Default STUN servers
        let ice_servers = vec![
            RTCIceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_owned()],
                ..Default::default()
            },
        ];

        Self {
            peer_manager,
            ice_servers,
        }
    }

    /// Handle WHEP offer and create peer connection
    pub async fn handle_offer(
        &self,
        resource_id: &str,
        offer: WhepOffer,
        base_url: &str,
    ) -> Result<WhepAnswer> {
        info!(resource_id = %resource_id, "handling WHEP offer");

        // Generate session ID
        let session_id = Uuid::new_v4().to_string();

        // Create WebRTC peer connection
        let peer = self.create_peer_connection().await?;

        // Set remote description (client's offer)
        let offer_sdp = RTCSessionDescription::offer(offer.sdp)?;
        peer.set_remote_description(offer_sdp).await?;

        // Create answer
        let answer = peer.create_answer(None).await?;

        // Set local description
        peer.set_local_description(answer.clone()).await?;

        // Store peer connection
        self.peer_manager.add_peer(&session_id, peer, resource_id).await?;

        // Build session URL
        let session_url = format!("{}/whep/session/{}", base_url, session_id);

        info!(session_id = %session_id, resource_id = %resource_id, "WHEP session created");

        Ok(WhepAnswer {
            sdp: answer.sdp,
            session_id,
            session_url,
        })
    }

    /// Handle session deletion
    pub async fn delete_session(&self, session_id: &str) -> Result<()> {
        info!(session_id = %session_id, "deleting WHEP session");
        self.peer_manager.remove_peer(session_id).await
    }

    /// Create a new WebRTC peer connection
    async fn create_peer_connection(&self) -> Result<Arc<RTCPeerConnection>> {
        // Create a MediaEngine
        let mut media_engine = MediaEngine::default();

        // Register codecs
        media_engine.register_default_codecs()?;

        // Create the API with the MediaEngine
        let mut registry = Registry::new();
        registry = register_default_interceptors(registry, &mut media_engine)?;

        let api = APIBuilder::new()
            .with_media_engine(media_engine)
            .with_interceptor_registry(registry)
            .build();

        // Create RTCConfiguration
        let config = RTCConfiguration {
            ice_servers: self.ice_servers.clone(),
            ..Default::default()
        };

        // Create peer connection
        let peer = Arc::new(api.new_peer_connection(config).await?);

        Ok(peer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_whep_handler_creation() {
        let peer_manager = Arc::new(WebRtcPeerManager::new());
        let handler = WhepHandler::new(peer_manager);
        assert_eq!(handler.ice_servers.len(), 1);
    }
}

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_sender::RTCRtpSender;
use webrtc::track::track_local::track_local_static_rtp::TrackLocalStaticRTP;
use webrtc::track::track_local::TrackLocal;

/// WebRTC peer connection data
struct PeerData {
    connection: Arc<RTCPeerConnection>,
    resource_id: String,
    video_track: Option<Arc<TrackLocalStaticRTP>>,
    audio_track: Option<Arc<TrackLocalStaticRTP>>,
}

/// Manages WebRTC peer connections
pub struct WebRtcPeerManager {
    peers: Arc<RwLock<HashMap<String, PeerData>>>,
}

impl WebRtcPeerManager {
    pub fn new() -> Self {
        Self {
            peers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a new peer connection
    pub async fn add_peer(
        &self,
        session_id: &str,
        peer: Arc<RTCPeerConnection>,
        resource_id: &str,
    ) -> Result<()> {
        info!(session_id = %session_id, resource_id = %resource_id, "adding WebRTC peer");

        // Create video track (H.264)
        let video_track = Arc::new(TrackLocalStaticRTP::new(
            webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability {
                mime_type: "video/H264".to_owned(),
                clock_rate: 90000,
                channels: 0,
                sdp_fmtp_line: "".to_owned(),
                rtcp_feedback: vec![],
            },
            format!("video-{}", session_id),
            format!("webrtc-video-{}", session_id),
        ));

        // Create audio track (Opus)
        let audio_track = Arc::new(TrackLocalStaticRTP::new(
            webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability {
                mime_type: "audio/opus".to_owned(),
                clock_rate: 48000,
                channels: 2,
                sdp_fmtp_line: "".to_owned(),
                rtcp_feedback: vec![],
            },
            format!("audio-{}", session_id),
            format!("webrtc-audio-{}", session_id),
        ));

        // Add tracks to peer connection
        let rtp_sender_video = peer
            .add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>)
            .await?;

        let rtp_sender_audio = peer
            .add_track(Arc::clone(&audio_track) as Arc<dyn TrackLocal + Send + Sync>)
            .await?;

        // Read RTCP packets (required for WebRTC to work properly)
        tokio::spawn(async move {
            let mut rtcp_buf_video = vec![0u8; 1500];
            let mut rtcp_buf_audio = vec![0u8; 1500];
            loop {
                tokio::select! {
                    result = rtp_sender_video.read(&mut rtcp_buf_video) => {
                        if let Err(e) = result {
                            warn!("video RTCP read error: {}", e);
                            break;
                        }
                    }
                    result = rtp_sender_audio.read(&mut rtcp_buf_audio) => {
                        if let Err(e) = result {
                            warn!("audio RTCP read error: {}", e);
                            break;
                        }
                    }
                }
            }
        });

        // Store peer data
        let peer_data = PeerData {
            connection: peer,
            resource_id: resource_id.to_string(),
            video_track: Some(video_track),
            audio_track: Some(audio_track),
        };

        let mut peers = self.peers.write().await;
        peers.insert(session_id.to_string(), peer_data);

        info!(session_id = %session_id, "WebRTC peer added successfully");
        Ok(())
    }

    /// Remove a peer connection
    pub async fn remove_peer(&self, session_id: &str) -> Result<()> {
        info!(session_id = %session_id, "removing WebRTC peer");

        let mut peers = self.peers.write().await;
        if let Some(peer_data) = peers.remove(session_id) {
            // Close the peer connection
            if let Err(e) = peer_data.connection.close().await {
                error!(session_id = %session_id, error = %e, "failed to close peer connection");
            }
            info!(session_id = %session_id, "WebRTC peer removed");
            Ok(())
        } else {
            Err(anyhow!("Peer not found: {}", session_id))
        }
    }

    /// Get peer connection
    pub async fn get_peer(&self, session_id: &str) -> Option<Arc<RTCPeerConnection>> {
        let peers = self.peers.read().await;
        peers.get(session_id).map(|p| p.connection.clone())
    }

    /// Get video track for a session
    pub async fn get_video_track(&self, session_id: &str) -> Option<Arc<TrackLocalStaticRTP>> {
        let peers = self.peers.read().await;
        peers.get(session_id).and_then(|p| p.video_track.clone())
    }

    /// Get audio track for a session
    pub async fn get_audio_track(&self, session_id: &str) -> Option<Arc<TrackLocalStaticRTP>> {
        let peers = self.peers.read().await;
        peers.get(session_id).and_then(|p| p.audio_track.clone())
    }

    /// List all active sessions
    pub async fn list_sessions(&self) -> Vec<String> {
        let peers = self.peers.read().await;
        peers.keys().cloned().collect()
    }

    /// Get session count
    pub async fn session_count(&self) -> usize {
        let peers = self.peers.read().await;
        peers.len()
    }
}

impl Default for WebRtcPeerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_peer_manager() {
        let manager = WebRtcPeerManager::new();
        assert_eq!(manager.session_count().await, 0);
    }
}

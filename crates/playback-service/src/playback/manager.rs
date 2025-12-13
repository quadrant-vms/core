use anyhow::{anyhow, Result};
use common::playback::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use super::dvr::DvrBufferManager;
use super::ll_hls::{BlockingParams, HlsVariant, LlHlsConfig, LlHlsPlaylistGenerator};
use super::store::PlaybackStore;

/// In-memory playback session data
struct SessionData {
    info: PlaybackInfo,
    cancel_token: CancellationToken,
    /// DVR buffer manager (only for DVR-enabled sessions)
    dvr_manager: Option<Arc<DvrBufferManager>>,
}

/// Playback session manager
pub struct PlaybackManager {
    sessions: Arc<RwLock<HashMap<String, SessionData>>>,
    store: Option<Arc<PlaybackStore>>,
    node_id: String,
    hls_base_url: String,
    rtsp_base_url: String,
    recording_storage_root: PathBuf,
    stream_hls_root: PathBuf,
    ll_hls_generator: Arc<LlHlsPlaylistGenerator>,
}

impl PlaybackManager {
    pub fn new(
        store: Option<Arc<PlaybackStore>>,
        node_id: String,
        hls_base_url: String,
        rtsp_base_url: String,
    ) -> Self {
        let recording_storage_root = std::env::var("RECORDING_STORAGE_ROOT")
            .unwrap_or_else(|_| "./data/recordings".to_string())
            .into();

        let stream_hls_root: PathBuf = std::env::var("HLS_ROOT")
            .unwrap_or_else(|_| "./data/hls".to_string())
            .into();

        // Initialize LL-HLS generator with default config
        let ll_hls_config = LlHlsConfig::default();
        let ll_hls_generator = Arc::new(LlHlsPlaylistGenerator::new(
            ll_hls_config,
            stream_hls_root.clone(),
        ));

        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            store,
            node_id,
            hls_base_url,
            rtsp_base_url,
            recording_storage_root,
            stream_hls_root,
            ll_hls_generator,
        }
    }

    /// Start a new playback session
    pub async fn start(&self, config: PlaybackConfig) -> Result<PlaybackInfo> {
        info!(session_id = %config.session_id, source = %config.source_id, "starting playback session");

        // Validate source exists
        self.validate_source(&config).await?;

        // Generate playback URL based on protocol
        let playback_url = self.generate_playback_url(&config)?;

        // Create session info
        let mut info = PlaybackInfo {
            config: config.clone(),
            state: PlaybackState::Starting,
            lease_id: None,
            last_error: None,
            node_id: Some(self.node_id.clone()),
            playback_url: Some(playback_url),
            current_position_secs: config.start_time_secs,
            duration_secs: None,
            started_at: Some(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs()),
            stopped_at: None,
            dvr_window: None, // Will be set later if DVR is enabled
        };

        // For recordings, get duration
        if config.source_type == PlaybackSourceType::Recording {
            if let Ok(duration) = self.get_recording_duration(&config.source_id).await {
                info.duration_secs = Some(duration);
            }
        }

        // Save to database if enabled
        if let Some(store) = &self.store {
            store.save(&info).await?;
        }

        // Update state to playing
        info.state = PlaybackState::Playing;
        if let Some(store) = &self.store {
            store.save(&info).await?;
        }

        // Create DVR manager if DVR is enabled
        let dvr_manager = if let Some(ref dvr_cfg) = config.dvr {
            if dvr_cfg.enabled && config.source_type == PlaybackSourceType::Stream {
                let hls_path = self.stream_hls_root.join(&config.source_id);
                let manager = Arc::new(DvrBufferManager::new(
                    config.source_id.clone(),
                    hls_path,
                    dvr_cfg.buffer_window_secs,
                ));

                // Initial segment scan
                if let Err(e) = manager.scan_segments().await {
                    warn!(session_id = %config.session_id, error = %e, "Failed to scan DVR segments");
                }

                // Update DVR window info
                if let Ok(window) = manager.get_window(None).await {
                    info.dvr_window = Some(window);
                }

                Some(manager)
            } else {
                None
            }
        } else {
            None
        };

        // Store in memory
        let cancel_token = CancellationToken::new();
        let mut sessions = self.sessions.write().await;
        sessions.insert(
            config.session_id.clone(),
            SessionData {
                info: info.clone(),
                cancel_token,
                dvr_manager,
            },
        );

        info!(session_id = %config.session_id, url = %info.playback_url.as_ref().unwrap(), "playback session started");
        Ok(info)
    }

    /// Stop a playback session
    pub async fn stop(&self, session_id: &str) -> Result<bool> {
        info!(session_id = %session_id, "stopping playback session");

        let mut sessions = self.sessions.write().await;
        if let Some(session_data) = sessions.remove(session_id) {
            // Cancel any background tasks
            session_data.cancel_token.cancel();

            // Update state
            let mut info = session_data.info;
            info.state = PlaybackState::Stopped;
            info.stopped_at = Some(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs());

            if let Some(store) = &self.store {
                store.save(&info).await?;
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Seek to a position (for recordings only)
    pub async fn seek(&self, session_id: &str, position_secs: f64) -> Result<f64> {
        info!(session_id = %session_id, position = %position_secs, "seeking playback");

        let mut sessions = self.sessions.write().await;
        if let Some(session_data) = sessions.get_mut(session_id) {
            // Only allow seeking for recordings
            if session_data.info.config.source_type != PlaybackSourceType::Recording {
                return Err(anyhow!("Seeking is only supported for recordings"));
            }

            // Validate position
            if let Some(duration) = session_data.info.duration_secs {
                if position_secs < 0.0 || position_secs > duration {
                    return Err(anyhow!("Invalid seek position: {} (duration: {})", position_secs, duration));
                }
            }

            // Update position
            session_data.info.current_position_secs = Some(position_secs);
            session_data.info.state = PlaybackState::Playing;

            if let Some(store) = &self.store {
                store.save(&session_data.info).await?;
            }

            Ok(position_secs)
        } else {
            Err(anyhow!("Session not found: {}", session_id))
        }
    }

    /// Pause a playback session
    pub async fn pause(&self, session_id: &str) -> Result<()> {
        self.update_state(session_id, PlaybackState::Paused).await
    }

    /// Resume a playback session
    pub async fn resume(&self, session_id: &str) -> Result<()> {
        self.update_state(session_id, PlaybackState::Playing).await
    }

    /// List all active sessions
    pub async fn list(&self) -> Vec<PlaybackInfo> {
        let sessions = self.sessions.read().await;
        sessions.values().map(|s| s.info.clone()).collect()
    }

    /// Get a specific session
    pub async fn get(&self, session_id: &str) -> Option<PlaybackInfo> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).map(|s| s.info.clone())
    }

    // Helper methods

    async fn validate_source(&self, config: &PlaybackConfig) -> Result<()> {
        match config.source_type {
            PlaybackSourceType::Stream => {
                // Check if stream HLS files exist
                let stream_dir = self.stream_hls_root.join(&config.source_id);
                let playlist = stream_dir.join("index.m3u8");
                if !playlist.exists() {
                    return Err(anyhow!("Stream not found or not active: {}", config.source_id));
                }
            }
            PlaybackSourceType::Recording => {
                // Check if recording file exists
                let recording_path = self.find_recording_path(&config.source_id)?;
                if !recording_path.exists() {
                    return Err(anyhow!("Recording not found: {}", config.source_id));
                }
            }
        }
        Ok(())
    }

    fn generate_playback_url(&self, config: &PlaybackConfig) -> Result<String> {
        match config.protocol {
            PlaybackProtocol::Hls => {
                match config.source_type {
                    PlaybackSourceType::Stream => {
                        // Live stream HLS
                        Ok(format!("{}/streams/{}/index.m3u8", self.hls_base_url, config.source_id))
                    }
                    PlaybackSourceType::Recording => {
                        // Recording HLS (if recording format is HLS) or generated on-the-fly
                        Ok(format!("{}/recordings/{}/index.m3u8", self.hls_base_url, config.source_id))
                    }
                }
            }
            PlaybackProtocol::Rtsp => {
                match config.source_type {
                    PlaybackSourceType::Stream => {
                        // RTSP proxy for live stream
                        Ok(format!("{}/streams/{}", self.rtsp_base_url, config.source_id))
                    }
                    PlaybackSourceType::Recording => {
                        // RTSP for recording playback
                        Ok(format!("{}/recordings/{}", self.rtsp_base_url, config.source_id))
                    }
                }
            }
            PlaybackProtocol::WebRtc => {
                // WebRTC uses WHEP protocol - client will POST to /whep endpoint
                // Return the WHEP endpoint URL
                let base_url = std::env::var("PLAYBACK_SERVICE_URL")
                    .unwrap_or_else(|_| "http://localhost:8087".to_string());
                match config.source_type {
                    PlaybackSourceType::Stream => {
                        Ok(format!("{}/api/whep/stream/{}", base_url, config.source_id))
                    }
                    PlaybackSourceType::Recording => {
                        Ok(format!("{}/api/whep/recording/{}", base_url, config.source_id))
                    }
                }
            }
        }
    }

    fn find_recording_path(&self, recording_id: &str) -> Result<PathBuf> {
        // Look for recording file with various extensions
        for ext in &["mp4", "mkv", "m3u8"] {
            let path = self.recording_storage_root.join(format!("{}.{}", recording_id, ext));
            if path.exists() {
                return Ok(path);
            }
            // Also check in subdirectory (HLS recordings)
            let path = self.recording_storage_root.join(recording_id).join(format!("index.{}", ext));
            if path.exists() {
                return Ok(path);
            }
        }
        Err(anyhow!("Recording file not found: {}", recording_id))
    }

    async fn get_recording_duration(&self, recording_id: &str) -> Result<f64> {
        let recording_path = self.find_recording_path(recording_id)?;

        // Use ffprobe to get duration
        let output = tokio::process::Command::new("ffprobe")
            .args(&[
                "-v", "error",
                "-show_entries", "format=duration",
                "-of", "default=noprint_wrappers=1:nokey=1",
                recording_path.to_str().unwrap(),
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow!("Failed to probe recording duration"));
        }

        let duration_str = String::from_utf8(output.stdout)?.trim().to_string();
        let duration: f64 = duration_str.parse()?;
        Ok(duration)
    }

    async fn update_state(&self, session_id: &str, state: PlaybackState) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        if let Some(session_data) = sessions.get_mut(session_id) {
            session_data.info.state = state;
            if let Some(store) = &self.store {
                store.save(&session_data.info).await?;
            }
            Ok(())
        } else {
            Err(anyhow!("Session not found: {}", session_id))
        }
    }

    /// Generate LL-HLS playlist for a stream
    pub async fn generate_ll_hls_playlist(
        &self,
        stream_id: &str,
        blocking_params: Option<BlockingParams>,
    ) -> Result<String> {
        let stream_path = self.stream_hls_root.join(stream_id);

        if !stream_path.exists() {
            return Err(anyhow!("Stream not found: {}", stream_id));
        }

        // Get current sequence number (simplified - in production, track this properly)
        let sequence_number = 0;

        let response = self.ll_hls_generator
            .generate_media_playlist(&stream_path, sequence_number, blocking_params)
            .await?;

        Ok(response.content)
    }

    /// Get LL-HLS configuration
    pub fn ll_hls_config(&self) -> &LlHlsPlaylistGenerator {
        &self.ll_hls_generator
    }

    // === DVR Methods ===

    /// Get DVR window information for a session
    pub async fn get_dvr_window(&self, session_id: &str) -> Result<DvrWindowInfo> {
        let sessions = self.sessions.read().await;
        if let Some(session_data) = sessions.get(session_id) {
            if let Some(dvr_manager) = &session_data.dvr_manager {
                // Scan latest segments
                dvr_manager.scan_segments().await?;

                // Get current position from session
                let current_pos = session_data
                    .info
                    .dvr_window
                    .as_ref()
                    .and_then(|w| w.current_position);

                let window = dvr_manager.get_window(current_pos).await?;
                Ok(window)
            } else {
                Err(anyhow!("DVR not enabled for this session"))
            }
        } else {
            Err(anyhow!("Session not found: {}", session_id))
        }
    }

    /// Seek to a specific timestamp in DVR buffer
    pub async fn dvr_seek(&self, request: DvrSeekRequest) -> Result<DvrSeekResponse> {
        let mut sessions = self.sessions.write().await;
        if let Some(session_data) = sessions.get_mut(&request.session_id) {
            if let Some(dvr_manager) = &session_data.dvr_manager {
                // Determine target timestamp
                let target_timestamp = if let Some(ts) = request.timestamp_secs {
                    ts
                } else if let Some(offset) = request.relative_offset_secs {
                    // Calculate timestamp from live edge
                    if let Some(live_edge) = dvr_manager.get_live_edge().await? {
                        if offset <= 0.0 {
                            // Negative offset = go back in time
                            live_edge.saturating_sub(offset.abs() as u64)
                        } else {
                            live_edge // Can't go beyond live
                        }
                    } else {
                        return Err(anyhow!("No live edge available"));
                    }
                } else {
                    return Err(anyhow!("Must provide either timestamp_secs or relative_offset_secs"));
                };

                // Validate timestamp is within DVR window
                let window = dvr_manager.get_window(None).await?;
                if target_timestamp < window.earliest_available
                    || target_timestamp > window.latest_available
                {
                    return Err(anyhow!(
                        "Timestamp {} out of DVR window range ({} - {})",
                        target_timestamp,
                        window.earliest_available,
                        window.latest_available
                    ));
                }

                // Find segment containing this timestamp
                if let Some(segment) = dvr_manager.find_segment_at_timestamp(target_timestamp).await? {
                    info!(
                        session_id = %request.session_id,
                        timestamp = target_timestamp,
                        segment = %segment.filename,
                        "DVR seek successful"
                    );

                    // Update session position
                    session_data.info.current_position_secs =
                        Some((target_timestamp - segment.start_timestamp) as f64);

                    // Update DVR window with new position
                    let updated_window = dvr_manager.get_window(Some(target_timestamp)).await?;
                    session_data.info.dvr_window = Some(updated_window.clone());

                    // Save to database
                    if let Some(store) = &self.store {
                        store.save(&session_data.info).await?;
                    }

                    Ok(DvrSeekResponse {
                        success: true,
                        timestamp_secs: Some(target_timestamp),
                        live_offset_secs: updated_window.live_offset_secs,
                        message: Some(format!("Seeked to segment: {}", segment.filename)),
                    })
                } else {
                    Err(anyhow!("No segment found for timestamp: {}", target_timestamp))
                }
            } else {
                Err(anyhow!("DVR not enabled for this session"))
            }
        } else {
            Err(anyhow!("Session not found: {}", request.session_id))
        }
    }

    /// Jump to live edge (exit DVR mode, return to live)
    pub async fn jump_to_live(&self, session_id: &str) -> Result<DvrSeekResponse> {
        let mut sessions = self.sessions.write().await;
        if let Some(session_data) = sessions.get_mut(session_id) {
            if let Some(dvr_manager) = &session_data.dvr_manager {
                // Get live edge
                let live_edge = dvr_manager
                    .get_live_edge()
                    .await?
                    .ok_or_else(|| anyhow!("No live edge available"))?;

                // Update session to live position
                session_data.info.current_position_secs = None; // Live has no fixed position

                // Update DVR window to live
                let window = dvr_manager.get_window(Some(live_edge)).await?;
                session_data.info.dvr_window = Some(window.clone());

                // Save to database
                if let Some(store) = &self.store {
                    store.save(&session_data.info).await?;
                }

                info!(session_id = %session_id, "Jumped to live edge");

                Ok(DvrSeekResponse {
                    success: true,
                    timestamp_secs: Some(live_edge),
                    live_offset_secs: Some(0.0), // At live edge
                    message: Some("Jumped to live edge".to_string()),
                })
            } else {
                Err(anyhow!("DVR not enabled for this session"))
            }
        } else {
            Err(anyhow!("Session not found: {}", session_id))
        }
    }

    /// Update DVR buffer window size
    pub async fn set_dvr_buffer_limit(&self, session_id: &str, buffer_secs: f64) -> Result<()> {
        let sessions = self.sessions.read().await;
        if let Some(session_data) = sessions.get(session_id) {
            if let Some(dvr_manager) = &session_data.dvr_manager {
                dvr_manager.set_buffer_limit(buffer_secs).await?;
                info!(session_id = %session_id, buffer_secs = %buffer_secs, "DVR buffer limit updated");
                Ok(())
            } else {
                Err(anyhow!("DVR not enabled for this session"))
            }
        } else {
            Err(anyhow!("Session not found: {}", session_id))
        }
    }
}

//! Frame capture and AI integration for active streams
//!
//! This module handles periodic frame extraction from active video streams
//! and submits them to the AI service for processing.

use anyhow::{Context, Result};
use base64::Engine;
use common::frame_extractor;
use reqwest::Client;
use serde_json::json;
use std::time::Duration;
use tokio::time;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Configuration for frame capture and AI processing
#[derive(Clone, Debug)]
pub struct FrameCaptureConfig {
    /// AI service base URL (e.g., "http://localhost:8084")
    pub ai_service_url: String,
    /// AI task ID to submit frames to
    pub ai_task_id: String,
    /// Frame capture interval in seconds
    pub capture_interval_secs: u64,
    /// Frame width (0 = auto)
    pub frame_width: u32,
    /// Frame height (0 = auto)
    pub frame_height: u32,
    /// JPEG quality (2-31, lower is better)
    pub jpeg_quality: u32,
}

impl Default for FrameCaptureConfig {
    fn default() -> Self {
        Self {
            ai_service_url: "http://localhost:8084".to_string(),
            ai_task_id: String::new(),
            capture_interval_secs: 1,
            frame_width: 640,
            frame_height: 0, // auto-scale
            jpeg_quality: 5,
        }
    }
}

/// Start frame capture loop for a stream
///
/// This spawns a background task that periodically extracts frames from the stream
/// and submits them to the AI service.
///
/// # Arguments
/// * `stream_id` - Unique stream identifier
/// * `source_uri` - Video source URI (RTSP, HLS, etc.)
/// * `config` - Frame capture configuration
/// * `cancel_token` - Token to stop the frame capture loop
pub fn start_frame_capture(
    stream_id: String,
    source_uri: String,
    config: FrameCaptureConfig,
    cancel_token: CancellationToken,
) {
    tokio::spawn(async move {
        info!(
            stream_id = %stream_id,
            ai_task_id = %config.ai_task_id,
            interval_secs = config.capture_interval_secs,
            "starting frame capture loop"
        );

        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());

        let mut interval = time::interval(Duration::from_secs(config.capture_interval_secs));
        let mut frame_seq = 0u64;

        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    info!(stream_id = %stream_id, "frame capture cancelled");
                    break;
                }
                _ = interval.tick() => {
                    frame_seq += 1;

                    // Extract frame from stream
                    match frame_extractor::extract_frame_jpeg(
                        &source_uri,
                        config.frame_width,
                        config.frame_height,
                        config.jpeg_quality,
                    ) {
                        Ok(jpeg_data) => {
                            debug!(
                                stream_id = %stream_id,
                                frame_seq = frame_seq,
                                bytes = jpeg_data.len(),
                                "extracted frame"
                            );

                            // Submit frame to AI service
                            if let Err(e) = submit_frame_to_ai(
                                &client,
                                &config.ai_service_url,
                                &config.ai_task_id,
                                frame_seq,
                                jpeg_data,
                            )
                            .await
                            {
                                warn!(
                                    stream_id = %stream_id,
                                    frame_seq = frame_seq,
                                    error = %e,
                                    "failed to submit frame to AI service"
                                );
                            }
                        }
                        Err(e) => {
                            error!(
                                stream_id = %stream_id,
                                frame_seq = frame_seq,
                                error = %e,
                                "failed to extract frame from stream"
                            );
                        }
                    }
                }
            }
        }

        info!(stream_id = %stream_id, total_frames = frame_seq, "frame capture stopped");
    });
}

/// Submit a frame to the AI service
async fn submit_frame_to_ai(
    client: &Client,
    ai_service_url: &str,
    task_id: &str,
    frame_seq: u64,
    jpeg_data: Vec<u8>,
) -> Result<()> {
    let base64_data = base64::engine::general_purpose::STANDARD.encode(&jpeg_data);

    let url = format!("{}/v1/tasks/{}/frames", ai_service_url, task_id);

    let payload = json!({
        "frame_data": base64_data,
        "sequence_number": frame_seq,
        "timestamp_ms": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
    });

    let response = client
        .post(&url)
        .json(&payload)
        .send()
        .await
        .context("failed to send frame to AI service")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("AI service returned error {}: {}", status, body);
    }

    debug!(task_id = %task_id, frame_seq = frame_seq, "frame submitted to AI service");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = FrameCaptureConfig::default();
        assert_eq!(config.capture_interval_secs, 1);
        assert_eq!(config.frame_width, 640);
        assert_eq!(config.frame_height, 0);
        assert_eq!(config.jpeg_quality, 5);
    }
}

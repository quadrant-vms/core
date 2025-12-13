//! Time-axis preview generation for recordings
//!
//! This module provides functionality to generate thumbnail previews along
//! the timeline of a recording, useful for video scrubbing and navigation.

use anyhow::{Context, Result};
use base64::Engine;
use common::playback::{
    PlaybackSourceType, TimeAxisPreviewRequest, TimeAxisPreviewResponse, TimeAxisThumbnail,
};
use common::thumbnail::{generate_thumbnail_grid, probe_video_duration};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Configuration for time-axis preview generation
pub struct PreviewConfig {
    /// Default number of thumbnails if not specified
    pub default_count: u32,
    /// Default thumbnail width
    pub default_width: u32,
    /// Default thumbnail height
    pub default_height: u32,
    /// Default JPEG quality
    pub default_quality: u32,
    /// Maximum number of thumbnails allowed per request
    pub max_count: u32,
}

impl Default for PreviewConfig {
    fn default() -> Self {
        Self {
            default_count: 10,
            default_width: 320,
            default_height: 180,
            default_quality: 5,
            max_count: 100,
        }
    }
}

/// Generate time-axis preview for a recording
pub fn generate_time_axis_preview(
    request: TimeAxisPreviewRequest,
    recording_storage_root: &Path,
    config: &PreviewConfig,
) -> Result<TimeAxisPreviewResponse> {
    info!(
        source_id = %request.source_id,
        source_type = ?request.source_type,
        count = request.count,
        "generating time-axis preview"
    );

    // Validate count
    let count = request.count.min(config.max_count);
    if count == 0 {
        anyhow::bail!("thumbnail count must be greater than 0");
    }

    // Only recordings are supported for now
    // (Live streams would require different handling)
    if request.source_type != PlaybackSourceType::Recording {
        anyhow::bail!("time-axis preview is only supported for recordings");
    }

    // Find the recording file
    let recording_path = find_recording_path(recording_storage_root, &request.source_id)?;

    // Get video duration
    let duration_secs = probe_video_duration(&recording_path)
        .context("failed to probe video duration")?;

    debug!(
        recording_path = %recording_path.display(),
        duration_secs = duration_secs,
        "probed recording duration"
    );

    // Generate thumbnails
    let width = request.width.unwrap_or(config.default_width);
    let height = request.height.unwrap_or(config.default_height);
    let quality = request.quality.unwrap_or(config.default_quality);

    let raw_thumbnails = generate_thumbnail_grid(
        &recording_path,
        count,
        width,
        height,
        quality,
    )
    .context("failed to generate thumbnail grid")?;

    // Convert to response format with position percentages
    let thumbnails: Vec<TimeAxisThumbnail> = raw_thumbnails
        .into_iter()
        .map(|(timestamp_secs, jpeg_data)| {
            let position_percent = if duration_secs > 0.0 {
                timestamp_secs / duration_secs
            } else {
                0.0
            };

            let image_data = base64::engine::general_purpose::STANDARD.encode(&jpeg_data);

            TimeAxisThumbnail {
                timestamp_secs,
                position_percent,
                width,
                height,
                image_data,
            }
        })
        .collect();

    info!(
        source_id = %request.source_id,
        thumbnail_count = thumbnails.len(),
        duration_secs = duration_secs,
        "time-axis preview generated successfully"
    );

    Ok(TimeAxisPreviewResponse {
        source_id: request.source_id,
        source_type: request.source_type,
        duration_secs,
        thumbnails,
    })
}

/// Find the recording file path from the recording ID
fn find_recording_path(storage_root: &Path, recording_id: &str) -> Result<PathBuf> {
    // Try different possible file extensions
    let extensions = ["mp4", "mkv", "m3u8"];

    for ext in &extensions {
        let path = storage_root.join(format!("{}.{}", recording_id, ext));
        if path.exists() {
            debug!(
                recording_id = recording_id,
                path = %path.display(),
                "found recording file"
            );
            return Ok(path);
        }
    }

    // Also check in subdirectories (for HLS)
    let hls_path = storage_root.join(recording_id).join("index.m3u8");
    if hls_path.exists() {
        debug!(
            recording_id = recording_id,
            path = %hls_path.display(),
            "found HLS recording"
        );
        return Ok(hls_path);
    }

    warn!(
        recording_id = recording_id,
        storage_root = %storage_root.display(),
        "recording file not found"
    );
    anyhow::bail!("recording file not found: {}", recording_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_preview_config() {
        let config = PreviewConfig::default();
        assert_eq!(config.default_count, 10);
        assert_eq!(config.default_width, 320);
        assert_eq!(config.default_height, 180);
        assert_eq!(config.default_quality, 5);
        assert_eq!(config.max_count, 100);
    }

    #[test]
    fn test_find_recording_path_nonexistent() {
        let storage_root = PathBuf::from("/tmp/nonexistent");
        let result = find_recording_path(&storage_root, "test-recording");
        assert!(result.is_err());
    }
}

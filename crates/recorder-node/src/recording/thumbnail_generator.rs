//! Thumbnail generation for recordings

use anyhow::{Context, Result};
use base64::Engine;
use common::thumbnail::{generate_thumbnail, generate_thumbnail_grid, probe_video_duration};
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

/// Configuration for thumbnail generation
pub struct ThumbnailConfig {
    pub width: u32,
    pub height: u32,
    pub quality: u32,
}

impl Default for ThumbnailConfig {
    fn default() -> Self {
        Self {
            width: 320,
            height: 180,
            quality: 5,
        }
    }
}

/// Generate a single thumbnail for a recording at a specific timestamp
pub fn generate_recording_thumbnail(
    recording_path: &Path,
    timestamp_secs: Option<f64>,
    config: &ThumbnailConfig,
) -> Result<(f64, String)> {
    debug!(
        recording = %recording_path.display(),
        timestamp = ?timestamp_secs,
        "generating thumbnail for recording"
    );

    // If no timestamp specified, use middle of the video
    let timestamp = if let Some(ts) = timestamp_secs {
        ts
    } else {
        let duration = probe_video_duration(recording_path)
            .context("failed to probe video duration")?;
        duration / 2.0
    };

    let jpeg_data = generate_thumbnail(
        recording_path,
        timestamp,
        config.width,
        config.height,
        config.quality,
    )
    .context("failed to generate thumbnail")?;

    // Encode to base64 for transport
    let base64_data = base64::engine::general_purpose::STANDARD.encode(&jpeg_data);

    debug!(
        recording = %recording_path.display(),
        timestamp = timestamp,
        size_bytes = jpeg_data.len(),
        "thumbnail generated successfully"
    );

    Ok((timestamp, base64_data))
}

/// Generate multiple thumbnails for a recording
pub fn generate_recording_thumbnail_grid(
    recording_path: &Path,
    count: u32,
    config: &ThumbnailConfig,
) -> Result<Vec<(f64, String)>> {
    debug!(
        recording = %recording_path.display(),
        count = count,
        "generating thumbnail grid for recording"
    );

    let thumbnails = generate_thumbnail_grid(
        recording_path,
        count,
        config.width,
        config.height,
        config.quality,
    )
    .context("failed to generate thumbnail grid")?;

    // Encode all thumbnails to base64
    let encoded_thumbnails: Vec<(f64, String)> = thumbnails
        .into_iter()
        .map(|(ts, jpeg_data)| {
            let base64_data = base64::engine::general_purpose::STANDARD.encode(&jpeg_data);
            (ts, base64_data)
        })
        .collect();

    debug!(
        recording = %recording_path.display(),
        count = encoded_thumbnails.len(),
        "thumbnail grid generated successfully"
    );

    Ok(encoded_thumbnails)
}

/// Find the recording file path from the recording ID
pub fn find_recording_path(storage_root: &Path, recording_id: &str) -> Result<PathBuf> {
    // Validate recording_id to prevent path traversal
    common::validation::validate_id(recording_id, "recording_id")?;

    // Try different possible file extensions
    let extensions = ["mp4", "mkv", "m3u8"];

    for ext in &extensions {
        let path = storage_root.join(format!("{}.{}", recording_id, ext));

        // Ensure the resolved path is within the storage root
        common::validation::validate_path_components(&path, Some(storage_root), "recording_path")?;

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

    // Ensure the resolved path is within the storage root
    common::validation::validate_path_components(&hls_path, Some(storage_root), "recording_path")?;

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
    use std::path::PathBuf;

    #[test]
    fn test_default_thumbnail_config() {
        let config = ThumbnailConfig::default();
        assert_eq!(config.width, 320);
        assert_eq!(config.height, 180);
        assert_eq!(config.quality, 5);
    }

    #[test]
    fn test_find_recording_path_nonexistent() {
        let storage_root = PathBuf::from("/tmp/nonexistent");
        let result = find_recording_path(&storage_root, "test-recording");
        assert!(result.is_err());
    }
}

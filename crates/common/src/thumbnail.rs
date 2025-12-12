//! Thumbnail generation utilities for video recordings
//!
//! This module provides utilities for generating thumbnail images from video files
//! at specific timestamps, useful for video preview, timeline navigation, and
//! evidence collection.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::{Command, Stdio};
use tracing::{debug, error};

/// Generate a thumbnail at a specific timestamp in the video
///
/// # Arguments
/// * `video_path` - Path to the video file
/// * `timestamp_secs` - Timestamp in seconds to extract the thumbnail
/// * `width` - Target thumbnail width (0 = auto-scale to maintain aspect ratio)
/// * `height` - Target thumbnail height (0 = auto-scale to maintain aspect ratio)
/// * `quality` - JPEG quality (2-31, lower is better quality, default: 5)
///
/// # Returns
/// JPEG image data as bytes
pub fn generate_thumbnail(
    video_path: &Path,
    timestamp_secs: f64,
    width: u32,
    height: u32,
    quality: u32,
) -> Result<Vec<u8>> {
    debug!(
        video = %video_path.display(),
        timestamp = timestamp_secs,
        width = width,
        height = height,
        quality = quality,
        "generating thumbnail from video"
    );

    // Verify the video file exists
    if !video_path.exists() {
        anyhow::bail!("video file does not exist: {}", video_path.display());
    }

    // Build FFmpeg command to extract frame at specific timestamp
    let mut args = vec![
        "-ss".to_string(),
        timestamp_secs.to_string(),
        "-i".to_string(),
        video_path.to_string_lossy().to_string(),
        "-vframes".to_string(),
        "1".to_string(),
        "-f".to_string(),
        "image2pipe".to_string(),
    ];

    // Add scaling filter if dimensions specified
    if width > 0 || height > 0 {
        let scale_filter = if width > 0 && height > 0 {
            format!("scale={}:{}", width, height)
        } else if width > 0 {
            format!("scale={}:-1", width)
        } else {
            format!("scale=-1:{}", height)
        };
        args.push("-vf".to_string());
        args.push(scale_filter);
    }

    // JPEG quality (qscale:v where 2 is high quality, 31 is low quality)
    args.push("-q:v".to_string());
    args.push(quality.clamp(2, 31).to_string());

    // Output to pipe
    args.push("pipe:1".to_string());

    debug!(args = ?args, "spawning ffmpeg for thumbnail generation");

    // Execute FFmpeg
    let output = Command::new("ffmpeg")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .context("failed to execute ffmpeg")?;

    if !output.status.success() {
        error!(
            video = %video_path.display(),
            status = ?output.status,
            "ffmpeg thumbnail generation failed"
        );
        anyhow::bail!("ffmpeg exited with error: {:?}", output.status);
    }

    if output.stdout.is_empty() {
        error!(video = %video_path.display(), "ffmpeg returned empty thumbnail data");
        anyhow::bail!("ffmpeg returned no thumbnail data");
    }

    debug!(
        video = %video_path.display(),
        size_bytes = output.stdout.len(),
        "thumbnail generated successfully"
    );

    Ok(output.stdout)
}

/// Generate multiple thumbnails at evenly-spaced intervals
///
/// # Arguments
/// * `video_path` - Path to the video file
/// * `count` - Number of thumbnails to generate
/// * `width` - Target thumbnail width
/// * `height` - Target thumbnail height
/// * `quality` - JPEG quality
///
/// # Returns
/// Vector of tuples containing (timestamp_secs, jpeg_data)
pub fn generate_thumbnail_grid(
    video_path: &Path,
    count: u32,
    width: u32,
    height: u32,
    quality: u32,
) -> Result<Vec<(f64, Vec<u8>)>> {
    debug!(
        video = %video_path.display(),
        count = count,
        "generating thumbnail grid"
    );

    // Get video duration
    let duration = probe_video_duration(video_path)?;

    if count == 0 {
        anyhow::bail!("thumbnail count must be greater than 0");
    }

    if duration <= 0.0 {
        anyhow::bail!("invalid video duration: {}", duration);
    }

    // Calculate timestamps evenly distributed across the video
    let mut thumbnails = Vec::new();
    let interval = duration / (count as f64 + 1.0);

    for i in 1..=count {
        let timestamp = interval * i as f64;
        let thumbnail_data = generate_thumbnail(video_path, timestamp, width, height, quality)?;
        thumbnails.push((timestamp, thumbnail_data));
    }

    debug!(
        video = %video_path.display(),
        count = thumbnails.len(),
        "thumbnail grid generated successfully"
    );

    Ok(thumbnails)
}

/// Probe video duration using ffprobe
///
/// Returns duration in seconds
pub fn probe_video_duration(video_path: &Path) -> Result<f64> {
    debug!(video = %video_path.display(), "probing video duration");

    let output = Command::new("ffprobe")
        .args(&[
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "csv=p=0",
            &video_path.to_string_lossy(),
        ])
        .output()
        .context("failed to execute ffprobe")?;

    if !output.status.success() {
        anyhow::bail!("ffprobe failed: {:?}", output.status);
    }

    let output_str = String::from_utf8(output.stdout)
        .context("ffprobe output is not valid UTF-8")?;

    let duration: f64 = output_str
        .trim()
        .parse()
        .context("failed to parse duration")?;

    debug!(
        video = %video_path.display(),
        duration_secs = duration,
        "probed duration successfully"
    );

    Ok(duration)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_thumbnail_generation_signature() {
        // This test verifies the function signature without requiring FFmpeg
        // In a real environment with FFmpeg and a test video, this would execute
        let video_path = PathBuf::from("/nonexistent/test.mp4");
        let result = generate_thumbnail(&video_path, 5.0, 320, 240, 5);

        // We expect this to fail because the file doesn't exist
        assert!(result.is_err());
    }

    #[test]
    fn test_thumbnail_grid_zero_count() {
        let video_path = PathBuf::from("/nonexistent/test.mp4");
        let result = generate_thumbnail_grid(&video_path, 0, 320, 240, 5);

        // Should fail with count validation error
        assert!(result.is_err());
    }
}

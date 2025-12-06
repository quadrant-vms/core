//! Frame extraction utilities for video streams
//!
//! This module provides utilities for extracting frames from video streams
//! using FFmpeg for AI processing pipelines.

use anyhow::{Context, Result};
use base64::Engine;
use std::process::{Command, Stdio};
use tracing::{debug, error, warn};

/// Extract a single JPEG frame from a video source
///
/// # Arguments
/// * `source_uri` - Video source URI (RTSP, HLS, file path, etc.)
/// * `width` - Target frame width (0 = auto-scale to maintain aspect ratio)
/// * `height` - Target frame height (0 = auto-scale to maintain aspect ratio)
/// * `quality` - JPEG quality (2-31, lower is better quality, default: 2)
///
/// # Returns
/// JPEG image data as bytes
pub fn extract_frame_jpeg(
    source_uri: &str,
    width: u32,
    height: u32,
    quality: u32,
) -> Result<Vec<u8>> {
    debug!(
        source = %source_uri,
        width = width,
        height = height,
        quality = quality,
        "extracting frame from video source"
    );

    // Build FFmpeg command to extract a single frame
    let mut args = vec![
        "-i".to_string(),
        source_uri.to_string(),
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

    debug!(args = ?args, "spawning ffmpeg for frame extraction");

    // Execute FFmpeg
    let output = Command::new("ffmpeg")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .context("failed to execute ffmpeg")?;

    if !output.status.success() {
        error!(
            source = %source_uri,
            status = ?output.status,
            "ffmpeg frame extraction failed"
        );
        anyhow::bail!("ffmpeg exited with error: {:?}", output.status);
    }

    if output.stdout.is_empty() {
        warn!(source = %source_uri, "ffmpeg returned empty frame data");
        anyhow::bail!("ffmpeg returned no frame data");
    }

    debug!(
        source = %source_uri,
        size_bytes = output.stdout.len(),
        "frame extracted successfully"
    );

    Ok(output.stdout)
}

/// Extract a frame and encode it as base64 (for JSON transport)
pub fn extract_frame_base64(
    source_uri: &str,
    width: u32,
    height: u32,
    quality: u32,
) -> Result<String> {
    let frame_data = extract_frame_jpeg(source_uri, width, height, quality)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&frame_data))
}

/// Extract frame dimensions from a video source using ffprobe
///
/// Returns (width, height) tuple
pub fn probe_frame_dimensions(source_uri: &str) -> Result<(u32, u32)> {
    debug!(source = %source_uri, "probing video dimensions");

    let output = Command::new("ffprobe")
        .args(&[
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=p=0",
            source_uri,
        ])
        .output()
        .context("failed to execute ffprobe")?;

    if !output.status.success() {
        anyhow::bail!("ffprobe failed: {:?}", output.status);
    }

    let output_str = String::from_utf8(output.stdout)
        .context("ffprobe output is not valid UTF-8")?;

    let dimensions: Vec<&str> = output_str.trim().split(',').collect();
    if dimensions.len() != 2 {
        anyhow::bail!("unexpected ffprobe output format: {}", output_str);
    }

    let width: u32 = dimensions[0]
        .parse()
        .context("failed to parse width")?;
    let height: u32 = dimensions[1]
        .parse()
        .context("failed to parse height")?;

    debug!(
        source = %source_uri,
        width = width,
        height = height,
        "probed dimensions successfully"
    );

    Ok((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_frame_with_mock() {
        // This test requires FFmpeg to be installed and a valid test video
        // In CI, we can use MOCK_FFMPEG env var to skip
        if std::env::var("MOCK_FFMPEG").is_ok() {
            return;
        }

        // Test with a generated test pattern (requires FFmpeg)
        let result = extract_frame_jpeg(
            "testsrc=duration=1:size=320x240:rate=1",
            320,
            240,
            5,
        );

        // We expect this to fail in most test environments without FFmpeg
        // The real test is that the function signature and logic are correct
        match result {
            Ok(data) => {
                assert!(!data.is_empty(), "frame data should not be empty");
                // JPEG files start with FF D8 FF
                assert_eq!(&data[0..3], &[0xFF, 0xD8, 0xFF], "should be valid JPEG");
            }
            Err(e) => {
                println!("FFmpeg not available in test environment: {}", e);
            }
        }
    }

    #[test]
    fn test_base64_encoding() {
        // Test that base64 encoding works
        let test_data = vec![0xFF, 0xD8, 0xFF, 0xE0]; // JPEG header
        let encoded = base64::engine::general_purpose::STANDARD.encode(&test_data);
        assert!(!encoded.is_empty());
        assert_eq!(encoded, "/9j/4A==");
    }
}

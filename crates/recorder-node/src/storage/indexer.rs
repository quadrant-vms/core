use anyhow::{anyhow, Context, Result};
use common::recordings::RecordingMetadata;
use std::path::Path;
use std::process::Command;
use tracing::info;

pub struct MediaIndexer;

impl MediaIndexer {
  pub async fn extract_metadata(file_path: &Path) -> Result<RecordingMetadata> {
    if !file_path.exists() {
      return Err(anyhow!("file does not exist: {:?}", file_path));
    }

    info!(path = ?file_path, "extracting metadata with ffprobe");

    let output = Command::new("ffprobe")
      .args(&[
        "-v",
        "quiet",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
        file_path
          .to_str()
          .ok_or_else(|| anyhow!("invalid path"))?,
      ])
      .output()
      .context("failed to run ffprobe")?;

    if !output.status.success() {
      return Err(anyhow!("ffprobe failed with status: {:?}", output.status));
    }

    let json: serde_json::Value =
      serde_json::from_slice(&output.stdout).context("failed to parse ffprobe output")?;

    // Extract metadata from ffprobe JSON
    let mut metadata = RecordingMetadata {
      duration_secs: None,
      file_size_bytes: None,
      video_codec: None,
      audio_codec: None,
      resolution: None,
      bitrate_kbps: None,
      fps: None,
    };

    // Get file size
    if let Ok(file_metadata) = std::fs::metadata(file_path) {
      metadata.file_size_bytes = Some(file_metadata.len());
    }

    // Parse format info
    if let Some(format) = json.get("format") {
      if let Some(duration) = format.get("duration").and_then(|d| d.as_str()) {
        if let Ok(dur) = duration.parse::<f64>() {
          metadata.duration_secs = Some(dur as u64);
        }
      }
      if let Some(bitrate) = format.get("bit_rate").and_then(|b| b.as_str()) {
        if let Ok(br) = bitrate.parse::<u64>() {
          metadata.bitrate_kbps = Some((br / 1000) as u32);
        }
      }
    }

    // Parse stream info
    if let Some(streams) = json.get("streams").and_then(|s| s.as_array()) {
      for stream in streams {
        let codec_type = stream.get("codec_type").and_then(|t| t.as_str());

        match codec_type {
          Some("video") => {
            if let Some(codec) = stream.get("codec_name").and_then(|c| c.as_str()) {
              metadata.video_codec = Some(codec.to_string());
            }
            if let (Some(width), Some(height)) = (
              stream.get("width").and_then(|w| w.as_u64()),
              stream.get("height").and_then(|h| h.as_u64()),
            ) {
              metadata.resolution = Some((width as u32, height as u32));
            }
            if let Some(fps_str) = stream.get("r_frame_rate").and_then(|f| f.as_str()) {
              if let Some((num, den)) = fps_str.split_once('/') {
                if let (Ok(n), Ok(d)) = (num.parse::<f32>(), den.parse::<f32>()) {
                  if d != 0.0 {
                    metadata.fps = Some(n / d);
                  }
                }
              }
            }
          }
          Some("audio") => {
            if let Some(codec) = stream.get("codec_name").and_then(|c| c.as_str()) {
              metadata.audio_codec = Some(codec.to_string());
            }
          }
          _ => {}
        }
      }
    }

    info!(path = ?file_path, metadata = ?metadata, "metadata extracted successfully");

    Ok(metadata)
  }

  #[allow(dead_code)]
  pub async fn index_recording(recording_id: &str, metadata: RecordingMetadata) -> Result<()> {
    // Note: Metadata storage is handled by RecordingManager via StateStore
    // This method is kept for future indexing/catalog functionality
    // (e.g., search index, time-series database, etc.)
    info!(
      recording_id = %recording_id,
      metadata = ?metadata,
      "recording indexed (metadata stored via StateStore)"
    );
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::path::PathBuf;

  #[tokio::test]
  async fn test_extract_metadata() {
    // Test with non-existent file - should return error
    let path = PathBuf::from("/tmp/nonexistent-test-file.mp4");
    let result = MediaIndexer::extract_metadata(&path).await;
    assert!(result.is_err(), "should fail on non-existent file");

    // Real metadata extraction requires ffprobe and a valid video file
    // In a real test environment, you would create a test fixture
  }
}

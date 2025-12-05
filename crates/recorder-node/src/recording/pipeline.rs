use anyhow::{anyhow, Context, Result};
use common::recordings::{RecordingConfig, RecordingFormat, RecordingMetadata};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::fs;
use tracing::{error, info, warn};

pub struct RecordingPipeline {
  config: RecordingConfig,
  output_path: PathBuf,
  process: Option<Child>,
  stopped: bool,
}

impl RecordingPipeline {
  pub fn new(config: RecordingConfig) -> Self {
    let output_path = Self::generate_output_path(&config);
    Self {
      config,
      output_path,
      process: None,
      stopped: false,
    }
  }

  fn generate_output_path(config: &RecordingConfig) -> PathBuf {
    let base_dir = std::env::var("RECORDINGS_ROOT")
      .unwrap_or_else(|_| "./data/recordings".to_string());
    let base_path = PathBuf::from(base_dir);

    let format = config.format.as_ref().unwrap_or(&RecordingFormat::Mp4);
    match format {
      RecordingFormat::Mp4 => base_path.join(&config.id).join("recording.mp4"),
      RecordingFormat::Hls => base_path.join(&config.id).join("index.m3u8"),
      RecordingFormat::Mkv => base_path.join(&config.id).join("recording.mkv"),
    }
  }

  pub fn output_path(&self) -> &Path {
    &self.output_path
  }

  pub async fn run(&mut self) -> Result<()> {
    info!(
      id = %self.config.id,
      source_uri = ?self.config.source_uri,
      output_path = ?self.output_path,
      "starting recording pipeline"
    );

    let source_uri = self
      .config
      .source_uri
      .as_ref()
      .ok_or_else(|| anyhow!("source_uri is required"))?;

    // Create output directory
    if let Some(parent) = self.output_path.parent() {
      fs::create_dir_all(parent)
        .await
        .context("failed to create output directory")?;
    }

    // Build FFmpeg command based on output format
    let format = self.config.format.as_ref().unwrap_or(&RecordingFormat::Mp4);
    let args = self.build_ffmpeg_args(source_uri, format)?;

    info!(id = %self.config.id, args = ?args, "launching ffmpeg");

    // Spawn FFmpeg process
    let child = Command::new("ffmpeg")
      .args(&args)
      .stdout(Stdio::null())
      .stderr(Stdio::piped())
      .spawn()
      .context("failed to spawn ffmpeg")?;

    // Store process handle
    self.process = Some(child);

    // Wait for process to complete or be stopped
    let result = self.monitor_process().await;

    match result {
      Ok(_) => {
        info!(id = %self.config.id, "recording completed successfully");
        Ok(())
      }
      Err(e) => {
        error!(id = %self.config.id, error = %e, "recording failed");
        Err(e)
      }
    }
  }

  fn build_ffmpeg_args(&self, source_uri: &str, format: &RecordingFormat) -> Result<Vec<String>> {
    let mut args = vec![];

    // Input options
    args.push("-i".to_string());
    args.push(source_uri.to_string());

    // Codec settings - copy streams when possible for efficiency
    args.push("-c:v".to_string());
    args.push("copy".to_string());
    args.push("-c:a".to_string());
    args.push("copy".to_string());

    // Format-specific options
    match format {
      RecordingFormat::Mp4 => {
        // MP4 container settings
        args.push("-movflags".to_string());
        args.push("faststart".to_string()); // Enable fast start for web playback
        args.push("-f".to_string());
        args.push("mp4".to_string());
      }
      RecordingFormat::Hls => {
        // HLS settings
        args.push("-f".to_string());
        args.push("hls".to_string());
        args.push("-hls_time".to_string());
        args.push("2".to_string()); // 2 second segments
        args.push("-hls_list_size".to_string());
        args.push("0".to_string()); // Keep all segments
        args.push("-hls_segment_filename".to_string());
        let segment_pattern = self
          .output_path
          .parent()
          .unwrap()
          .join("segment_%05d.ts")
          .to_string_lossy()
          .to_string();
        args.push(segment_pattern);
      }
      RecordingFormat::Mkv => {
        // MKV container settings
        args.push("-f".to_string());
        args.push("matroska".to_string());
      }
    }

    // Output file
    args.push(
      self
        .output_path
        .to_str()
        .ok_or_else(|| anyhow!("invalid output path"))?
        .to_string(),
    );

    Ok(args)
  }

  async fn monitor_process(&mut self) -> Result<()> {
    let process = self
      .process
      .as_mut()
      .ok_or_else(|| anyhow!("no process running"))?;

    // Poll process status
    loop {
      if self.stopped {
        // Kill process if stopped
        let _ = process.kill();
        let _ = process.wait();
        return Ok(());
      }

      // Check if process is still running
      match process.try_wait() {
        Ok(Some(status)) => {
          if status.success() {
            return Ok(());
          } else {
            return Err(anyhow!("ffmpeg exited with status: {}", status));
          }
        }
        Ok(None) => {
          // Process still running
          tokio::time::sleep(Duration::from_millis(500)).await;
        }
        Err(e) => {
          return Err(anyhow!("failed to check process status: {}", e));
        }
      }
    }
  }

  pub async fn stop(&mut self) -> Result<()> {
    info!(id = %self.config.id, "stopping recording pipeline");
    self.stopped = true;

    if let Some(mut process) = self.process.take() {
      // Send SIGTERM to FFmpeg for graceful shutdown
      let _ = process.kill();

      // Wait for process to terminate
      match tokio::time::timeout(Duration::from_secs(5), async {
        loop {
          if let Ok(Some(_)) = process.try_wait() {
            break;
          }
          tokio::time::sleep(Duration::from_millis(100)).await;
        }
      })
      .await
      {
        Ok(_) => info!(id = %self.config.id, "ffmpeg terminated gracefully"),
        Err(_) => {
          warn!(id = %self.config.id, "ffmpeg did not terminate, forcing kill");
          let _ = process.kill();
        }
      }
    }

    Ok(())
  }

  pub async fn extract_metadata(&self) -> Result<RecordingMetadata> {
    if !self.output_path.exists() {
      return Err(anyhow!("output file does not exist"));
    }

    info!(
      id = %self.config.id,
      path = ?self.output_path,
      "extracting metadata with ffprobe"
    );

    let output = Command::new("ffprobe")
      .args(&[
        "-v",
        "quiet",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
        self
          .output_path
          .to_str()
          .ok_or_else(|| anyhow!("invalid path"))?,
      ])
      .output()
      .context("failed to run ffprobe")?;

    if !output.status.success() {
      return Err(anyhow!("ffprobe failed"));
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
    if let Ok(file_metadata) = std::fs::metadata(&self.output_path) {
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

    info!(
      id = %self.config.id,
      metadata = ?metadata,
      "metadata extracted successfully"
    );

    Ok(metadata)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_generate_output_path_mp4() {
    let config = RecordingConfig {
      id: "test-rec-1".to_string(),
      source_stream_id: None,
      source_uri: Some("rtsp://example.com/stream".to_string()),
      retention_hours: None,
      format: Some(RecordingFormat::Mp4),
    };
    let path = RecordingPipeline::generate_output_path(&config);
    assert!(path.to_string_lossy().contains("test-rec-1"));
    assert!(path.to_string_lossy().ends_with(".mp4"));
  }

  #[test]
  fn test_generate_output_path_hls() {
    let config = RecordingConfig {
      id: "test-rec-2".to_string(),
      source_stream_id: None,
      source_uri: Some("rtsp://example.com/stream".to_string()),
      retention_hours: None,
      format: Some(RecordingFormat::Hls),
    };
    let path = RecordingPipeline::generate_output_path(&config);
    assert!(path.to_string_lossy().contains("test-rec-2"));
    assert!(path.to_string_lossy().ends_with(".m3u8"));
  }

  #[test]
  fn test_build_ffmpeg_args_mp4() {
    let config = RecordingConfig {
      id: "test-rec-3".to_string(),
      source_stream_id: None,
      source_uri: Some("rtsp://example.com/stream".to_string()),
      retention_hours: None,
      format: Some(RecordingFormat::Mp4),
    };
    let pipeline = RecordingPipeline::new(config);
    let args = pipeline
      .build_ffmpeg_args("rtsp://example.com/stream", &RecordingFormat::Mp4)
      .unwrap();

    let joined = args.join(" ");
    assert!(joined.contains("-i rtsp://example.com/stream"));
    assert!(joined.contains("-c:v copy"));
    assert!(joined.contains("-c:a copy"));
    assert!(joined.contains("-f mp4"));
    assert!(joined.contains("faststart"));
  }

  #[test]
  fn test_build_ffmpeg_args_hls() {
    let config = RecordingConfig {
      id: "test-rec-4".to_string(),
      source_stream_id: None,
      source_uri: Some("rtsp://example.com/stream".to_string()),
      retention_hours: None,
      format: Some(RecordingFormat::Hls),
    };
    let pipeline = RecordingPipeline::new(config);
    let args = pipeline
      .build_ffmpeg_args("rtsp://example.com/stream", &RecordingFormat::Hls)
      .unwrap();

    let joined = args.join(" ");
    assert!(joined.contains("-i rtsp://example.com/stream"));
    assert!(joined.contains("-f hls"));
    assert!(joined.contains("-hls_time 2"));
  }
}

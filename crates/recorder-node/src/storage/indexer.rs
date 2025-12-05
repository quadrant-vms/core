use anyhow::Result;
use common::recordings::RecordingMetadata;
use std::path::Path;
use tracing::info;

pub struct MediaIndexer;

impl MediaIndexer {
  pub async fn extract_metadata(_file_path: &Path) -> Result<RecordingMetadata> {
    // TODO: Implement metadata extraction using ffprobe or similar
    // - Duration
    // - File size
    // - Video/audio codecs
    // - Resolution
    // - Bitrate
    // - FPS

    info!("extracting metadata (stub implementation)");

    Ok(RecordingMetadata {
      duration_secs: None,
      file_size_bytes: None,
      video_codec: None,
      audio_codec: None,
      resolution: None,
      bitrate_kbps: None,
      fps: None,
    })
  }

  pub async fn index_recording(_recording_id: &str, _metadata: RecordingMetadata) -> Result<()> {
    // TODO: Store metadata in database/index
    info!("indexing recording (stub implementation)");
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::path::PathBuf;

  #[tokio::test]
  async fn test_extract_metadata() {
    let path = PathBuf::from("/tmp/test.mp4");
    let result = MediaIndexer::extract_metadata(&path).await;
    assert!(result.is_ok());
  }
}

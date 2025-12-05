use anyhow::Result;
use common::recordings::RecordingConfig;
use tracing::info;

pub struct RecordingPipeline {
  config: RecordingConfig,
  stopped: bool,
}

impl RecordingPipeline {
  pub fn new(config: RecordingConfig) -> Self {
    Self {
      config,
      stopped: false,
    }
  }

  pub async fn run(&mut self) -> Result<()> {
    info!(
      id = %self.config.id,
      source_uri = ?self.config.source_uri,
      "recording pipeline running (stub implementation)"
    );

    // TODO: Implement actual recording pipeline
    // - Consume HLS/RTSP stream
    // - Write to MP4/HLS format
    // - Upload to S3
    // - Extract metadata

    while !self.stopped {
      tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }

    Ok(())
  }

  pub async fn stop(&mut self) -> Result<()> {
    info!(id = %self.config.id, "stopping recording pipeline");
    self.stopped = true;
    Ok(())
  }
}

use anyhow::Result;
use common::search::*;
use std::sync::Arc;
use tracing::info;
use crate::recording::manager::RECORDING_MANAGER;
use super::store::SearchStore;

pub struct SearchIndexer {
  store: Arc<dyn SearchStore>,
}

impl SearchIndexer {
  pub fn new(store: Arc<dyn SearchStore>) -> Self {
    Self { store }
  }

  pub async fn index_all_recordings(&self) -> Result<usize> {
    let recordings = RECORDING_MANAGER.list().await;
    let mut indexed = 0;

    for rec in recordings {
      let entry = RecordingIndexEntry {
        id: uuid::Uuid::new_v4().to_string(),
        recording_id: rec.config.id.clone(),
        tenant_id: None,
        device_id: rec.config.source_stream_id.clone(),
        device_name: None,
        zone: None,
        location: None,
        started_at: rec.started_at.unwrap_or(0) as i64,
        stopped_at: rec.stopped_at.map(|t| t as i64),
        duration_secs: rec.metadata.as_ref().and_then(|m| m.duration_secs.map(|d| d as i32)),
        resolution: rec.metadata.as_ref().and_then(|m| {
          m.resolution.map(|(w, h)| format!("{}x{}", w, h))
        }),
        video_codec: rec.metadata.as_ref().and_then(|m| m.video_codec.clone()),
        audio_codec: rec.metadata.as_ref().and_then(|m| m.audio_codec.clone()),
        file_size_bytes: rec.metadata.as_ref().and_then(|m| m.file_size_bytes.map(|s| s as i64)),
        storage_path: rec.storage_path.clone(),
        tags: vec![],
        labels: std::collections::HashMap::new(),
        state: format!("{:?}", rec.state),
        indexed_at: chrono::Utc::now().timestamp(),
        updated_at: chrono::Utc::now().timestamp(),
      };

      self.store.index_recording(&entry).await?;
      indexed += 1;
    }

    info!(count = indexed, "indexed recordings");
    Ok(indexed)
  }
}

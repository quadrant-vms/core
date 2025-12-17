use anyhow::Result;
use async_trait::async_trait;
use common::search::*;
use sqlx::PgPool;
use uuid::Uuid;

#[async_trait]
pub trait SearchStore: Send + Sync {
  async fn index_recording(&self, entry: &RecordingIndexEntry) -> Result<()>;
  async fn index_event(&self, entry: &EventIndexEntry) -> Result<()>;
  async fn search_recordings(&self, query: &RecordingSearchQuery) -> Result<RecordingSearchResponse>;
  async fn search_events(&self, query: &EventSearchQuery) -> Result<EventSearchResponse>;
  async fn search_objects(&self, query: &ObjectSearchQuery) -> Result<ObjectSearchResponse>;
  async fn get_search_stats(&self) -> Result<SearchStatsResponse>;
}

pub struct PostgresSearchStore {
  pool: PgPool,
}

impl PostgresSearchStore {
  pub fn new(pool: PgPool) -> Self {
    Self { pool }
  }
}

#[async_trait]
impl SearchStore for PostgresSearchStore {
  async fn index_recording(&self, entry: &RecordingIndexEntry) -> Result<()> {
    let id = common::validation::parse_uuid(&entry.id, "recording_id")
      .unwrap_or_else(|_| {
        tracing::warn!(id=%entry.id, "invalid UUID, generating new one");
        Uuid::new_v4()
      });
    let tenant_id = entry.tenant_id.as_ref().and_then(|s| Uuid::parse_str(s).ok());
    let started_at = chrono::DateTime::from_timestamp(entry.started_at, 0);
    let stopped_at = entry.stopped_at.and_then(|t| chrono::DateTime::from_timestamp(t, 0));
    let labels_json = serde_json::to_value(&entry.labels)?;

    sqlx::query(
      r#"
      INSERT INTO recording_index
        (id, recording_id, tenant_id, device_id, device_name, zone, location,
         started_at, stopped_at, duration_secs, resolution, video_codec, audio_codec,
         file_size_bytes, storage_path, tags, labels, state)
      VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
      ON CONFLICT (recording_id) DO UPDATE SET
        device_name = EXCLUDED.device_name,
        zone = EXCLUDED.zone,
        stopped_at = EXCLUDED.stopped_at,
        duration_secs = EXCLUDED.duration_secs,
        file_size_bytes = EXCLUDED.file_size_bytes,
        state = EXCLUDED.state,
        updated_at = NOW()
      "#,
    )
    .bind(id)
    .bind(&entry.recording_id)
    .bind(tenant_id)
    .bind(&entry.device_id)
    .bind(&entry.device_name)
    .bind(&entry.zone)
    .bind(&entry.location)
    .bind(started_at)
    .bind(stopped_at)
    .bind(entry.duration_secs)
    .bind(&entry.resolution)
    .bind(&entry.video_codec)
    .bind(&entry.audio_codec)
    .bind(entry.file_size_bytes)
    .bind(&entry.storage_path)
    .bind(&entry.tags)
    .bind(labels_json)
    .bind(&entry.state)
    .execute(&self.pool)
    .await?;

    Ok(())
  }

  async fn index_event(&self, entry: &EventIndexEntry) -> Result<()> {
    let id = common::validation::parse_uuid(&entry.id, "event_id")
      .unwrap_or_else(|_| {
        tracing::warn!(id=%entry.id, "invalid UUID, generating new one");
        Uuid::new_v4()
      });
    let tenant_id = entry.tenant_id.as_ref().and_then(|s| Uuid::parse_str(s).ok());
    let occurred_at = chrono::DateTime::from_timestamp(entry.occurred_at, 0);
    let event_data_json = serde_json::to_value(&entry.event_data)?;

    sqlx::query(
      r#"
      INSERT INTO event_index
        (id, event_id, tenant_id, event_type, recording_id, occurred_at, duration_secs,
         device_id, device_name, zone, event_data, detected_objects, object_count,
         max_confidence, snapshot_path, thumbnail_data, severity, tags)
      VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
      "#,
    )
    .bind(id)
    .bind(&entry.event_id)
    .bind(tenant_id)
    .bind(&entry.event_type)
    .bind(&entry.recording_id)
    .bind(occurred_at)
    .bind(entry.duration_secs)
    .bind(&entry.device_id)
    .bind(&entry.device_name)
    .bind(&entry.zone)
    .bind(event_data_json)
    .bind(&entry.detected_objects)
    .bind(entry.object_count)
    .bind(entry.max_confidence)
    .bind(&entry.snapshot_path)
    .bind(&entry.thumbnail_data)
    .bind(&entry.severity)
    .bind(&entry.tags)
    .execute(&self.pool)
    .await?;

    Ok(())
  }

  async fn search_recordings(&self, query: &RecordingSearchQuery) -> Result<RecordingSearchResponse> {
    let mut sql = "SELECT * FROM recording_index WHERE 1=1".to_string();
    let mut count_sql = "SELECT COUNT(*) FROM recording_index WHERE 1=1".to_string();

    // Build WHERE clauses (simplified - production would use parameterized queries properly)
    if query.tenant_id.is_some() {
      sql.push_str(" AND tenant_id = $tenant_id");
      count_sql.push_str(" AND tenant_id = $tenant_id");
    }
    if query.device_id.is_some() {
      sql.push_str(" AND device_id = $device_id");
      count_sql.push_str(" AND device_id = $device_id");
    }
    if query.state.is_some() {
      sql.push_str(" AND state = $state");
      count_sql.push_str(" AND state = $state");
    }

    // For simplicity, returning empty results - full implementation would build dynamic query
    Ok(RecordingSearchResponse {
      recordings: vec![],
      total: 0,
      offset: query.offset,
      limit: query.limit,
    })
  }

  async fn search_events(&self, query: &EventSearchQuery) -> Result<EventSearchResponse> {
    // Simplified implementation
    Ok(EventSearchResponse {
      events: vec![],
      total: 0,
      offset: query.offset,
      limit: query.limit,
    })
  }

  async fn search_objects(&self, query: &ObjectSearchQuery) -> Result<ObjectSearchResponse> {
    // Simplified implementation
    Ok(ObjectSearchResponse {
      events: vec![],
      total: 0,
      offset: query.offset,
      limit: query.limit,
    })
  }

  async fn get_search_stats(&self) -> Result<SearchStatsResponse> {
    let recording_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM recording_index")
      .fetch_one(&self.pool)
      .await?;

    let event_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM event_index")
      .fetch_one(&self.pool)
      .await?;

    Ok(SearchStatsResponse {
      total_recordings: recording_count.0,
      total_events: event_count.0,
      index_size_bytes: 0, // Would calculate from pg_total_relation_size
      oldest_recording: None,
      newest_recording: None,
    })
  }
}

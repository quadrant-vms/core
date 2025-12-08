use anyhow::{Context, Result};
use async_trait::async_trait;
use common::ai_tasks::{AiTaskConfig, AiTaskInfo, AiTaskState};
use common::recordings::{RecordingConfig, RecordingFormat, RecordingInfo, RecordingMetadata, RecordingState};
use common::state_store::StateStore;
use common::streams::{StreamConfig, StreamInfo, StreamState};
use sqlx::PgPool;
use tracing::warn;

pub struct PgStateStore {
    pool: PgPool,
}

impl PgStateStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn parse_stream_state(s: &str) -> StreamState {
        match s {
            "pending" => StreamState::Pending,
            "starting" => StreamState::Starting,
            "running" => StreamState::Running,
            "stopping" => StreamState::Stopping,
            "stopped" => StreamState::Stopped,
            "error" => StreamState::Error,
            _ => {
                warn!("unknown stream state: {}, defaulting to error", s);
                StreamState::Error
            }
        }
    }

    fn stream_state_to_str(state: &StreamState) -> &'static str {
        match state {
            StreamState::Pending => "pending",
            StreamState::Starting => "starting",
            StreamState::Running => "running",
            StreamState::Stopping => "stopping",
            StreamState::Stopped => "stopped",
            StreamState::Error => "error",
        }
    }

    fn parse_recording_state(s: &str) -> RecordingState {
        match s {
            "pending" => RecordingState::Pending,
            "starting" => RecordingState::Starting,
            "recording" => RecordingState::Recording,
            "paused" => RecordingState::Paused,
            "stopping" => RecordingState::Stopping,
            "stopped" => RecordingState::Stopped,
            "error" => RecordingState::Error,
            _ => {
                warn!("unknown recording state: {}, defaulting to error", s);
                RecordingState::Error
            }
        }
    }

    fn recording_state_to_str(state: &RecordingState) -> &'static str {
        match state {
            RecordingState::Pending => "pending",
            RecordingState::Starting => "starting",
            RecordingState::Recording => "recording",
            RecordingState::Paused => "paused",
            RecordingState::Stopping => "stopping",
            RecordingState::Stopped => "stopped",
            RecordingState::Error => "error",
        }
    }

    fn parse_ai_task_state(s: &str) -> AiTaskState {
        match s {
            "pending" => AiTaskState::Pending,
            "initializing" => AiTaskState::Initializing,
            "processing" => AiTaskState::Processing,
            "paused" => AiTaskState::Paused,
            "stopping" => AiTaskState::Stopping,
            "stopped" => AiTaskState::Stopped,
            "error" => AiTaskState::Error,
            _ => {
                warn!("unknown ai task state: {}, defaulting to error", s);
                AiTaskState::Error
            }
        }
    }

    fn ai_task_state_to_str(state: &AiTaskState) -> &'static str {
        match state {
            AiTaskState::Pending => "pending",
            AiTaskState::Initializing => "initializing",
            AiTaskState::Processing => "processing",
            AiTaskState::Paused => "paused",
            AiTaskState::Stopping => "stopping",
            AiTaskState::Stopped => "stopped",
            AiTaskState::Error => "error",
        }
    }
}

#[async_trait]
impl StateStore for PgStateStore {
    async fn save_stream(&self, info: &StreamInfo) -> Result<()> {
        let state_str = Self::stream_state_to_str(&info.state);

        sqlx::query!(
            r#"
            INSERT INTO streams (stream_id, uri, codec, container, state, node_id, lease_id,
                                 playlist_path, output_dir, last_error, started_at, stopped_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (stream_id) DO UPDATE SET
                uri = EXCLUDED.uri,
                codec = EXCLUDED.codec,
                container = EXCLUDED.container,
                state = EXCLUDED.state,
                node_id = EXCLUDED.node_id,
                lease_id = EXCLUDED.lease_id,
                playlist_path = EXCLUDED.playlist_path,
                output_dir = EXCLUDED.output_dir,
                last_error = EXCLUDED.last_error,
                started_at = EXCLUDED.started_at,
                stopped_at = EXCLUDED.stopped_at
            "#,
            &info.config.id,
            &info.config.uri,
            info.config.codec.as_deref().unwrap_or("h264"),
            info.config.container.as_deref().unwrap_or("ts"),
            state_str,
            info.node_id.as_deref(),
            info.lease_id.as_deref(),
            info.playlist_path.as_deref(),
            info.output_dir.as_deref(),
            info.last_error.as_deref(),
            info.started_at.map(|v| v as i64),
            info.stopped_at.map(|v| v as i64),
        )
        .execute(&self.pool)
        .await
        .context("Failed to save stream")?;

        Ok(())
    }

    async fn get_stream(&self, stream_id: &str) -> Result<Option<StreamInfo>> {
        let row = sqlx::query!(
            r#"
            SELECT stream_id, uri, codec, container, state, node_id, lease_id,
                   playlist_path, output_dir, last_error, started_at, stopped_at
            FROM streams WHERE stream_id = $1
            "#,
            stream_id
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch stream")?;

        Ok(row.map(|r| StreamInfo {
            config: StreamConfig {
                id: r.stream_id,
                camera_id: None,
                uri: r.uri,
                codec: Some(r.codec),
                container: Some(r.container),
            },
            state: Self::parse_stream_state(&r.state),
            lease_id: r.lease_id,
            last_error: r.last_error,
            node_id: r.node_id,
            playlist_path: r.playlist_path,
            output_dir: r.output_dir,
            started_at: r.started_at.map(|v| v as u64),
            stopped_at: r.stopped_at.map(|v| v as u64),
        }))
    }

    async fn list_streams(&self, node_id: Option<&str>) -> Result<Vec<StreamInfo>> {
        let rows = if let Some(nid) = node_id {
            sqlx::query!(
                r#"
                SELECT stream_id, uri, codec, container, state, node_id, lease_id,
                       playlist_path, output_dir, last_error, started_at, stopped_at
                FROM streams WHERE node_id = $1
                ORDER BY created_at DESC
                "#,
                nid
            )
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query!(
                r#"
                SELECT stream_id, uri, codec, container, state, node_id, lease_id,
                       playlist_path, output_dir, last_error, started_at, stopped_at
                FROM streams
                ORDER BY created_at DESC
                "#
            )
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|r| StreamInfo {
                config: StreamConfig {
                    id: r.stream_id,
                    camera_id: None,
                    uri: r.uri,
                    codec: Some(r.codec),
                    container: Some(r.container),
                },
                state: Self::parse_stream_state(&r.state),
                lease_id: r.lease_id,
                last_error: r.last_error,
                node_id: r.node_id,
                playlist_path: r.playlist_path,
                output_dir: r.output_dir,
                started_at: r.started_at.map(|v| v as u64),
                stopped_at: r.stopped_at.map(|v| v as u64),
            })
            .collect())
    }

    async fn delete_stream(&self, stream_id: &str) -> Result<()> {
        sqlx::query!("DELETE FROM streams WHERE stream_id = $1", stream_id)
            .execute(&self.pool)
            .await
            .context("Failed to delete stream")?;
        Ok(())
    }

    async fn update_stream_state(
        &self,
        stream_id: &str,
        state: &str,
        error: Option<&str>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE streams SET state = $1, last_error = $2
            WHERE stream_id = $3
            "#,
            state,
            error,
            stream_id
        )
        .execute(&self.pool)
        .await
        .context("Failed to update stream state")?;
        Ok(())
    }

    async fn save_recording(&self, info: &RecordingInfo) -> Result<()> {
        let state_str = Self::recording_state_to_str(&info.state);
        let format_str = match info.config.format {
            Some(RecordingFormat::Mp4) => "mp4",
            Some(RecordingFormat::Hls) => "hls",
            Some(RecordingFormat::Mkv) => "mkv",
            None => "mp4",
        };

        let (duration, file_size, resolution, codec_name, bitrate, fps) = if let Some(meta) = &info.metadata {
            (
                meta.duration_secs.map(|v| v as f64),
                meta.file_size_bytes.map(|v| v as i64),
                meta.resolution.map(|(w, h)| format!("{}x{}", w, h)),
                meta.video_codec.clone(),
                meta.bitrate_kbps.map(|v| v as i32),
                meta.fps.map(|v| v as f64),
            )
        } else {
            (None, None, None, None, None, None)
        };

        sqlx::query!(
            r#"
            INSERT INTO recordings (recording_id, source_stream_id, source_uri, retention_hours,
                                    format, state, node_id, lease_id, storage_path, last_error,
                                    started_at, stopped_at, duration_secs, file_size_bytes,
                                    resolution, codec_name, bitrate_kbps, fps)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
            ON CONFLICT (recording_id) DO UPDATE SET
                source_stream_id = EXCLUDED.source_stream_id,
                source_uri = EXCLUDED.source_uri,
                retention_hours = EXCLUDED.retention_hours,
                format = EXCLUDED.format,
                state = EXCLUDED.state,
                node_id = EXCLUDED.node_id,
                lease_id = EXCLUDED.lease_id,
                storage_path = EXCLUDED.storage_path,
                last_error = EXCLUDED.last_error,
                started_at = EXCLUDED.started_at,
                stopped_at = EXCLUDED.stopped_at,
                duration_secs = EXCLUDED.duration_secs,
                file_size_bytes = EXCLUDED.file_size_bytes,
                resolution = EXCLUDED.resolution,
                codec_name = EXCLUDED.codec_name,
                bitrate_kbps = EXCLUDED.bitrate_kbps,
                fps = EXCLUDED.fps
            "#,
            &info.config.id,
            info.config.source_stream_id.as_deref(),
            info.config.source_uri.as_deref(),
            info.config.retention_hours.map(|v| v as i32),
            format_str,
            state_str,
            info.node_id.as_deref(),
            info.lease_id.as_deref(),
            info.storage_path.as_deref(),
            info.last_error.as_deref(),
            info.started_at.map(|v| v as i64),
            info.stopped_at.map(|v| v as i64),
            duration,
            file_size,
            resolution.as_deref(),
            codec_name.as_deref(),
            bitrate,
            fps,
        )
        .execute(&self.pool)
        .await
        .context("Failed to save recording")?;

        Ok(())
    }

    async fn get_recording(&self, recording_id: &str) -> Result<Option<RecordingInfo>> {
        let row = sqlx::query!(
            r#"
            SELECT recording_id, source_stream_id, source_uri, retention_hours, format, state,
                   node_id, lease_id, storage_path, last_error, started_at, stopped_at,
                   duration_secs, file_size_bytes, resolution, codec_name, bitrate_kbps, fps
            FROM recordings WHERE recording_id = $1
            "#,
            recording_id
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch recording")?;

        Ok(row.map(|r| {
            let format = match r.format.as_str() {
                "mp4" => RecordingFormat::Mp4,
                "hls" => RecordingFormat::Hls,
                "mkv" => RecordingFormat::Mkv,
                _ => RecordingFormat::Mp4,
            };

            let metadata = if r.duration_secs.is_some()
                || r.file_size_bytes.is_some()
                || r.codec_name.is_some()
            {
                let resolution = r.resolution.as_ref().and_then(|res| {
                    let parts: Vec<&str> = res.split('x').collect();
                    if parts.len() == 2 {
                        let w = parts[0].parse::<u32>().ok()?;
                        let h = parts[1].parse::<u32>().ok()?;
                        Some((w, h))
                    } else {
                        None
                    }
                });

                Some(RecordingMetadata {
                    duration_secs: r.duration_secs.map(|v| v as u64),
                    file_size_bytes: r.file_size_bytes.map(|v| v as u64),
                    video_codec: r.codec_name,
                    audio_codec: None,
                    resolution,
                    bitrate_kbps: r.bitrate_kbps.map(|v| v as u32),
                    fps: r.fps.map(|v| v as f32),
                })
            } else {
                None
            };

            RecordingInfo {
                config: RecordingConfig {
                    id: r.recording_id,
                    source_stream_id: r.source_stream_id,
                    source_uri: r.source_uri,
                    retention_hours: r.retention_hours.map(|v| v as u32),
                    format: Some(format),
                },
                state: Self::parse_recording_state(&r.state),
                lease_id: r.lease_id,
                storage_path: r.storage_path,
                last_error: r.last_error,
                started_at: r.started_at.map(|v| v as u64),
                stopped_at: r.stopped_at.map(|v| v as u64),
                node_id: r.node_id,
                metadata,
            }
        }))
    }

    async fn list_recordings(&self, node_id: Option<&str>) -> Result<Vec<RecordingInfo>> {
        let rows = if let Some(nid) = node_id {
            sqlx::query!(
                r#"
                SELECT recording_id, source_stream_id, source_uri, retention_hours, format, state,
                       node_id, lease_id, storage_path, last_error, started_at, stopped_at,
                       duration_secs, file_size_bytes, resolution, codec_name, bitrate_kbps, fps
                FROM recordings WHERE node_id = $1
                ORDER BY created_at DESC
                "#,
                nid
            )
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query!(
                r#"
                SELECT recording_id, source_stream_id, source_uri, retention_hours, format, state,
                       node_id, lease_id, storage_path, last_error, started_at, stopped_at,
                       duration_secs, file_size_bytes, resolution, codec_name, bitrate_kbps, fps
                FROM recordings
                ORDER BY created_at DESC
                "#
            )
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|r| {
                let format = match r.format.as_str() {
                    "mp4" => RecordingFormat::Mp4,
                    "hls" => RecordingFormat::Hls,
                    "mkv" => RecordingFormat::Mkv,
                    _ => RecordingFormat::Mp4,
                };

                let metadata = if r.duration_secs.is_some()
                    || r.file_size_bytes.is_some()
                    || r.codec_name.is_some()
                {
                    let resolution = r.resolution.as_ref().and_then(|res| {
                        let parts: Vec<&str> = res.split('x').collect();
                        if parts.len() == 2 {
                            let w = parts[0].parse::<u32>().ok()?;
                            let h = parts[1].parse::<u32>().ok()?;
                            Some((w, h))
                        } else {
                            None
                        }
                    });

                    Some(RecordingMetadata {
                        duration_secs: r.duration_secs.map(|v| v as u64),
                        file_size_bytes: r.file_size_bytes.map(|v| v as u64),
                        video_codec: r.codec_name,
                        audio_codec: None,
                        resolution,
                        bitrate_kbps: r.bitrate_kbps.map(|v| v as u32),
                        fps: r.fps.map(|v| v as f32),
                    })
                } else {
                    None
                };

                RecordingInfo {
                    config: RecordingConfig {
                        id: r.recording_id,
                        source_stream_id: r.source_stream_id,
                        source_uri: r.source_uri,
                        retention_hours: r.retention_hours.map(|v| v as u32),
                        format: Some(format),
                    },
                    state: Self::parse_recording_state(&r.state),
                    lease_id: r.lease_id,
                    storage_path: r.storage_path,
                    last_error: r.last_error,
                    started_at: r.started_at.map(|v| v as u64),
                    stopped_at: r.stopped_at.map(|v| v as u64),
                    node_id: r.node_id,
                    metadata,
                }
            })
            .collect())
    }

    async fn delete_recording(&self, recording_id: &str) -> Result<()> {
        sqlx::query!(
            "DELETE FROM recordings WHERE recording_id = $1",
            recording_id
        )
        .execute(&self.pool)
        .await
        .context("Failed to delete recording")?;
        Ok(())
    }

    async fn update_recording_state(
        &self,
        recording_id: &str,
        state: &str,
        error: Option<&str>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE recordings SET state = $1, last_error = $2
            WHERE recording_id = $3
            "#,
            state,
            error,
            recording_id
        )
        .execute(&self.pool)
        .await
        .context("Failed to update recording state")?;
        Ok(())
    }

    async fn save_ai_task(&self, info: &AiTaskInfo) -> Result<()> {
        let state_str = Self::ai_task_state_to_str(&info.state);

        // Serialize config as JSON
        let output_config_json = serde_json::to_value(&info.config.output)?;
        let frame_config_json = serde_json::to_value(&info.config.frame_config)?;

        sqlx::query!(
            r#"
            INSERT INTO ai_tasks (task_id, plugin_type, source_stream_id, source_recording_id,
                                  output_format, output_config, frame_config, state, node_id,
                                  lease_id, last_error, started_at, stopped_at, last_processed_frame,
                                  frames_processed, detections_made)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            ON CONFLICT (task_id) DO UPDATE SET
                plugin_type = EXCLUDED.plugin_type,
                source_stream_id = EXCLUDED.source_stream_id,
                source_recording_id = EXCLUDED.source_recording_id,
                output_format = EXCLUDED.output_format,
                output_config = EXCLUDED.output_config,
                frame_config = EXCLUDED.frame_config,
                state = EXCLUDED.state,
                node_id = EXCLUDED.node_id,
                lease_id = EXCLUDED.lease_id,
                last_error = EXCLUDED.last_error,
                started_at = EXCLUDED.started_at,
                stopped_at = EXCLUDED.stopped_at,
                last_processed_frame = EXCLUDED.last_processed_frame,
                frames_processed = EXCLUDED.frames_processed,
                detections_made = EXCLUDED.detections_made
            "#,
            &info.config.id,
            &info.config.plugin_type,
            info.config.source_stream_id.as_deref(),
            info.config.source_recording_id.as_deref(),
            &info.config.output.output_type,
            output_config_json,
            frame_config_json,
            state_str,
            info.node_id.as_deref(),
            info.lease_id.as_deref(),
            info.last_error.as_deref(),
            info.started_at.map(|v| v as i64),
            info.stopped_at.map(|v| v as i64),
            info.last_processed_frame.map(|v| v as i64),
            info.frames_processed as i64,
            info.detections_made as i64,
        )
        .execute(&self.pool)
        .await
        .context("Failed to save AI task")?;

        Ok(())
    }

    async fn get_ai_task(&self, task_id: &str) -> Result<Option<AiTaskInfo>> {
        let row = sqlx::query!(
            r#"
            SELECT task_id, plugin_type, source_stream_id, source_recording_id,
                   output_format, output_config, frame_config, state, node_id, lease_id, last_error,
                   started_at, stopped_at, last_processed_frame, frames_processed, detections_made
            FROM ai_tasks WHERE task_id = $1
            "#,
            task_id
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch AI task")?;

        Ok(row.map(|r| {
            // Deserialize JSON configs
            let output = serde_json::from_value(r.output_config).unwrap_or_else(|_| {
                common::ai_tasks::AiOutputConfig {
                    output_type: r.output_format,
                    config: serde_json::Value::Null,
                }
            });

            let frame_config = serde_json::from_value(r.frame_config).unwrap_or_default();

            AiTaskInfo {
                config: AiTaskConfig {
                    id: r.task_id,
                    plugin_type: r.plugin_type,
                    source_stream_id: r.source_stream_id,
                    source_recording_id: r.source_recording_id,
                    model_config: serde_json::Value::Null,
                    output,
                    frame_config,
                },
                state: Self::parse_ai_task_state(&r.state),
                node_id: r.node_id,
                lease_id: r.lease_id,
                last_error: r.last_error,
                started_at: r.started_at.map(|v| v as u64),
                stopped_at: r.stopped_at.map(|v| v as u64),
                last_processed_frame: r.last_processed_frame.map(|v| v as u64),
                frames_processed: r.frames_processed as u64,
                detections_made: r.detections_made as u64,
            }
        }))
    }

    async fn list_ai_tasks(&self, node_id: Option<&str>) -> Result<Vec<AiTaskInfo>> {
        let rows = if let Some(nid) = node_id {
            sqlx::query!(
                r#"
                SELECT task_id, plugin_type, source_stream_id, source_recording_id,
                       output_format, output_config, frame_config, state, node_id, lease_id, last_error,
                       started_at, stopped_at, last_processed_frame, frames_processed, detections_made
                FROM ai_tasks WHERE node_id = $1
                ORDER BY created_at DESC
                "#,
                nid
            )
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query!(
                r#"
                SELECT task_id, plugin_type, source_stream_id, source_recording_id,
                       output_format, output_config, frame_config, state, node_id, lease_id, last_error,
                       started_at, stopped_at, last_processed_frame, frames_processed, detections_made
                FROM ai_tasks
                ORDER BY created_at DESC
                "#
            )
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|r| {
                let output = serde_json::from_value(r.output_config).unwrap_or_else(|_| {
                    common::ai_tasks::AiOutputConfig {
                        output_type: r.output_format,
                        config: serde_json::Value::Null,
                    }
                });

                let frame_config = serde_json::from_value(r.frame_config).unwrap_or_default();

                AiTaskInfo {
                    config: AiTaskConfig {
                        id: r.task_id,
                        plugin_type: r.plugin_type,
                        source_stream_id: r.source_stream_id,
                        source_recording_id: r.source_recording_id,
                        model_config: serde_json::Value::Null,
                        output,
                        frame_config,
                    },
                    state: Self::parse_ai_task_state(&r.state),
                    node_id: r.node_id,
                    lease_id: r.lease_id,
                    last_error: r.last_error,
                    started_at: r.started_at.map(|v| v as u64),
                    stopped_at: r.stopped_at.map(|v| v as u64),
                    last_processed_frame: r.last_processed_frame.map(|v| v as u64),
                    frames_processed: r.frames_processed as u64,
                    detections_made: r.detections_made as u64,
                }
            })
            .collect())
    }

    async fn delete_ai_task(&self, task_id: &str) -> Result<()> {
        sqlx::query!("DELETE FROM ai_tasks WHERE task_id = $1", task_id)
            .execute(&self.pool)
            .await
            .context("Failed to delete AI task")?;
        Ok(())
    }

    async fn update_ai_task_state(
        &self,
        task_id: &str,
        state: &str,
        error: Option<&str>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE ai_tasks SET state = $1, last_error = $2
            WHERE task_id = $3
            "#,
            state,
            error,
            task_id
        )
        .execute(&self.pool)
        .await
        .context("Failed to update AI task state")?;
        Ok(())
    }

    async fn update_ai_task_stats(
        &self,
        task_id: &str,
        frames_delta: u64,
        detections_delta: u64,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE ai_tasks
            SET frames_processed = frames_processed + $1,
                detections_made = detections_made + $2,
                last_processed_frame = $3
            WHERE task_id = $4
            "#,
            frames_delta as i64,
            detections_delta as i64,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64,
            task_id
        )
        .execute(&self.pool)
        .await
        .context("Failed to update AI task stats")?;
        Ok(())
    }

    async fn health_check(&self) -> Result<bool> {
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map(|_| true)
            .or(Ok(false))
    }
}

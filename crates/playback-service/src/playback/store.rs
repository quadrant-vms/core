use anyhow::Result;
use common::playback::*;
use sqlx::{PgPool, Row};
use tracing::{error, info};

/// Database store for playback sessions
pub struct PlaybackStore {
    pool: PgPool,
}

impl PlaybackStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Save or update a playback session
    pub async fn save(&self, session: &PlaybackInfo) -> Result<()> {
        let state_str = match session.state {
            PlaybackState::Pending => "pending",
            PlaybackState::Starting => "starting",
            PlaybackState::Playing => "playing",
            PlaybackState::Paused => "paused",
            PlaybackState::Seeking => "seeking",
            PlaybackState::Stopping => "stopping",
            PlaybackState::Stopped => "stopped",
            PlaybackState::Error => "error",
        };

        let source_type_str = match session.config.source_type {
            PlaybackSourceType::Stream => "stream",
            PlaybackSourceType::Recording => "recording",
        };

        let protocol_str = match session.config.protocol {
            PlaybackProtocol::Hls => "hls",
            PlaybackProtocol::Rtsp => "rtsp",
            PlaybackProtocol::WebRtc => "webrtc",
        };

        sqlx::query(
            r#"
            INSERT INTO playback_sessions (
                session_id, source_type, source_id, protocol, state,
                lease_id, node_id, playback_url, current_position_secs,
                duration_secs, start_time_secs, speed, last_error,
                started_at, stopped_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            ON CONFLICT (session_id) DO UPDATE SET
                state = EXCLUDED.state,
                lease_id = EXCLUDED.lease_id,
                node_id = EXCLUDED.node_id,
                playback_url = EXCLUDED.playback_url,
                current_position_secs = EXCLUDED.current_position_secs,
                duration_secs = EXCLUDED.duration_secs,
                start_time_secs = EXCLUDED.start_time_secs,
                speed = EXCLUDED.speed,
                last_error = EXCLUDED.last_error,
                started_at = EXCLUDED.started_at,
                stopped_at = EXCLUDED.stopped_at
            "#,
        )
        .bind(&session.config.session_id)
        .bind(source_type_str)
        .bind(&session.config.source_id)
        .bind(protocol_str)
        .bind(state_str)
        .bind(session.lease_id.as_deref())
        .bind(session.node_id.as_deref())
        .bind(session.playback_url.as_deref())
        .bind(session.current_position_secs)
        .bind(session.duration_secs)
        .bind(session.config.start_time_secs)
        .bind(session.config.speed.unwrap_or(1.0))
        .bind(session.last_error.as_deref())
        .bind(session.started_at.map(|t| t as i64))
        .bind(session.stopped_at.map(|t| t as i64))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get a playback session by ID
    pub async fn get(&self, session_id: &str) -> Result<Option<PlaybackInfo>> {
        let row = sqlx::query(
            r#"
            SELECT session_id, source_type, source_id, protocol, state,
                   lease_id, node_id, playback_url, current_position_secs,
                   duration_secs, start_time_secs, speed, last_error,
                   started_at, stopped_at
            FROM playback_sessions
            WHERE session_id = $1
            "#,
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => Ok(Some(row_to_playback_info(r)?)),
            None => Ok(None),
        }
    }

    /// List all active playback sessions
    pub async fn list_active(&self) -> Result<Vec<PlaybackInfo>> {
        let rows = sqlx::query(
            r#"
            SELECT session_id, source_type, source_id, protocol, state,
                   lease_id, node_id, playback_url, current_position_secs,
                   duration_secs, start_time_secs, speed, last_error,
                   started_at, stopped_at
            FROM playback_sessions
            WHERE state IN ('pending', 'starting', 'playing', 'paused', 'seeking')
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row_to_playback_info(row)?);
        }
        Ok(sessions)
    }

    /// List playback sessions by node_id
    pub async fn list_by_node(&self, node_id: &str) -> Result<Vec<PlaybackInfo>> {
        let rows = sqlx::query(
            r#"
            SELECT session_id, source_type, source_id, protocol, state,
                   lease_id, node_id, playback_url, current_position_secs,
                   duration_secs, start_time_secs, speed, last_error,
                   started_at, stopped_at
            FROM playback_sessions
            WHERE node_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(node_id)
        .fetch_all(&self.pool)
        .await?;

        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row_to_playback_info(row)?);
        }
        Ok(sessions)
    }

    /// Delete a playback session
    pub async fn delete(&self, session_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM playback_sessions WHERE session_id = $1")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

fn row_to_playback_info(row: sqlx::postgres::PgRow) -> Result<PlaybackInfo> {
    let source_type_str: String = row.try_get("source_type")?;
    let source_type = match source_type_str.as_str() {
        "stream" => PlaybackSourceType::Stream,
        "recording" => PlaybackSourceType::Recording,
        _ => PlaybackSourceType::Stream,
    };

    let protocol_str: String = row.try_get("protocol")?;
    let protocol = match protocol_str.as_str() {
        "hls" => PlaybackProtocol::Hls,
        "rtsp" => PlaybackProtocol::Rtsp,
        "webrtc" => PlaybackProtocol::WebRtc,
        _ => PlaybackProtocol::Hls,
    };

    let state_str: String = row.try_get("state")?;
    let state = match state_str.as_str() {
        "pending" => PlaybackState::Pending,
        "starting" => PlaybackState::Starting,
        "playing" => PlaybackState::Playing,
        "paused" => PlaybackState::Paused,
        "seeking" => PlaybackState::Seeking,
        "stopping" => PlaybackState::Stopping,
        "stopped" => PlaybackState::Stopped,
        "error" => PlaybackState::Error,
        _ => PlaybackState::Pending,
    };

    let session_id: String = row.try_get("session_id")?;
    let source_id: String = row.try_get("source_id")?;
    let start_time_secs: Option<f64> = row.try_get("start_time_secs").ok();
    let speed: Option<f64> = row.try_get("speed").ok();

    Ok(PlaybackInfo {
        config: PlaybackConfig {
            session_id,
            source_type,
            source_id,
            protocol,
            start_time_secs,
            speed,
            low_latency: false, // Default to false for database rows
        },
        state,
        lease_id: row.try_get("lease_id").ok(),
        node_id: row.try_get("node_id").ok(),
        playback_url: row.try_get("playback_url").ok(),
        current_position_secs: row.try_get("current_position_secs").ok(),
        duration_secs: row.try_get("duration_secs").ok(),
        last_error: row.try_get("last_error").ok(),
        started_at: row.try_get::<Option<i64>, _>("started_at").ok().flatten().map(|t| t as u64),
        stopped_at: row.try_get::<Option<i64>, _>("stopped_at").ok().flatten().map(|t| t as u64),
    })
}

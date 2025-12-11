use crate::types::*;
use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct DeviceStore {
    pool: PgPool,
}

impl DeviceStore {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPool::connect(database_url)
            .await
            .context("failed to connect to database")?;

        // Run migrations
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .context("failed to run migrations")?;

        Ok(Self { pool })
    }

    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Create a new device
    pub async fn create_device(
        &self,
        tenant_id: &str,
        req: CreateDeviceRequest,
    ) -> Result<Device> {
        let device_id = Uuid::new_v4().to_string();
        let now = Utc::now();

        // Encrypt password if provided (simple placeholder - should use proper encryption)
        let password_encrypted = req.password.map(|p| self.encrypt_password(&p));

        let device = sqlx::query_as!(
            Device,
            r#"
            INSERT INTO devices (
                device_id, tenant_id, name, device_type, manufacturer, model,
                primary_uri, secondary_uri, protocol, username, password_encrypted,
                location, zone, tags, status, health_check_interval_secs,
                auto_start, recording_enabled, ai_enabled, metadata,
                created_at, updated_at,
                capabilities, video_codecs, audio_codecs, resolutions,
                consecutive_failures
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14,
                'provisioning', $15, $16, $17, $18, $19, $20, $20,
                NULL, ARRAY[]::TEXT[], ARRAY[]::TEXT[], ARRAY[]::TEXT[], 0
            )
            RETURNING
                device_id as "device_id!", tenant_id as "tenant_id!", name as "name!",
                device_type as "device_type!: DeviceType",
                manufacturer, model, firmware_version,
                primary_uri as "primary_uri!", secondary_uri,
                protocol as "protocol!: ConnectionProtocol",
                username, password_encrypted,
                location, zone, tags as "tags!",
                status as "status!: DeviceStatus",
                last_seen_at, last_health_check_at,
                health_check_interval_secs as "health_check_interval_secs!", consecutive_failures as "consecutive_failures!",
                capabilities, video_codecs as "video_codecs!", audio_codecs as "audio_codecs!", resolutions as "resolutions!",
                description, notes, metadata,
                auto_start as "auto_start!", recording_enabled as "recording_enabled!", ai_enabled as "ai_enabled!",
                created_at as "created_at!", updated_at as "updated_at!"
            "#,
            device_id,
            tenant_id,
            req.name,
            req.device_type as DeviceType,
            req.manufacturer,
            req.model,
            req.primary_uri,
            req.secondary_uri,
            req.protocol as ConnectionProtocol,
            req.username,
            password_encrypted,
            req.location,
            req.zone,
            &req.tags.unwrap_or_default(),
            req.health_check_interval_secs.unwrap_or(60),
            req.auto_start.unwrap_or(true),
            req.recording_enabled.unwrap_or(false),
            req.ai_enabled.unwrap_or(false),
            req.metadata,
            now,
        )
        .fetch_one(&self.pool)
        .await
        .context("failed to create device")?;

        // Log creation event
        self.log_event(&device_id, "created", None, None, None).await?;

        Ok(device)
    }

    /// Get device by ID
    pub async fn get_device(&self, device_id: &str) -> Result<Option<Device>> {
        let device = sqlx::query_as!(
            Device,
            r#"
            SELECT
                device_id as "device_id!", tenant_id as "tenant_id!", name as "name!",
                device_type as "device_type!: DeviceType",
                manufacturer, model, firmware_version,
                primary_uri as "primary_uri!", secondary_uri,
                protocol as "protocol!: ConnectionProtocol",
                username, password_encrypted,
                location, zone, tags as "tags!",
                status as "status!: DeviceStatus",
                last_seen_at, last_health_check_at,
                health_check_interval_secs as "health_check_interval_secs!", consecutive_failures as "consecutive_failures!",
                capabilities, video_codecs as "video_codecs!", audio_codecs as "audio_codecs!", resolutions as "resolutions!",
                description, notes, metadata,
                auto_start as "auto_start!", recording_enabled as "recording_enabled!", ai_enabled as "ai_enabled!",
                created_at as "created_at!", updated_at as "updated_at!"
            FROM devices
            WHERE device_id = $1
            "#,
            device_id
        )
        .fetch_optional(&self.pool)
        .await
        .context("failed to fetch device")?;

        Ok(device)
    }

    /// List devices with optional filters
    pub async fn list_devices(&self, query: DeviceListQuery) -> Result<Vec<Device>> {
        let mut sql = String::from(
            r#"
            SELECT
                device_id as "device_id!", tenant_id as "tenant_id!", name as "name!",
                device_type as "device_type!: DeviceType",
                manufacturer, model, firmware_version,
                primary_uri as "primary_uri!", secondary_uri,
                protocol as "protocol!: ConnectionProtocol",
                username, password_encrypted,
                location, zone, tags as "tags!",
                status as "status!: DeviceStatus",
                last_seen_at, last_health_check_at,
                health_check_interval_secs as "health_check_interval_secs!", consecutive_failures as "consecutive_failures!",
                capabilities, video_codecs as "video_codecs!", audio_codecs as "audio_codecs!", resolutions as "resolutions!",
                description, notes, metadata,
                auto_start as "auto_start!", recording_enabled as "recording_enabled!", ai_enabled as "ai_enabled!",
                created_at as "created_at!", updated_at as "updated_at!"
            FROM devices
            WHERE 1=1
            "#,
        );

        let mut param_count = 0;
        if query.tenant_id.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND tenant_id = ${}", param_count));
        }
        if query.status.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND status = ${}", param_count));
        }
        if query.device_type.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND device_type = ${}", param_count));
        }
        if query.zone.is_some() {
            param_count += 1;
            sql.push_str(&format!(" AND zone = ${}", param_count));
        }

        sql.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = query.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = query.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let mut query_builder = sqlx::query_as::<_, Device>(&sql);

        if let Some(tenant_id) = &query.tenant_id {
            query_builder = query_builder.bind(tenant_id);
        }
        if let Some(status) = &query.status {
            query_builder = query_builder.bind(status);
        }
        if let Some(device_type) = &query.device_type {
            query_builder = query_builder.bind(device_type);
        }
        if let Some(zone) = &query.zone {
            query_builder = query_builder.bind(zone);
        }

        let devices = query_builder
            .fetch_all(&self.pool)
            .await
            .context("failed to list devices")?;

        Ok(devices)
    }

    /// Update device
    pub async fn update_device(
        &self,
        device_id: &str,
        req: UpdateDeviceRequest,
    ) -> Result<Device> {
        // Get current device for comparison
        let _current = self
            .get_device(device_id)
            .await?
            .context("device not found")?;

        let password_encrypted = req.password.map(|p| self.encrypt_password(&p));

        let device = sqlx::query_as!(
            Device,
            r#"
            UPDATE devices
            SET
                name = COALESCE($2, name),
                manufacturer = COALESCE($3, manufacturer),
                model = COALESCE($4, model),
                firmware_version = COALESCE($5, firmware_version),
                primary_uri = COALESCE($6, primary_uri),
                secondary_uri = COALESCE($7, secondary_uri),
                username = COALESCE($8, username),
                password_encrypted = COALESCE($9, password_encrypted),
                location = COALESCE($10, location),
                zone = COALESCE($11, zone),
                tags = COALESCE($12, tags),
                description = COALESCE($13, description),
                notes = COALESCE($14, notes),
                health_check_interval_secs = COALESCE($15, health_check_interval_secs),
                auto_start = COALESCE($16, auto_start),
                recording_enabled = COALESCE($17, recording_enabled),
                ai_enabled = COALESCE($18, ai_enabled),
                status = COALESCE($19, status),
                metadata = COALESCE($20, metadata),
                updated_at = NOW()
            WHERE device_id = $1
            RETURNING
                device_id as "device_id!", tenant_id as "tenant_id!", name as "name!",
                device_type as "device_type!: DeviceType",
                manufacturer, model, firmware_version,
                primary_uri as "primary_uri!", secondary_uri,
                protocol as "protocol!: ConnectionProtocol",
                username, password_encrypted,
                location, zone, tags as "tags!",
                status as "status!: DeviceStatus",
                last_seen_at, last_health_check_at,
                health_check_interval_secs as "health_check_interval_secs!", consecutive_failures as "consecutive_failures!",
                capabilities, video_codecs as "video_codecs!", audio_codecs as "audio_codecs!", resolutions as "resolutions!",
                description, notes, metadata,
                auto_start as "auto_start!", recording_enabled as "recording_enabled!", ai_enabled as "ai_enabled!",
                created_at as "created_at!", updated_at as "updated_at!"
            "#,
            device_id,
            req.name,
            req.manufacturer,
            req.model,
            req.firmware_version,
            req.primary_uri,
            req.secondary_uri,
            req.username,
            password_encrypted,
            req.location,
            req.zone,
            req.tags.as_deref(),
            req.description,
            req.notes,
            req.health_check_interval_secs,
            req.auto_start,
            req.recording_enabled,
            req.ai_enabled,
            req.status as Option<DeviceStatus>,
            req.metadata,
        )
        .fetch_one(&self.pool)
        .await
        .context("failed to update device")?;

        // Log update event
        self.log_event(device_id, "updated", None, None, None).await?;

        Ok(device)
    }

    /// Delete device
    pub async fn delete_device(&self, device_id: &str) -> Result<()> {
        sqlx::query!("DELETE FROM devices WHERE device_id = $1", device_id)
            .execute(&self.pool)
            .await
            .context("failed to delete device")?;

        Ok(())
    }

    /// Update device health status
    pub async fn update_health_status(
        &self,
        device_id: &str,
        status: DeviceStatus,
        response_time_ms: Option<i32>,
        error_message: Option<String>,
    ) -> Result<()> {
        let now = Utc::now();

        // Update device status
        let result = sqlx::query!(
            r#"
            UPDATE devices
            SET
                status = $2::device_status,
                last_health_check_at = $3,
                last_seen_at = CASE WHEN $2::device_status = 'online' THEN $3 ELSE last_seen_at END,
                consecutive_failures = CASE
                    WHEN $2::device_status IN ('online', 'maintenance') THEN 0
                    ELSE consecutive_failures + 1
                END,
                updated_at = NOW()
            WHERE device_id = $1
            "#,
            device_id,
            status.clone() as DeviceStatus,
            now,
        )
        .execute(&self.pool)
        .await
        .context("failed to update health status")?;

        if result.rows_affected() == 0 {
            anyhow::bail!("device not found");
        }

        // Insert health history record
        sqlx::query!(
            r#"
            INSERT INTO device_health_history
            (device_id, status, response_time_ms, error_message, checked_at)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            device_id,
            status as DeviceStatus,
            response_time_ms,
            error_message,
            now,
        )
        .execute(&self.pool)
        .await
        .context("failed to insert health history")?;

        Ok(())
    }

    /// Get device health history
    pub async fn get_health_history(
        &self,
        device_id: &str,
        limit: i64,
    ) -> Result<Vec<DeviceHealthHistory>> {
        let history = sqlx::query_as!(
            DeviceHealthHistory,
            r#"
            SELECT
                history_id as "history_id!",
                device_id as "device_id!",
                status as "status!: DeviceStatus",
                response_time_ms,
                error_message,
                metadata,
                checked_at as "checked_at!"
            FROM device_health_history
            WHERE device_id = $1
            ORDER BY checked_at DESC
            LIMIT $2
            "#,
            device_id,
            limit,
        )
        .fetch_all(&self.pool)
        .await
        .context("failed to fetch health history")?;

        Ok(history)
    }

    /// Get devices requiring health check
    pub async fn get_devices_needing_health_check(&self) -> Result<Vec<Device>> {
        let devices = sqlx::query_as!(
            Device,
            r#"
            SELECT
                device_id as "device_id!", tenant_id as "tenant_id!", name as "name!",
                device_type as "device_type!: DeviceType",
                manufacturer, model, firmware_version,
                primary_uri as "primary_uri!", secondary_uri,
                protocol as "protocol!: ConnectionProtocol",
                username, password_encrypted,
                location, zone, tags as "tags!",
                status as "status!: DeviceStatus",
                last_seen_at, last_health_check_at,
                health_check_interval_secs as "health_check_interval_secs!", consecutive_failures as "consecutive_failures!",
                capabilities, video_codecs as "video_codecs!", audio_codecs as "audio_codecs!", resolutions as "resolutions!",
                description, notes, metadata,
                auto_start as "auto_start!", recording_enabled as "recording_enabled!", ai_enabled as "ai_enabled!",
                created_at as "created_at!", updated_at as "updated_at!"
            FROM devices
            WHERE
                status NOT IN ('maintenance', 'provisioning')
                AND (
                    last_health_check_at IS NULL
                    OR last_health_check_at < NOW() - (health_check_interval_secs || ' seconds')::INTERVAL
                )
            ORDER BY last_health_check_at ASC NULLS FIRST
            "#
        )
        .fetch_all(&self.pool)
        .await
        .context("failed to fetch devices needing health check")?;

        Ok(devices)
    }

    /// Log device event
    async fn log_event(
        &self,
        device_id: &str,
        event_type: &str,
        old_value: Option<String>,
        new_value: Option<String>,
        user_id: Option<String>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO device_events (device_id, event_type, old_value, new_value, user_id)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            device_id,
            event_type,
            old_value,
            new_value,
            user_id,
        )
        .execute(&self.pool)
        .await
        .context("failed to log event")?;

        Ok(())
    }

    /// Simple password encryption placeholder - should use proper encryption in production
    fn encrypt_password(&self, password: &str) -> String {
        // TODO: Implement proper encryption using a key management system
        // For now, just base64 encode (NOT SECURE - placeholder only)
        use base64::{engine::general_purpose, Engine as _};
        general_purpose::STANDARD.encode(password.as_bytes())
    }

    /// Simple password decryption placeholder
    pub fn decrypt_password(&self, encrypted: &str) -> Result<String> {
        use base64::{engine::general_purpose, Engine as _};
        let bytes = general_purpose::STANDARD
            .decode(encrypted)
            .context("failed to decode password")?;
        Ok(String::from_utf8(bytes).context("invalid utf8 in password")?)
    }

    // PTZ Preset Methods

    /// Create a new PTZ preset
    pub async fn create_ptz_preset(
        &self,
        device_id: &str,
        req: CreatePtzPresetRequest,
        position: PtzPosition,
    ) -> Result<PtzPreset> {
        let preset_id = Uuid::new_v4().to_string();
        let now = Utc::now();

        let preset = sqlx::query_as!(
            PtzPreset,
            r#"
            INSERT INTO ptz_presets (preset_id, device_id, name, position, description, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $6)
            RETURNING preset_id as "preset_id!", device_id as "device_id!", name as "name!", position as "position!: PtzPosition",
                      description, thumbnail_url, created_at as "created_at!", updated_at as "updated_at!"
            "#,
            preset_id,
            device_id,
            req.name,
            serde_json::to_value(&position)?,
            req.description,
            now,
        )
        .fetch_one(&self.pool)
        .await
        .context("failed to create PTZ preset")?;

        Ok(preset)
    }

    /// Get PTZ preset by ID
    pub async fn get_ptz_preset(&self, preset_id: &str) -> Result<Option<PtzPreset>> {
        let preset = sqlx::query_as!(
            PtzPreset,
            r#"
            SELECT preset_id as "preset_id!", device_id as "device_id!", name as "name!", position as "position!: PtzPosition",
                   description, thumbnail_url, created_at as "created_at!", updated_at as "updated_at!"
            FROM ptz_presets
            WHERE preset_id = $1
            "#,
            preset_id
        )
        .fetch_optional(&self.pool)
        .await
        .context("failed to fetch PTZ preset")?;

        Ok(preset)
    }

    /// List PTZ presets for a device
    pub async fn list_ptz_presets(&self, device_id: &str) -> Result<Vec<PtzPreset>> {
        let presets = sqlx::query_as!(
            PtzPreset,
            r#"
            SELECT preset_id as "preset_id!", device_id as "device_id!", name as "name!", position as "position!: PtzPosition",
                   description, thumbnail_url, created_at as "created_at!", updated_at as "updated_at!"
            FROM ptz_presets
            WHERE device_id = $1
            ORDER BY name ASC
            "#,
            device_id
        )
        .fetch_all(&self.pool)
        .await
        .context("failed to list PTZ presets")?;

        Ok(presets)
    }

    /// Update PTZ preset
    pub async fn update_ptz_preset(
        &self,
        preset_id: &str,
        req: UpdatePtzPresetRequest,
    ) -> Result<PtzPreset> {
        let preset = sqlx::query_as!(
            PtzPreset,
            r#"
            UPDATE ptz_presets
            SET
                name = COALESCE($2, name),
                description = COALESCE($3, description),
                position = COALESCE($4, position),
                updated_at = NOW()
            WHERE preset_id = $1
            RETURNING preset_id as "preset_id!", device_id as "device_id!", name as "name!", position as "position!: PtzPosition",
                      description, thumbnail_url, created_at as "created_at!", updated_at as "updated_at!"
            "#,
            preset_id,
            req.name,
            req.description,
            req.position.as_ref().map(|p| serde_json::to_value(p).unwrap()),
        )
        .fetch_one(&self.pool)
        .await
        .context("failed to update PTZ preset")?;

        Ok(preset)
    }

    /// Delete PTZ preset
    pub async fn delete_ptz_preset(&self, preset_id: &str) -> Result<()> {
        sqlx::query!("DELETE FROM ptz_presets WHERE preset_id = $1", preset_id)
            .execute(&self.pool)
            .await
            .context("failed to delete PTZ preset")?;

        Ok(())
    }

    // PTZ Tour Methods

    /// Create a new PTZ tour
    pub async fn create_ptz_tour(
        &self,
        device_id: &str,
        req: CreatePtzTourRequest,
    ) -> Result<PtzTour> {
        let tour_id = Uuid::new_v4().to_string();
        let now = Utc::now();

        let tour = sqlx::query_as!(
            PtzTour,
            r#"
            INSERT INTO ptz_tours (tour_id, device_id, name, description, state, loop_enabled, created_at, updated_at)
            VALUES ($1, $2, $3, $4, 'stopped', $5, $6, $6)
            RETURNING tour_id as "tour_id!", device_id as "device_id!", name as "name!", description,
                      state as "state!: TourState",
                      loop_enabled as "loop_enabled!", created_at as "created_at!", updated_at as "updated_at!"
            "#,
            tour_id,
            device_id,
            req.name,
            req.description,
            req.loop_enabled,
            now,
        )
        .fetch_one(&self.pool)
        .await
        .context("failed to create PTZ tour")?;

        Ok(tour)
    }

    /// Get PTZ tour by ID
    pub async fn get_ptz_tour(&self, tour_id: &str) -> Result<Option<PtzTour>> {
        let tour = sqlx::query_as!(
            PtzTour,
            r#"
            SELECT tour_id as "tour_id!", device_id as "device_id!", name as "name!", description,
                   state as "state!: TourState",
                   loop_enabled as "loop_enabled!", created_at as "created_at!", updated_at as "updated_at!"
            FROM ptz_tours
            WHERE tour_id = $1
            "#,
            tour_id
        )
        .fetch_optional(&self.pool)
        .await
        .context("failed to fetch PTZ tour")?;

        Ok(tour)
    }

    /// List PTZ tours for a device
    pub async fn list_ptz_tours(&self, device_id: &str) -> Result<Vec<PtzTour>> {
        let tours = sqlx::query_as!(
            PtzTour,
            r#"
            SELECT tour_id as "tour_id!", device_id as "device_id!", name as "name!", description,
                   state as "state!: TourState",
                   loop_enabled as "loop_enabled!", created_at as "created_at!", updated_at as "updated_at!"
            FROM ptz_tours
            WHERE device_id = $1
            ORDER BY name ASC
            "#,
            device_id
        )
        .fetch_all(&self.pool)
        .await
        .context("failed to list PTZ tours")?;

        Ok(tours)
    }

    /// Update PTZ tour
    pub async fn update_ptz_tour(
        &self,
        tour_id: &str,
        req: UpdatePtzTourRequest,
    ) -> Result<PtzTour> {
        let tour = sqlx::query_as!(
            PtzTour,
            r#"
            UPDATE ptz_tours
            SET
                name = COALESCE($2, name),
                description = COALESCE($3, description),
                loop_enabled = COALESCE($4, loop_enabled),
                updated_at = NOW()
            WHERE tour_id = $1
            RETURNING tour_id as "tour_id!", device_id as "device_id!", name as "name!", description,
                      state as "state!: TourState",
                      loop_enabled as "loop_enabled!", created_at as "created_at!", updated_at as "updated_at!"
            "#,
            tour_id,
            req.name,
            req.description,
            req.loop_enabled,
        )
        .fetch_one(&self.pool)
        .await
        .context("failed to update PTZ tour")?;

        Ok(tour)
    }

    /// Update PTZ tour state
    pub async fn update_ptz_tour_state(&self, tour_id: &str, state: TourState) -> Result<()> {
        sqlx::query!(
            "UPDATE ptz_tours SET state = $2, updated_at = NOW() WHERE tour_id = $1",
            tour_id,
            state as TourState,
        )
        .execute(&self.pool)
        .await
        .context("failed to update PTZ tour state")?;

        Ok(())
    }

    /// Delete PTZ tour
    pub async fn delete_ptz_tour(&self, tour_id: &str) -> Result<()> {
        sqlx::query!("DELETE FROM ptz_tours WHERE tour_id = $1", tour_id)
            .execute(&self.pool)
            .await
            .context("failed to delete PTZ tour")?;

        Ok(())
    }

    // PTZ Tour Step Methods

    /// Add step to PTZ tour
    pub async fn add_ptz_tour_step(
        &self,
        tour_id: &str,
        req: AddTourStepRequest,
    ) -> Result<PtzTourStep> {
        let step_id = Uuid::new_v4().to_string();

        // Get the next sequence order
        let next_order = sqlx::query!(
            "SELECT COALESCE(MAX(sequence_order), -1) + 1 as next_order FROM ptz_tour_steps WHERE tour_id = $1",
            tour_id
        )
        .fetch_one(&self.pool)
        .await
        .context("failed to get next sequence order")?
        .next_order
        .unwrap_or(0);

        let step = sqlx::query_as!(
            PtzTourStep,
            r#"
            INSERT INTO ptz_tour_steps (step_id, tour_id, sequence_order, preset_id, position, dwell_time_ms, speed)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING step_id as "step_id!", tour_id as "tour_id!", sequence_order as "sequence_order!", preset_id,
                      position as "position: PtzPosition",
                      dwell_time_ms as "dwell_time_ms!", speed as "speed!"
            "#,
            step_id,
            tour_id,
            next_order,
            req.preset_id,
            req.position.as_ref().map(|p| serde_json::to_value(p).unwrap()),
            req.dwell_time_ms,
            req.speed,
        )
        .fetch_one(&self.pool)
        .await
        .context("failed to add PTZ tour step")?;

        Ok(step)
    }

    /// Get steps for a tour
    pub async fn get_ptz_tour_steps(&self, tour_id: &str) -> Result<Vec<PtzTourStep>> {
        let steps = sqlx::query_as!(
            PtzTourStep,
            r#"
            SELECT step_id as "step_id!", tour_id as "tour_id!", sequence_order as "sequence_order!", preset_id,
                   position as "position: PtzPosition",
                   dwell_time_ms as "dwell_time_ms!", speed as "speed!"
            FROM ptz_tour_steps
            WHERE tour_id = $1
            ORDER BY sequence_order ASC
            "#,
            tour_id
        )
        .fetch_all(&self.pool)
        .await
        .context("failed to get PTZ tour steps")?;

        Ok(steps)
    }

    /// Delete PTZ tour step
    pub async fn delete_ptz_tour_step(&self, step_id: &str) -> Result<()> {
        sqlx::query!("DELETE FROM ptz_tour_steps WHERE step_id = $1", step_id)
            .execute(&self.pool)
            .await
            .context("failed to delete PTZ tour step")?;

        Ok(())
    }

    /// Reorder PTZ tour steps
    pub async fn reorder_ptz_tour_steps(&self, tour_id: &str, step_ids: Vec<String>) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        for (idx, step_id) in step_ids.iter().enumerate() {
            sqlx::query!(
                "UPDATE ptz_tour_steps SET sequence_order = $1 WHERE step_id = $2 AND tour_id = $3",
                idx as i32,
                step_id,
                tour_id
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_password_encryption() {
        let store = DeviceStore::from_pool(PgPool::connect("").await.unwrap());
        let password = "test123";
        let encrypted = store.encrypt_password(password);
        let decrypted = store.decrypt_password(&encrypted).unwrap();
        assert_eq!(password, decrypted);
    }
}

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

        // Run migrations (commented out - run migrations manually)
        // sqlx::migrate!()
        //     .run(&pool)
        //     .await
        //     .context("failed to run migrations")?;

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

    /// Retrieve device events
    pub async fn get_device_events(
        &self,
        device_id: &str,
        event_type: Option<String>,
        start_time: Option<chrono::DateTime<chrono::Utc>>,
        end_time: Option<chrono::DateTime<chrono::Utc>>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<crate::types::DeviceEvent>> {
        let limit = limit.unwrap_or(100).min(1000); // Max 1000 events
        let offset = offset.unwrap_or(0);

        let mut query = String::from(
            r#"
            SELECT event_id, device_id, event_type, old_value, new_value, user_id, metadata, created_at
            FROM device_events
            WHERE device_id = $1
            "#,
        );

        let mut params_count = 1;

        // Add event_type filter
        if event_type.is_some() {
            params_count += 1;
            query.push_str(&format!(" AND event_type = ${}", params_count));
        }

        // Add start_time filter
        if start_time.is_some() {
            params_count += 1;
            query.push_str(&format!(" AND created_at >= ${}", params_count));
        }

        // Add end_time filter
        if end_time.is_some() {
            params_count += 1;
            query.push_str(&format!(" AND created_at <= ${}", params_count));
        }

        // Add ordering, limit, and offset
        params_count += 1;
        let limit_param = params_count;
        params_count += 1;
        let offset_param = params_count;

        query.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            limit_param, offset_param
        ));

        // Build query with parameters
        let mut sqlx_query = sqlx::query_as::<_, crate::types::DeviceEvent>(&query)
            .bind(device_id);

        if let Some(et) = event_type {
            sqlx_query = sqlx_query.bind(et);
        }

        if let Some(st) = start_time {
            sqlx_query = sqlx_query.bind(st);
        }

        if let Some(et) = end_time {
            sqlx_query = sqlx_query.bind(et);
        }

        sqlx_query = sqlx_query.bind(limit).bind(offset);

        let events = sqlx_query
            .fetch_all(&self.pool)
            .await
            .context("failed to retrieve device events")?;

        Ok(events)
    }

    /// Encrypt password using AES-256-GCM with Argon2 key derivation
    ///
    /// Format: {version}${salt}${nonce}${ciphertext}${tag}
    /// - version: encryption version (currently "v1")
    /// - salt: base64-encoded random salt for key derivation (32 bytes)
    /// - nonce: base64-encoded random nonce for AES-GCM (12 bytes)
    /// - ciphertext: base64-encoded encrypted password
    /// - tag: included in ciphertext by AES-GCM
    ///
    /// This implementation uses:
    /// - Argon2id for key derivation from environment master key
    /// - AES-256-GCM for authenticated encryption
    /// - Random salt and nonce per encryption operation
    ///
    /// Future: Can be extended to fetch master key from external KMS (AWS KMS, Vault, etc.)
    fn encrypt_password(&self, password: &str) -> String {
        use aes_gcm::{
            aead::{Aead, KeyInit},
            Aes256Gcm,
        };
        use argon2::{Algorithm, Argon2, Params, Version};
        use base64::{engine::general_purpose, Engine as _};
        use rand::Rng;

        // Get master key from environment (in production, fetch from KMS)
        let master_key = std::env::var("DEVICE_CREDENTIAL_MASTER_KEY")
            .unwrap_or_else(|_| "INSECURE_DEFAULT_KEY_CHANGE_IN_PRODUCTION".to_string());

        // Generate random salt for key derivation
        let salt: [u8; 32] = rand::thread_rng().gen();

        // Derive encryption key using Argon2id
        let argon2 = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(19456, 2, 1, Some(32)).expect("BUG: invalid Argon2 params"),
        );

        let mut derived_key = [0u8; 32];
        argon2
            .hash_password_into(master_key.as_bytes(), &salt, &mut derived_key)
            .expect("BUG: Argon2 key derivation failed");

        // Create cipher
        let cipher = Aes256Gcm::new(&derived_key.into());

        // Generate random nonce
        let nonce_bytes: [u8; 12] = rand::thread_rng().gen();
        let nonce = &nonce_bytes.into();

        // Encrypt password
        let ciphertext = cipher
            .encrypt(nonce, password.as_bytes())
            .expect("BUG: AES-GCM encryption failed");

        // Encode components to base64
        let salt_b64 = general_purpose::STANDARD.encode(salt);
        let nonce_b64 = general_purpose::STANDARD.encode(nonce_bytes);
        let ciphertext_b64 = general_purpose::STANDARD.encode(ciphertext);

        // Return formatted encrypted string
        format!("v1${}${}${}", salt_b64, nonce_b64, ciphertext_b64)
    }

    /// Decrypt password encrypted with encrypt_password
    pub fn decrypt_password(&self, encrypted: &str) -> Result<String> {
        use aes_gcm::{
            aead::{Aead, KeyInit},
            Aes256Gcm,
        };
        use argon2::{Algorithm, Argon2, Params, Version};
        use base64::{engine::general_purpose, Engine as _};

        // Parse encrypted format: v1$salt$nonce$ciphertext
        let parts: Vec<&str> = encrypted.split('$').collect();

        if parts.len() != 4 || parts[0] != "v1" {
            anyhow::bail!("invalid encrypted password format");
        }

        let salt_b64 = parts[1];
        let nonce_b64 = parts[2];
        let ciphertext_b64 = parts[3];

        // Decode from base64
        let salt = general_purpose::STANDARD
            .decode(salt_b64)
            .context("failed to decode salt")?;
        let nonce_bytes = general_purpose::STANDARD
            .decode(nonce_b64)
            .context("failed to decode nonce")?;
        let ciphertext = general_purpose::STANDARD
            .decode(ciphertext_b64)
            .context("failed to decode ciphertext")?;

        // Get master key from environment (in production, fetch from KMS)
        let master_key = std::env::var("DEVICE_CREDENTIAL_MASTER_KEY")
            .unwrap_or_else(|_| "INSECURE_DEFAULT_KEY_CHANGE_IN_PRODUCTION".to_string());

        // Derive decryption key using Argon2id
        let params = Params::new(19456, 2, 1, Some(32))
            .map_err(|e| anyhow::anyhow!("invalid Argon2 params: {}", e))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let mut derived_key = [0u8; 32];
        argon2
            .hash_password_into(master_key.as_bytes(), &salt, &mut derived_key)
            .map_err(|e| anyhow::anyhow!("Argon2 key derivation failed: {}", e))?;

        // Create cipher
        let cipher = Aes256Gcm::new(&derived_key.into());

        // Decrypt password
        if nonce_bytes.len() != 12 {
            anyhow::bail!("invalid nonce length: expected 12 bytes, got {}", nonce_bytes.len());
        }
        let nonce: [u8; 12] = nonce_bytes.as_slice().try_into()
            .context("failed to convert nonce to fixed-size array")?;
        let plaintext = cipher
            .decrypt(&nonce.into(), ciphertext.as_ref())
            .map_err(|e| anyhow::anyhow!("AES-GCM decryption failed: {}", e))?;

        Ok(String::from_utf8(plaintext).context("invalid utf8 in password")?)
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

    /// Save discovery scan to database
    pub async fn save_discovery_scan(&self, scan: &crate::discovery::DiscoveryScan) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO discovery_scans (scan_id, started_at, completed_at, devices_found, status, error_message)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (scan_id) DO UPDATE SET
                completed_at = EXCLUDED.completed_at,
                devices_found = EXCLUDED.devices_found,
                status = EXCLUDED.status,
                error_message = EXCLUDED.error_message
            "#,
            scan.scan_id,
            scan.started_at,
            scan.completed_at,
            scan.devices_found as i32,
            format!("{:?}", scan.status).to_lowercase(),
            scan.error_message
        )
        .execute(&self.pool)
        .await
        .context("failed to save discovery scan")?;

        Ok(())
    }

    /// Get discovery scan by ID
    pub async fn get_discovery_scan(&self, scan_id: &str) -> Result<Option<crate::discovery::DiscoveryScan>> {
        let row = sqlx::query!(
            r#"
            SELECT scan_id, started_at, completed_at, devices_found, status, error_message
            FROM discovery_scans
            WHERE scan_id = $1
            "#,
            scan_id
        )
        .fetch_optional(&self.pool)
        .await
        .context("failed to get discovery scan")?;

        Ok(row.map(|r| crate::discovery::DiscoveryScan {
            scan_id: r.scan_id,
            started_at: r.started_at,
            completed_at: r.completed_at,
            devices_found: r.devices_found as usize,
            status: match r.status.as_str() {
                "running" => crate::discovery::DiscoveryScanStatus::Running,
                "completed" => crate::discovery::DiscoveryScanStatus::Completed,
                "failed" => crate::discovery::DiscoveryScanStatus::Failed,
                "cancelled" => crate::discovery::DiscoveryScanStatus::Cancelled,
                _ => crate::discovery::DiscoveryScanStatus::Failed,
            },
            error_message: r.error_message,
        }))
    }

    /// List all discovery scans
    pub async fn list_discovery_scans(&self, limit: Option<i64>) -> Result<Vec<crate::discovery::DiscoveryScan>> {
        let limit = limit.unwrap_or(100);

        let rows = sqlx::query!(
            r#"
            SELECT scan_id, started_at, completed_at, devices_found, status, error_message
            FROM discovery_scans
            ORDER BY started_at DESC
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(&self.pool)
        .await
        .context("failed to list discovery scans")?;

        Ok(rows
            .into_iter()
            .map(|r| crate::discovery::DiscoveryScan {
                scan_id: r.scan_id,
                started_at: r.started_at,
                completed_at: r.completed_at,
                devices_found: r.devices_found as usize,
                status: match r.status.as_str() {
                    "running" => crate::discovery::DiscoveryScanStatus::Running,
                    "completed" => crate::discovery::DiscoveryScanStatus::Completed,
                    "failed" => crate::discovery::DiscoveryScanStatus::Failed,
                    "cancelled" => crate::discovery::DiscoveryScanStatus::Cancelled,
                    _ => crate::discovery::DiscoveryScanStatus::Failed,
                },
                error_message: r.error_message,
            })
            .collect())
    }

    /// Save discovered device to database
    pub async fn save_discovered_device(
        &self,
        scan_id: &str,
        device: &crate::discovery::DiscoveredDevice,
    ) -> Result<String> {
        let discovery_id = Uuid::new_v4().to_string();

        sqlx::query!(
            r#"
            INSERT INTO discovered_devices (
                discovery_id, scan_id, device_service_url, scopes, types, xaddrs,
                manufacturer, model, hardware_id, name, location, discovered_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            "#,
            discovery_id,
            scan_id,
            device.device_service_url,
            &device.scopes,
            &device.types,
            &device.xaddrs,
            device.manufacturer,
            device.model,
            device.hardware_id,
            device.name,
            device.location,
            device.discovered_at
        )
        .execute(&self.pool)
        .await
        .context("failed to save discovered device")?;

        Ok(discovery_id)
    }

    /// List discovered devices for a scan
    pub async fn list_discovered_devices(
        &self,
        scan_id: &str,
    ) -> Result<Vec<crate::discovery::DiscoveredDevice>> {
        let rows = sqlx::query!(
            r#"
            SELECT device_service_url, scopes, types, xaddrs, manufacturer, model,
                   hardware_id, name, location, discovered_at
            FROM discovered_devices
            WHERE scan_id = $1
            ORDER BY discovered_at DESC
            "#,
            scan_id
        )
        .fetch_all(&self.pool)
        .await
        .context("failed to list discovered devices")?;

        Ok(rows
            .into_iter()
            .map(|r| crate::discovery::DiscoveredDevice {
                device_service_url: r.device_service_url,
                scopes: r.scopes,
                types: r.types,
                xaddrs: r.xaddrs,
                manufacturer: r.manufacturer,
                model: r.model,
                hardware_id: r.hardware_id,
                name: r.name,
                location: r.location,
                discovered_at: r.discovered_at,
            })
            .collect())
    }

    /// Mark discovered device as imported
    pub async fn mark_discovered_device_imported(
        &self,
        discovery_id: &str,
        device_id: &str,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE discovered_devices
            SET imported = TRUE, imported_device_id = $1, imported_at = NOW()
            WHERE discovery_id = $2
            "#,
            device_id,
            discovery_id
        )
        .execute(&self.pool)
        .await
        .context("failed to mark device as imported")?;

        Ok(())
    }

    /// Delete old discovery scans and their discovered devices
    pub async fn cleanup_old_discovery_scans(&self, days: i32) -> Result<u64> {
        let result = sqlx::query!(
            r#"
            DELETE FROM discovery_scans
            WHERE started_at < NOW() - INTERVAL '1 day' * $1
            "#,
            days as f64
        )
        .execute(&self.pool)
        .await
        .context("failed to cleanup old discovery scans")?;

        Ok(result.rows_affected())
    }

    // Device Configuration Management

    /// Save a new device configuration
    pub async fn save_device_configuration(
        &self,
        config: DeviceConfiguration,
    ) -> Result<DeviceConfiguration> {
        let saved = sqlx::query_as!(
            DeviceConfiguration,
            r#"
            INSERT INTO device_configurations (
                config_id, device_id, requested_config, applied_config,
                status, error_message, applied_by, created_at, applied_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING
                config_id as "config_id!",
                device_id as "device_id!",
                requested_config as "requested_config!",
                applied_config,
                status as "status!: ConfigurationStatus",
                error_message,
                applied_by,
                created_at as "created_at!",
                applied_at
            "#,
            config.config_id,
            config.device_id,
            config.requested_config,
            config.applied_config,
            config.status as ConfigurationStatus,
            config.error_message,
            config.applied_by,
            config.created_at,
            config.applied_at
        )
        .fetch_one(&self.pool)
        .await
        .context("failed to save device configuration")?;

        Ok(saved)
    }

    /// Get a specific configuration by ID
    pub async fn get_device_configuration(&self, config_id: &str) -> Result<DeviceConfiguration> {
        let config = sqlx::query_as!(
            DeviceConfiguration,
            r#"
            SELECT
                config_id, device_id, requested_config, applied_config,
                status as "status!: ConfigurationStatus",
                error_message, applied_by, created_at, applied_at
            FROM device_configurations
            WHERE config_id = $1
            "#,
            config_id
        )
        .fetch_one(&self.pool)
        .await
        .context("failed to get device configuration")?;

        Ok(config)
    }

    /// Update configuration status and applied config
    pub async fn update_device_configuration_status(
        &self,
        config_id: &str,
        status: ConfigurationStatus,
        applied_config: Option<serde_json::Value>,
        error_message: Option<String>,
    ) -> Result<DeviceConfiguration> {
        let updated = sqlx::query_as!(
            DeviceConfiguration,
            r#"
            UPDATE device_configurations
            SET status = $2,
                applied_config = $3,
                error_message = $4
            WHERE config_id = $1
            RETURNING
                config_id, device_id, requested_config, applied_config,
                status as "status!: ConfigurationStatus",
                error_message, applied_by, created_at, applied_at
            "#,
            config_id,
            status as ConfigurationStatus,
            applied_config,
            error_message
        )
        .fetch_one(&self.pool)
        .await
        .context("failed to update device configuration status")?;

        Ok(updated)
    }

    /// Get latest configuration for a device
    pub async fn get_latest_device_configuration(
        &self,
        device_id: &str,
    ) -> Result<Option<DeviceConfiguration>> {
        let config = sqlx::query_as!(
            DeviceConfiguration,
            r#"
            SELECT
                config_id, device_id, requested_config, applied_config,
                status as "status!: ConfigurationStatus",
                error_message, applied_by, created_at, applied_at
            FROM device_configurations
            WHERE device_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            device_id
        )
        .fetch_optional(&self.pool)
        .await
        .context("failed to get latest device configuration")?;

        Ok(config)
    }

    /// List configuration history for a device
    pub async fn list_device_configuration_history(
        &self,
        query: ConfigurationHistoryQuery,
    ) -> Result<Vec<DeviceConfiguration>> {
        let limit = query.limit.unwrap_or(50);
        let offset = query.offset.unwrap_or(0);

        let configs = if let Some(status) = query.status {
            sqlx::query_as!(
                DeviceConfiguration,
                r#"
                SELECT
                    config_id, device_id, requested_config, applied_config,
                    status as "status!: ConfigurationStatus",
                    error_message, applied_by, created_at, applied_at
                FROM device_configurations
                WHERE device_id = $1 AND status = $2
                ORDER BY created_at DESC
                LIMIT $3 OFFSET $4
                "#,
                query.device_id,
                status as ConfigurationStatus,
                limit,
                offset
            )
            .fetch_all(&self.pool)
            .await
            .context("failed to list device configuration history")?
        } else {
            sqlx::query_as!(
                DeviceConfiguration,
                r#"
                SELECT
                    config_id, device_id, requested_config, applied_config,
                    status as "status!: ConfigurationStatus",
                    error_message, applied_by, created_at, applied_at
                FROM device_configurations
                WHERE device_id = $1
                ORDER BY created_at DESC
                LIMIT $2 OFFSET $3
                "#,
                query.device_id,
                limit,
                offset
            )
            .fetch_all(&self.pool)
            .await
            .context("failed to list device configuration history")?
        };

        Ok(configs)
    }

    /// Delete old configuration history
    pub async fn cleanup_old_configurations(&self, device_id: &str, keep_count: i64) -> Result<u64> {
        let result = sqlx::query!(
            r#"
            DELETE FROM device_configurations
            WHERE config_id IN (
                SELECT config_id
                FROM device_configurations
                WHERE device_id = $1
                ORDER BY created_at DESC
                OFFSET $2
            )
            "#,
            device_id,
            keep_count
        )
        .execute(&self.pool)
        .await
        .context("failed to cleanup old configurations")?;

        Ok(result.rows_affected())
    }

    // ============================================================================
    // Firmware Update Operations
    // ============================================================================

    /// Create a new firmware update record
    pub async fn create_firmware_update(
        &self,
        device_id: &str,
        firmware_version: &str,
        firmware_file_path: &str,
        firmware_file_size: i64,
        firmware_checksum: &str,
        previous_firmware_version: Option<&str>,
        manufacturer: Option<&str>,
        model: Option<&str>,
        release_notes: Option<&str>,
        initiated_by: Option<&str>,
        max_retries: i32,
    ) -> Result<FirmwareUpdate> {
        let update_id = Uuid::new_v4().to_string();
        let now = Utc::now();

        let update = sqlx::query_as!(
            FirmwareUpdate,
            r#"
            INSERT INTO firmware_updates (
                update_id, device_id, firmware_version, firmware_file_path,
                firmware_file_size, firmware_checksum, status, progress_percent,
                retry_count, max_retries, previous_firmware_version,
                manufacturer, model, release_notes, initiated_by, initiated_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, 'pending', 0, 0, $7, $8, $9, $10, $11, $12, $13, $13)
            RETURNING
                update_id, device_id, firmware_version, firmware_file_path,
                firmware_file_size, firmware_checksum,
                status as "status!: FirmwareUpdateStatus",
                progress_percent, error_message, retry_count, max_retries,
                previous_firmware_version, manufacturer, model, release_notes, release_date,
                can_rollback, rollback_data,
                initiated_by, initiated_at, started_at, completed_at, updated_at
            "#,
            update_id,
            device_id,
            firmware_version,
            firmware_file_path,
            firmware_file_size,
            firmware_checksum,
            max_retries,
            previous_firmware_version,
            manufacturer,
            model,
            release_notes,
            initiated_by,
            now
        )
        .fetch_one(&self.pool)
        .await
        .context("failed to create firmware update")?;

        Ok(update)
    }

    /// Update firmware update status
    pub async fn update_firmware_status(
        &self,
        update_id: &str,
        status: FirmwareUpdateStatus,
        progress_percent: i32,
        error_message: Option<&str>,
    ) -> Result<()> {
        let status_str = status.to_string();
        let now = Utc::now();

        let mut started_at: Option<chrono::DateTime<Utc>> = None;
        let mut completed_at: Option<chrono::DateTime<Utc>> = None;

        // Set started_at if moving to installing status
        if status == FirmwareUpdateStatus::Installing {
            started_at = Some(now);
        }

        // Set completed_at if moving to terminal status
        if matches!(status, FirmwareUpdateStatus::Completed | FirmwareUpdateStatus::Failed | FirmwareUpdateStatus::Cancelled) {
            completed_at = Some(now);
        }

        if let Some(started_at_val) = started_at {
            sqlx::query!(
                r#"
                UPDATE firmware_updates
                SET status = $2, progress_percent = $3, error_message = $4, started_at = $5, updated_at = CURRENT_TIMESTAMP
                WHERE update_id = $1
                "#,
                update_id,
                status_str,
                progress_percent,
                error_message,
                started_at_val
            )
            .execute(&self.pool)
            .await
            .context("failed to update firmware status")?;
        } else if let Some(completed_at_val) = completed_at {
            sqlx::query!(
                r#"
                UPDATE firmware_updates
                SET status = $2, progress_percent = $3, error_message = $4, completed_at = $5, updated_at = CURRENT_TIMESTAMP
                WHERE update_id = $1
                "#,
                update_id,
                status_str,
                progress_percent,
                error_message,
                completed_at_val
            )
            .execute(&self.pool)
            .await
            .context("failed to update firmware status")?;
        } else {
            sqlx::query!(
                r#"
                UPDATE firmware_updates
                SET status = $2, progress_percent = $3, error_message = $4, updated_at = CURRENT_TIMESTAMP
                WHERE update_id = $1
                "#,
                update_id,
                status_str,
                progress_percent,
                error_message
            )
            .execute(&self.pool)
            .await
            .context("failed to update firmware status")?;
        }

        Ok(())
    }

    /// Get firmware update by ID
    pub async fn get_firmware_update(&self, update_id: &str) -> Result<FirmwareUpdate> {
        let update = sqlx::query_as!(
            FirmwareUpdate,
            r#"
            SELECT
                update_id, device_id, firmware_version, firmware_file_path,
                firmware_file_size, firmware_checksum,
                status as "status!: FirmwareUpdateStatus",
                progress_percent, error_message, retry_count, max_retries,
                previous_firmware_version, manufacturer, model, release_notes, release_date,
                can_rollback, rollback_data,
                initiated_by, initiated_at, started_at, completed_at, updated_at
            FROM firmware_updates
            WHERE update_id = $1
            "#,
            update_id
        )
        .fetch_one(&self.pool)
        .await
        .context("failed to get firmware update")?;

        Ok(update)
    }

    /// List firmware updates with optional filters
    pub async fn list_firmware_updates(&self, query: &FirmwareUpdateListQuery) -> Result<Vec<FirmwareUpdate>> {
        let limit = query.limit.unwrap_or(100);
        let offset = query.offset.unwrap_or(0);

        let status_str = query.status.as_ref().map(|s| s.to_string());

        let updates = sqlx::query_as!(
            FirmwareUpdate,
            r#"
            SELECT
                update_id, device_id, firmware_version, firmware_file_path,
                firmware_file_size, firmware_checksum,
                status as "status!: FirmwareUpdateStatus",
                progress_percent, error_message, retry_count, max_retries,
                previous_firmware_version, manufacturer, model, release_notes, release_date,
                can_rollback, rollback_data,
                initiated_by, initiated_at, started_at, completed_at, updated_at
            FROM firmware_updates
            WHERE ($1::TEXT IS NULL OR device_id = $1)
              AND ($2::TEXT IS NULL OR status = $2)
            ORDER BY initiated_at DESC
            LIMIT $3 OFFSET $4
            "#,
            query.device_id.as_deref(),
            status_str.as_deref(),
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await
        .context("failed to list firmware updates")?;

        Ok(updates)
    }

    /// Get firmware update history
    pub async fn get_firmware_update_history(&self, update_id: &str) -> Result<Vec<FirmwareUpdateHistory>> {
        let history = sqlx::query_as!(
            FirmwareUpdateHistory,
            r#"
            SELECT history_id, update_id, status, progress_percent, message, metadata, recorded_at
            FROM firmware_update_history
            WHERE update_id = $1
            ORDER BY recorded_at ASC
            "#,
            update_id
        )
        .fetch_all(&self.pool)
        .await
        .context("failed to get firmware update history")?;

        Ok(history)
    }

    /// Increment retry count for firmware update
    pub async fn increment_firmware_retry_count(&self, update_id: &str) -> Result<i32> {
        let result = sqlx::query!(
            r#"
            UPDATE firmware_updates
            SET retry_count = retry_count + 1, updated_at = CURRENT_TIMESTAMP
            WHERE update_id = $1
            RETURNING retry_count
            "#,
            update_id
        )
        .fetch_one(&self.pool)
        .await
        .context("failed to increment retry count")?;

        Ok(result.retry_count)
    }

    /// Cancel firmware update
    pub async fn cancel_firmware_update(&self, update_id: &str) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE firmware_updates
            SET status = 'cancelled', updated_at = CURRENT_TIMESTAMP
            WHERE update_id = $1 AND status NOT IN ('completed', 'failed', 'cancelled')
            "#,
            update_id
        )
        .execute(&self.pool)
        .await
        .context("failed to cancel firmware update")?;

        Ok(())
    }

    // ============================================================================
    // Firmware File Catalog Operations
    // ============================================================================

    /// Upload firmware file to catalog
    pub async fn create_firmware_file(
        &self,
        manufacturer: &str,
        model: &str,
        firmware_version: &str,
        file_path: &str,
        file_size: i64,
        checksum: &str,
        release_notes: Option<&str>,
        release_date: Option<chrono::DateTime<Utc>>,
        min_device_version: Option<&str>,
        compatible_models: Option<&[String]>,
        uploaded_by: Option<&str>,
    ) -> Result<FirmwareFile> {
        let file_id = Uuid::new_v4().to_string();
        let now = Utc::now();

        let compatible_models_arr = compatible_models.unwrap_or(&[]);

        let file = sqlx::query_as!(
            FirmwareFile,
            r#"
            INSERT INTO firmware_files (
                file_id, manufacturer, model, firmware_version, file_path,
                file_size, checksum, release_notes, release_date,
                min_device_version, compatible_models, uploaded_by, uploaded_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING
                file_id, manufacturer, model, firmware_version, file_path,
                file_size, checksum, mime_type, release_notes, release_date,
                min_device_version, compatible_models, metadata,
                is_verified, is_deprecated, uploaded_by, uploaded_at, verified_at
            "#,
            file_id,
            manufacturer,
            model,
            firmware_version,
            file_path,
            file_size,
            checksum,
            release_notes,
            release_date,
            min_device_version,
            compatible_models_arr,
            uploaded_by,
            now
        )
        .fetch_one(&self.pool)
        .await
        .context("failed to create firmware file")?;

        Ok(file)
    }

    /// Get firmware file by ID
    pub async fn get_firmware_file(&self, file_id: &str) -> Result<FirmwareFile> {
        let file = sqlx::query_as!(
            FirmwareFile,
            r#"
            SELECT
                file_id, manufacturer, model, firmware_version, file_path,
                file_size, checksum, mime_type, release_notes, release_date,
                min_device_version, compatible_models, metadata,
                is_verified, is_deprecated, uploaded_by, uploaded_at, verified_at
            FROM firmware_files
            WHERE file_id = $1
            "#,
            file_id
        )
        .fetch_one(&self.pool)
        .await
        .context("failed to get firmware file")?;

        Ok(file)
    }

    /// List firmware files with optional filters
    pub async fn list_firmware_files(&self, query: &FirmwareFileListQuery) -> Result<Vec<FirmwareFile>> {
        let limit = query.limit.unwrap_or(100);
        let offset = query.offset.unwrap_or(0);

        let files = sqlx::query_as!(
            FirmwareFile,
            r#"
            SELECT
                file_id, manufacturer, model, firmware_version, file_path,
                file_size, checksum, mime_type, release_notes, release_date,
                min_device_version, compatible_models, metadata,
                is_verified, is_deprecated, uploaded_by, uploaded_at, verified_at
            FROM firmware_files
            WHERE ($1::TEXT IS NULL OR manufacturer = $1)
              AND ($2::TEXT IS NULL OR model = $2)
              AND ($3::BOOLEAN IS NULL OR is_verified = $3)
              AND ($4::BOOLEAN IS NULL OR is_deprecated = $4)
            ORDER BY uploaded_at DESC
            LIMIT $5 OFFSET $6
            "#,
            query.manufacturer.as_deref(),
            query.model.as_deref(),
            query.is_verified,
            query.is_deprecated,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await
        .context("failed to list firmware files")?;

        Ok(files)
    }

    /// Mark firmware file as verified
    pub async fn verify_firmware_file(&self, file_id: &str) -> Result<()> {
        let now = Utc::now();

        sqlx::query!(
            r#"
            UPDATE firmware_files
            SET is_verified = true, verified_at = $2
            WHERE file_id = $1
            "#,
            file_id,
            now
        )
        .execute(&self.pool)
        .await
        .context("failed to verify firmware file")?;

        Ok(())
    }

    /// Mark firmware file as deprecated
    pub async fn deprecate_firmware_file(&self, file_id: &str) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE firmware_files
            SET is_deprecated = true
            WHERE file_id = $1
            "#,
            file_id
        )
        .execute(&self.pool)
        .await
        .context("failed to deprecate firmware file")?;

        Ok(())
    }

    /// Delete firmware file from catalog
    pub async fn delete_firmware_file(&self, file_id: &str) -> Result<()> {
        sqlx::query!(
            r#"
            DELETE FROM firmware_files
            WHERE file_id = $1
            "#,
            file_id
        )
        .execute(&self.pool)
        .await
        .context("failed to delete firmware file")?;

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

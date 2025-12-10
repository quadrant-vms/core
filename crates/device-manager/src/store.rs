use crate::types::*;
use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::{PgPool, Row};
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
                device_id, tenant_id, name,
                device_type as "device_type: DeviceType",
                manufacturer, model, firmware_version,
                primary_uri, secondary_uri,
                protocol as "protocol: ConnectionProtocol",
                username, password_encrypted,
                location, zone, tags,
                status as "status: DeviceStatus",
                last_seen_at, last_health_check_at,
                health_check_interval_secs, consecutive_failures,
                capabilities, video_codecs, audio_codecs, resolutions,
                description, notes, metadata,
                auto_start, recording_enabled, ai_enabled,
                created_at, updated_at
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
                device_id, tenant_id, name,
                device_type as "device_type: DeviceType",
                manufacturer, model, firmware_version,
                primary_uri, secondary_uri,
                protocol as "protocol: ConnectionProtocol",
                username, password_encrypted,
                location, zone, tags,
                status as "status: DeviceStatus",
                last_seen_at, last_health_check_at,
                health_check_interval_secs, consecutive_failures,
                capabilities, video_codecs, audio_codecs, resolutions,
                description, notes, metadata,
                auto_start, recording_enabled, ai_enabled,
                created_at, updated_at
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
                device_id, tenant_id, name,
                device_type as "device_type: DeviceType",
                manufacturer, model, firmware_version,
                primary_uri, secondary_uri,
                protocol as "protocol: ConnectionProtocol",
                username, password_encrypted,
                location, zone, tags,
                status as "status: DeviceStatus",
                last_seen_at, last_health_check_at,
                health_check_interval_secs, consecutive_failures,
                capabilities, video_codecs, audio_codecs, resolutions,
                description, notes, metadata,
                auto_start, recording_enabled, ai_enabled,
                created_at, updated_at
            FROM devices
            WHERE 1=1
            "#,
        );

        let mut conditions = Vec::new();
        if query.tenant_id.is_some() {
            conditions.push("tenant_id = $1");
        }
        if query.status.is_some() {
            conditions.push(format!("status = ${}", conditions.len() + 1));
        }
        if query.device_type.is_some() {
            conditions.push(format!("device_type = ${}", conditions.len() + 1));
        }
        if query.zone.is_some() {
            conditions.push(format!("zone = ${}", conditions.len() + 1));
        }

        for condition in conditions {
            sql.push_str(&format!(" AND {}", condition));
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
        let current = self
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
                device_id, tenant_id, name,
                device_type as "device_type: DeviceType",
                manufacturer, model, firmware_version,
                primary_uri, secondary_uri,
                protocol as "protocol: ConnectionProtocol",
                username, password_encrypted,
                location, zone, tags,
                status as "status: DeviceStatus",
                last_seen_at, last_health_check_at,
                health_check_interval_secs, consecutive_failures,
                capabilities, video_codecs, audio_codecs, resolutions,
                description, notes, metadata,
                auto_start, recording_enabled, ai_enabled,
                created_at, updated_at
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
                status = $2,
                last_health_check_at = $3,
                last_seen_at = CASE WHEN $2 = 'online' THEN $3 ELSE last_seen_at END,
                consecutive_failures = CASE
                    WHEN $2 IN ('online', 'maintenance') THEN 0
                    ELSE consecutive_failures + 1
                END,
                updated_at = NOW()
            WHERE device_id = $1
            "#,
            device_id,
            status as DeviceStatus,
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
                history_id,
                device_id,
                status as "status: DeviceStatus",
                response_time_ms,
                error_message,
                metadata,
                checked_at
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
                device_id, tenant_id, name,
                device_type as "device_type: DeviceType",
                manufacturer, model, firmware_version,
                primary_uri, secondary_uri,
                protocol as "protocol: ConnectionProtocol",
                username, password_encrypted,
                location, zone, tags,
                status as "status: DeviceStatus",
                last_seen_at, last_health_check_at,
                health_check_interval_secs, consecutive_failures,
                capabilities, video_codecs, audio_codecs, resolutions,
                description, notes, metadata,
                auto_start, recording_enabled, ai_enabled,
                created_at, updated_at
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

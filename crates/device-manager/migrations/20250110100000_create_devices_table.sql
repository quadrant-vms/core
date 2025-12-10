-- Create device types enum
CREATE TYPE device_type AS ENUM ('camera', 'nvr', 'encoder', 'other');

-- Create device status enum
CREATE TYPE device_status AS ENUM ('online', 'offline', 'error', 'maintenance', 'provisioning');

-- Create device connection protocol enum
CREATE TYPE connection_protocol AS ENUM ('rtsp', 'onvif', 'http', 'rtmp', 'webrtc');

-- Create devices table
CREATE TABLE IF NOT EXISTS devices (
    device_id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    name TEXT NOT NULL,
    device_type device_type NOT NULL DEFAULT 'camera',
    manufacturer TEXT,
    model TEXT,
    firmware_version TEXT,

    -- Connection information
    primary_uri TEXT NOT NULL,
    secondary_uri TEXT,
    protocol connection_protocol NOT NULL DEFAULT 'rtsp',
    username TEXT,
    password_encrypted TEXT,

    -- Location and grouping
    location TEXT,
    zone TEXT,
    tags TEXT[], -- Array of tags for categorization

    -- Status and health
    status device_status NOT NULL DEFAULT 'provisioning',
    last_seen_at TIMESTAMP WITH TIME ZONE,
    last_health_check_at TIMESTAMP WITH TIME ZONE,
    health_check_interval_secs INTEGER DEFAULT 60,
    consecutive_failures INTEGER DEFAULT 0,

    -- Device capabilities (from probing/ONVIF)
    capabilities JSONB, -- {"ptz": true, "audio": true, "motion_detection": true, etc.}
    video_codecs TEXT[], -- ["h264", "h265", "mjpeg"]
    audio_codecs TEXT[],
    resolutions TEXT[], -- ["1920x1080", "1280x720"]

    -- Metadata
    description TEXT,
    notes TEXT,
    metadata JSONB, -- Additional custom metadata

    -- Configuration
    auto_start BOOLEAN DEFAULT true, -- Auto-start streaming on device online
    recording_enabled BOOLEAN DEFAULT false,
    ai_enabled BOOLEAN DEFAULT false,

    -- Timestamps
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    -- Foreign keys
    FOREIGN KEY (tenant_id) REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    UNIQUE(tenant_id, name)
);

-- Create device_health_history table for tracking health over time
CREATE TABLE IF NOT EXISTS device_health_history (
    history_id BIGSERIAL PRIMARY KEY,
    device_id TEXT NOT NULL,
    status device_status NOT NULL,
    response_time_ms INTEGER,
    error_message TEXT,
    metadata JSONB,
    checked_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    FOREIGN KEY (device_id) REFERENCES devices(device_id) ON DELETE CASCADE
);

-- Create device_events table for audit trail
CREATE TABLE IF NOT EXISTS device_events (
    event_id BIGSERIAL PRIMARY KEY,
    device_id TEXT NOT NULL,
    event_type TEXT NOT NULL, -- "created", "updated", "status_change", "health_check", "firmware_update"
    old_value TEXT,
    new_value TEXT,
    user_id TEXT,
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    FOREIGN KEY (device_id) REFERENCES devices(device_id) ON DELETE CASCADE
);

-- Indexes for efficient queries
CREATE INDEX IF NOT EXISTS idx_devices_tenant ON devices(tenant_id);
CREATE INDEX IF NOT EXISTS idx_devices_status ON devices(status);
CREATE INDEX IF NOT EXISTS idx_devices_type ON devices(device_type);
CREATE INDEX IF NOT EXISTS idx_devices_zone ON devices(zone);
CREATE INDEX IF NOT EXISTS idx_devices_tags ON devices USING GIN(tags);
CREATE INDEX IF NOT EXISTS idx_devices_last_seen ON devices(last_seen_at DESC);
CREATE INDEX IF NOT EXISTS idx_devices_tenant_status ON devices(tenant_id, status);

CREATE INDEX IF NOT EXISTS idx_device_health_device ON device_health_history(device_id);
CREATE INDEX IF NOT EXISTS idx_device_health_checked_at ON device_health_history(checked_at DESC);
CREATE INDEX IF NOT EXISTS idx_device_health_device_time ON device_health_history(device_id, checked_at DESC);

CREATE INDEX IF NOT EXISTS idx_device_events_device ON device_events(device_id);
CREATE INDEX IF NOT EXISTS idx_device_events_type ON device_events(event_type);
CREATE INDEX IF NOT EXISTS idx_device_events_created_at ON device_events(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_device_events_device_time ON device_events(device_id, created_at DESC);

-- Trigger for automatic updated_at
CREATE TRIGGER update_devices_updated_at BEFORE UPDATE ON devices
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Trigger to log device status changes
CREATE OR REPLACE FUNCTION log_device_status_change()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.status IS DISTINCT FROM NEW.status THEN
        INSERT INTO device_events (device_id, event_type, old_value, new_value, metadata)
        VALUES (
            NEW.device_id,
            'status_change',
            OLD.status::TEXT,
            NEW.status::TEXT,
            jsonb_build_object(
                'previous_last_seen', OLD.last_seen_at,
                'current_last_seen', NEW.last_seen_at,
                'consecutive_failures', NEW.consecutive_failures
            )
        );
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_device_status_change
AFTER UPDATE ON devices
FOR EACH ROW
EXECUTE FUNCTION log_device_status_change();

-- Firmware updates table
CREATE TABLE IF NOT EXISTS firmware_updates (
    update_id TEXT PRIMARY KEY,
    device_id TEXT NOT NULL REFERENCES devices(device_id) ON DELETE CASCADE,
    firmware_version TEXT NOT NULL,
    firmware_file_path TEXT NOT NULL,
    firmware_file_size BIGINT NOT NULL,
    firmware_checksum TEXT NOT NULL, -- SHA-256 hash

    -- Status tracking
    status TEXT NOT NULL CHECK (status IN ('pending', 'uploading', 'uploaded', 'installing', 'rebooting', 'verifying', 'completed', 'failed', 'cancelled')),
    progress_percent INT NOT NULL DEFAULT 0 CHECK (progress_percent >= 0 AND progress_percent <= 100),

    -- Error handling
    error_message TEXT,
    retry_count INT NOT NULL DEFAULT 0,
    max_retries INT NOT NULL DEFAULT 3,

    -- Metadata
    previous_firmware_version TEXT,
    manufacturer TEXT,
    model TEXT,
    release_notes TEXT,
    release_date TIMESTAMPTZ,

    -- Rollback support
    can_rollback BOOLEAN NOT NULL DEFAULT false,
    rollback_data JSONB, -- Store state needed for rollback

    -- Audit
    initiated_by TEXT, -- user_id
    initiated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_firmware_updates_device_id ON firmware_updates(device_id);
CREATE INDEX idx_firmware_updates_status ON firmware_updates(status);
CREATE INDEX idx_firmware_updates_initiated_at ON firmware_updates(initiated_at DESC);

-- Firmware update history (append-only log)
CREATE TABLE IF NOT EXISTS firmware_update_history (
    history_id BIGSERIAL PRIMARY KEY,
    update_id TEXT NOT NULL REFERENCES firmware_updates(update_id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    progress_percent INT NOT NULL DEFAULT 0,
    message TEXT,
    metadata JSONB,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_firmware_update_history_update_id ON firmware_update_history(update_id);
CREATE INDEX idx_firmware_update_history_recorded_at ON firmware_update_history(recorded_at DESC);

-- Firmware files catalog (separate from updates for reusability)
CREATE TABLE IF NOT EXISTS firmware_files (
    file_id TEXT PRIMARY KEY,
    manufacturer TEXT NOT NULL,
    model TEXT NOT NULL,
    firmware_version TEXT NOT NULL,

    -- File information
    file_path TEXT NOT NULL,
    file_size BIGINT NOT NULL,
    checksum TEXT NOT NULL, -- SHA-256
    mime_type TEXT,

    -- Metadata
    release_notes TEXT,
    release_date TIMESTAMPTZ,
    min_device_version TEXT,
    compatible_models TEXT[], -- Array of compatible model names
    metadata JSONB,

    -- Validation
    is_verified BOOLEAN NOT NULL DEFAULT false,
    is_deprecated BOOLEAN NOT NULL DEFAULT false,

    -- Timestamps
    uploaded_by TEXT, -- user_id
    uploaded_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    verified_at TIMESTAMPTZ,

    UNIQUE(manufacturer, model, firmware_version)
);

CREATE INDEX idx_firmware_files_manufacturer_model ON firmware_files(manufacturer, model);
CREATE INDEX idx_firmware_files_version ON firmware_files(firmware_version);
CREATE INDEX idx_firmware_files_uploaded_at ON firmware_files(uploaded_at DESC);

-- Trigger to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_firmware_update_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER firmware_updates_updated_at
    BEFORE UPDATE ON firmware_updates
    FOR EACH ROW
    EXECUTE FUNCTION update_firmware_update_timestamp();

-- Trigger to log firmware update status changes to history
CREATE OR REPLACE FUNCTION log_firmware_update_status_change()
RETURNS TRIGGER AS $$
BEGIN
    -- Log any status or progress changes
    IF (TG_OP = 'INSERT') OR
       (TG_OP = 'UPDATE' AND (NEW.status != OLD.status OR NEW.progress_percent != OLD.progress_percent)) THEN
        INSERT INTO firmware_update_history (update_id, status, progress_percent, message)
        VALUES (NEW.update_id, NEW.status, NEW.progress_percent, NEW.error_message);
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER firmware_update_status_change_trigger
    AFTER INSERT OR UPDATE ON firmware_updates
    FOR EACH ROW
    EXECUTE FUNCTION log_firmware_update_status_change();

-- Trigger to log firmware update events to device_events
CREATE OR REPLACE FUNCTION log_firmware_update_to_device_events()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO device_events (device_id, event_type, new_value, metadata)
        VALUES (
            NEW.device_id,
            'firmware_update_initiated',
            NEW.firmware_version,
            jsonb_build_object('update_id', NEW.update_id, 'previous_version', NEW.previous_firmware_version)
        );
    ELSIF TG_OP = 'UPDATE' AND NEW.status = 'completed' AND OLD.status != 'completed' THEN
        INSERT INTO device_events (device_id, event_type, old_value, new_value, metadata)
        VALUES (
            NEW.device_id,
            'firmware_update_completed',
            NEW.previous_firmware_version,
            NEW.firmware_version,
            jsonb_build_object('update_id', NEW.update_id, 'duration_secs', EXTRACT(EPOCH FROM (NEW.completed_at - NEW.started_at)))
        );

        -- Update device firmware_version
        UPDATE devices SET firmware_version = NEW.firmware_version, updated_at = CURRENT_TIMESTAMP
        WHERE device_id = NEW.device_id;

    ELSIF TG_OP = 'UPDATE' AND NEW.status = 'failed' AND OLD.status != 'failed' THEN
        INSERT INTO device_events (device_id, event_type, new_value, metadata)
        VALUES (
            NEW.device_id,
            'firmware_update_failed',
            NEW.error_message,
            jsonb_build_object('update_id', NEW.update_id, 'retry_count', NEW.retry_count)
        );
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER firmware_update_device_events_trigger
    AFTER INSERT OR UPDATE ON firmware_updates
    FOR EACH ROW
    EXECUTE FUNCTION log_firmware_update_to_device_events();

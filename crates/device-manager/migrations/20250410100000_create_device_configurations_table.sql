-- Add device_configurations table for camera configuration management
-- This table tracks configuration changes pushed to devices

-- Configuration status enum
DO $$ BEGIN
    CREATE TYPE configuration_status AS ENUM ('pending', 'applied', 'failed', 'partiallyapplied');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- Device configurations table
CREATE TABLE IF NOT EXISTS device_configurations (
    config_id TEXT PRIMARY KEY,
    device_id TEXT NOT NULL,
    requested_config JSONB NOT NULL,      -- Configuration that was requested
    applied_config JSONB,                  -- Configuration that was actually applied
    status configuration_status NOT NULL DEFAULT 'pending',
    error_message TEXT,
    applied_by TEXT,                       -- User ID who applied the configuration
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    applied_at TIMESTAMP WITH TIME ZONE,

    FOREIGN KEY (device_id) REFERENCES devices(device_id) ON DELETE CASCADE
);

-- Indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_device_configurations_device_id ON device_configurations(device_id);
CREATE INDEX IF NOT EXISTS idx_device_configurations_status ON device_configurations(status);
CREATE INDEX IF NOT EXISTS idx_device_configurations_created_at ON device_configurations(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_device_configurations_applied_at ON device_configurations(applied_at DESC);

-- Function to automatically update applied_at timestamp
CREATE OR REPLACE FUNCTION update_configuration_applied_at()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.status = 'applied' OR NEW.status = 'partiallyapplied' THEN
        IF NEW.applied_at IS NULL THEN
            NEW.applied_at = NOW();
        END IF;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to update applied_at timestamp
CREATE TRIGGER trigger_update_configuration_applied_at
    BEFORE UPDATE ON device_configurations
    FOR EACH ROW
    WHEN (NEW.status = 'applied' OR NEW.status = 'partiallyapplied')
    EXECUTE FUNCTION update_configuration_applied_at();

-- Function to log configuration changes to device_events
CREATE OR REPLACE FUNCTION log_configuration_change()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO device_events (device_id, event_type, new_value, metadata, created_at)
        VALUES (
            NEW.device_id,
            'configuration_requested',
            NEW.config_id,
            jsonb_build_object(
                'requested_config', NEW.requested_config,
                'applied_by', NEW.applied_by
            ),
            NOW()
        );
    ELSIF TG_OP = 'UPDATE' AND OLD.status != NEW.status THEN
        INSERT INTO device_events (device_id, event_type, old_value, new_value, metadata, created_at)
        VALUES (
            NEW.device_id,
            'configuration_status_changed',
            OLD.status::TEXT,
            NEW.status::TEXT,
            jsonb_build_object(
                'config_id', NEW.config_id,
                'applied_config', NEW.applied_config,
                'error_message', NEW.error_message
            ),
            NOW()
        );
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to log configuration changes
CREATE TRIGGER trigger_log_configuration_change
    AFTER INSERT OR UPDATE ON device_configurations
    FOR EACH ROW
    EXECUTE FUNCTION log_configuration_change();

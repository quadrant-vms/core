-- Create tour state enum
CREATE TYPE tour_state AS ENUM ('stopped', 'running', 'paused');

-- Create PTZ presets table
CREATE TABLE IF NOT EXISTS ptz_presets (
    preset_id TEXT PRIMARY KEY,
    device_id TEXT NOT NULL,
    name TEXT NOT NULL,
    position JSONB NOT NULL, -- {"pan": 0.0, "tilt": 0.0, "zoom": 0.0}
    description TEXT,
    thumbnail_url TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    FOREIGN KEY (device_id) REFERENCES devices(device_id) ON DELETE CASCADE,
    UNIQUE(device_id, name)
);

-- Create PTZ tours table
CREATE TABLE IF NOT EXISTS ptz_tours (
    tour_id TEXT PRIMARY KEY,
    device_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    state tour_state NOT NULL DEFAULT 'stopped',
    loop_enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    FOREIGN KEY (device_id) REFERENCES devices(device_id) ON DELETE CASCADE,
    UNIQUE(device_id, name)
);

-- Create PTZ tour steps table
CREATE TABLE IF NOT EXISTS ptz_tour_steps (
    step_id TEXT PRIMARY KEY,
    tour_id TEXT NOT NULL,
    sequence_order INTEGER NOT NULL,
    preset_id TEXT, -- Optional: reference to a preset
    position JSONB, -- Optional: direct position if no preset
    dwell_time_ms BIGINT NOT NULL DEFAULT 5000,
    speed REAL NOT NULL DEFAULT 0.5,
    FOREIGN KEY (tour_id) REFERENCES ptz_tours(tour_id) ON DELETE CASCADE,
    FOREIGN KEY (preset_id) REFERENCES ptz_presets(preset_id) ON DELETE SET NULL,
    UNIQUE(tour_id, sequence_order),
    CHECK (preset_id IS NOT NULL OR position IS NOT NULL)
);

-- Indexes for efficient queries
CREATE INDEX IF NOT EXISTS idx_ptz_presets_device ON ptz_presets(device_id);
CREATE INDEX IF NOT EXISTS idx_ptz_tours_device ON ptz_tours(device_id);
CREATE INDEX IF NOT EXISTS idx_ptz_tours_state ON ptz_tours(state);
CREATE INDEX IF NOT EXISTS idx_ptz_tour_steps_tour ON ptz_tour_steps(tour_id);
CREATE INDEX IF NOT EXISTS idx_ptz_tour_steps_order ON ptz_tour_steps(tour_id, sequence_order);

-- Triggers for automatic updated_at
CREATE TRIGGER update_ptz_presets_updated_at BEFORE UPDATE ON ptz_presets
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_ptz_tours_updated_at BEFORE UPDATE ON ptz_tours
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

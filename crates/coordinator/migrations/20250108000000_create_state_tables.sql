-- Create streams table for persistent stream state
CREATE TABLE IF NOT EXISTS streams (
    stream_id TEXT PRIMARY KEY,
    uri TEXT NOT NULL,
    codec TEXT NOT NULL,
    container TEXT NOT NULL,
    state TEXT NOT NULL,
    node_id TEXT NOT NULL,
    lease_id TEXT,
    playlist_path TEXT,
    output_dir TEXT,
    last_error TEXT,
    started_at BIGINT,
    stopped_at BIGINT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    FOREIGN KEY (lease_id) REFERENCES leases(lease_id) ON DELETE SET NULL
);

-- Create recordings table for persistent recording state
CREATE TABLE IF NOT EXISTS recordings (
    recording_id TEXT PRIMARY KEY,
    source_stream_id TEXT,
    source_uri TEXT,
    retention_hours INTEGER,
    format TEXT NOT NULL,
    state TEXT NOT NULL,
    node_id TEXT NOT NULL,
    lease_id TEXT,
    storage_path TEXT,
    last_error TEXT,
    started_at BIGINT,
    stopped_at BIGINT,
    duration_secs REAL,
    file_size_bytes BIGINT,
    resolution TEXT,
    codec_name TEXT,
    bitrate_kbps INTEGER,
    fps REAL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    FOREIGN KEY (lease_id) REFERENCES leases(lease_id) ON DELETE SET NULL
);

-- Create ai_tasks table for persistent AI task state
CREATE TABLE IF NOT EXISTS ai_tasks (
    task_id TEXT PRIMARY KEY,
    plugin_type TEXT NOT NULL,
    source_stream_id TEXT,
    source_recording_id TEXT,
    output_format TEXT NOT NULL,
    output_config JSONB NOT NULL,
    frame_config JSONB NOT NULL,
    state TEXT NOT NULL,
    node_id TEXT NOT NULL,
    lease_id TEXT,
    last_error TEXT,
    started_at BIGINT,
    stopped_at BIGINT,
    last_processed_frame BIGINT,
    frames_processed BIGINT DEFAULT 0,
    detections_made BIGINT DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    FOREIGN KEY (lease_id) REFERENCES leases(lease_id) ON DELETE SET NULL
);

-- Indexes for efficient queries
CREATE INDEX IF NOT EXISTS idx_streams_state ON streams(state);
CREATE INDEX IF NOT EXISTS idx_streams_node_id ON streams(node_id);
CREATE INDEX IF NOT EXISTS idx_streams_lease_id ON streams(lease_id);

CREATE INDEX IF NOT EXISTS idx_recordings_state ON recordings(state);
CREATE INDEX IF NOT EXISTS idx_recordings_node_id ON recordings(node_id);
CREATE INDEX IF NOT EXISTS idx_recordings_lease_id ON recordings(lease_id);
CREATE INDEX IF NOT EXISTS idx_recordings_source_stream ON recordings(source_stream_id);

CREATE INDEX IF NOT EXISTS idx_ai_tasks_state ON ai_tasks(state);
CREATE INDEX IF NOT EXISTS idx_ai_tasks_node_id ON ai_tasks(node_id);
CREATE INDEX IF NOT EXISTS idx_ai_tasks_lease_id ON ai_tasks(lease_id);
CREATE INDEX IF NOT EXISTS idx_ai_tasks_plugin_type ON ai_tasks(plugin_type);
CREATE INDEX IF NOT EXISTS idx_ai_tasks_source_stream ON ai_tasks(source_stream_id);
CREATE INDEX IF NOT EXISTS idx_ai_tasks_source_recording ON ai_tasks(source_recording_id);

-- Function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Triggers for automatic updated_at
CREATE TRIGGER update_streams_updated_at BEFORE UPDATE ON streams
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_recordings_updated_at BEFORE UPDATE ON recordings
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_ai_tasks_updated_at BEFORE UPDATE ON ai_tasks
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

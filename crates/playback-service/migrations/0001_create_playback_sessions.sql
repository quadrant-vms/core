-- Create playback_sessions table
CREATE TABLE IF NOT EXISTS playback_sessions (
    session_id VARCHAR(255) PRIMARY KEY,
    source_type VARCHAR(50) NOT NULL,
    source_id VARCHAR(255) NOT NULL,
    protocol VARCHAR(50) NOT NULL,
    state VARCHAR(50) NOT NULL,
    lease_id VARCHAR(255),
    node_id VARCHAR(255),
    playback_url TEXT,
    current_position_secs DOUBLE PRECISION,
    duration_secs DOUBLE PRECISION,
    start_time_secs DOUBLE PRECISION,
    speed DOUBLE PRECISION DEFAULT 1.0,
    last_error TEXT,
    started_at BIGINT,
    stopped_at BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for common queries
CREATE INDEX idx_playback_sessions_source ON playback_sessions(source_type, source_id);
CREATE INDEX idx_playback_sessions_state ON playback_sessions(state);
CREATE INDEX idx_playback_sessions_node_id ON playback_sessions(node_id);
CREATE INDEX idx_playback_sessions_lease_id ON playback_sessions(lease_id);

-- Create updated_at trigger
CREATE OR REPLACE FUNCTION update_playback_session_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_playback_session_updated_at
    BEFORE UPDATE ON playback_sessions
    FOR EACH ROW
    EXECUTE FUNCTION update_playback_session_updated_at();

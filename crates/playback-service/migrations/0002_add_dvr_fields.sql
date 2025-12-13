-- Add DVR fields to playback_sessions table
ALTER TABLE playback_sessions
ADD COLUMN dvr_enabled BOOLEAN DEFAULT FALSE NOT NULL,
ADD COLUMN dvr_rewind_limit_secs DOUBLE PRECISION,
ADD COLUMN dvr_buffer_window_secs DOUBLE PRECISION,
ADD COLUMN dvr_earliest_timestamp BIGINT,
ADD COLUMN dvr_latest_timestamp BIGINT,
ADD COLUMN dvr_current_position BIGINT;

-- Add index for DVR-enabled sessions
CREATE INDEX idx_playback_sessions_dvr_enabled ON playback_sessions(dvr_enabled) WHERE dvr_enabled = TRUE;

-- Add index for DVR timestamp queries
CREATE INDEX idx_playback_sessions_dvr_timestamps ON playback_sessions(dvr_earliest_timestamp, dvr_latest_timestamp) WHERE dvr_enabled = TRUE;

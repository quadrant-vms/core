-- Recording Index Table (for fast search)
CREATE TABLE IF NOT EXISTS recording_index (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    recording_id VARCHAR(255) NOT NULL UNIQUE,
    tenant_id UUID,

    -- Recording metadata
    device_id VARCHAR(255),
    device_name VARCHAR(255),
    zone VARCHAR(255),
    location VARCHAR(512),

    -- Time range
    started_at TIMESTAMPTZ NOT NULL,
    stopped_at TIMESTAMPTZ,
    duration_secs INTEGER,

    -- Video metadata
    resolution VARCHAR(50),
    video_codec VARCHAR(50),
    audio_codec VARCHAR(50),
    file_size_bytes BIGINT,
    storage_path VARCHAR(512),

    -- Tags and labels
    tags TEXT[], -- Array of tags for categorization
    labels JSONB DEFAULT '{}', -- Key-value labels

    -- Full-text search
    search_vector tsvector,

    -- Status
    state VARCHAR(50) NOT NULL,

    -- Timestamps
    indexed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT fk_tenant FOREIGN KEY (tenant_id) REFERENCES auth_service.tenants(id) ON DELETE CASCADE
);

CREATE INDEX idx_recording_index_tenant ON recording_index(tenant_id);
CREATE INDEX idx_recording_index_device ON recording_index(device_id);
CREATE INDEX idx_recording_index_zone ON recording_index(zone);
CREATE INDEX idx_recording_index_started_at ON recording_index(started_at DESC);
CREATE INDEX idx_recording_index_stopped_at ON recording_index(stopped_at DESC);
CREATE INDEX idx_recording_index_state ON recording_index(state);
CREATE INDEX idx_recording_index_tags ON recording_index USING GIN(tags);
CREATE INDEX idx_recording_index_labels ON recording_index USING GIN(labels);
CREATE INDEX idx_recording_index_search ON recording_index USING GIN(search_vector);

-- Event Index Table (AI detections, motion, alerts)
CREATE TABLE IF NOT EXISTS event_index (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id VARCHAR(255) NOT NULL,
    tenant_id UUID,

    -- Event type: ai_detection, motion_detected, alert_triggered, system_event
    event_type VARCHAR(50) NOT NULL,

    -- Associated recording
    recording_id VARCHAR(255),

    -- Time information
    occurred_at TIMESTAMPTZ NOT NULL,
    duration_secs INTEGER,

    -- Location information
    device_id VARCHAR(255),
    device_name VARCHAR(255),
    zone VARCHAR(255),

    -- Event details (JSON)
    -- For AI detections: {"object_type": "person", "confidence": 0.95, "bbox": {...}}
    -- For motion: {"motion_score": 0.8, "region": {...}}
    -- For alerts: {"rule_id": "...", "severity": "high"}
    event_data JSONB NOT NULL DEFAULT '{}',

    -- Object detection specifics (for AI events)
    detected_objects TEXT[], -- Array of object types: ["person", "car"]
    object_count INTEGER,
    max_confidence FLOAT,

    -- Snapshot reference
    snapshot_path VARCHAR(512),
    thumbnail_data TEXT, -- Base64 encoded thumbnail

    -- Severity (for alerts)
    severity VARCHAR(20), -- info, warning, error, critical

    -- Tags
    tags TEXT[],

    -- Full-text search
    search_vector tsvector,

    -- Timestamps
    indexed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_event_index_tenant ON event_index(tenant_id);
CREATE INDEX idx_event_index_type ON event_index(event_type);
CREATE INDEX idx_event_index_recording ON event_index(recording_id);
CREATE INDEX idx_event_index_occurred_at ON event_index(occurred_at DESC);
CREATE INDEX idx_event_index_device ON event_index(device_id);
CREATE INDEX idx_event_index_zone ON event_index(zone);
CREATE INDEX idx_event_index_severity ON event_index(severity);
CREATE INDEX idx_event_index_objects ON event_index USING GIN(detected_objects);
CREATE INDEX idx_event_index_tags ON event_index USING GIN(tags);
CREATE INDEX idx_event_index_data ON event_index USING GIN(event_data);
CREATE INDEX idx_event_index_search ON event_index USING GIN(search_vector);

-- Composite index for time-range queries
CREATE INDEX idx_event_index_time_range ON event_index(tenant_id, occurred_at DESC, event_type);

-- Search Query Log (for analytics and optimization)
CREATE TABLE IF NOT EXISTS search_query_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID,
    user_id UUID,

    -- Query details
    query_type VARCHAR(50) NOT NULL, -- recording_search, event_search, object_search
    query_params JSONB NOT NULL,

    -- Results
    result_count INTEGER NOT NULL,
    execution_time_ms INTEGER NOT NULL,

    -- Timestamps
    executed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_search_log_tenant ON search_query_log(tenant_id);
CREATE INDEX idx_search_log_executed_at ON search_query_log(executed_at DESC);
CREATE INDEX idx_search_log_query_type ON search_query_log(query_type);

-- Triggers for full-text search vector updates
CREATE OR REPLACE FUNCTION update_recording_search_vector()
RETURNS TRIGGER AS $$
BEGIN
    NEW.search_vector :=
        setweight(to_tsvector('english', COALESCE(NEW.device_name, '')), 'A') ||
        setweight(to_tsvector('english', COALESCE(NEW.zone, '')), 'B') ||
        setweight(to_tsvector('english', COALESCE(NEW.location, '')), 'C') ||
        setweight(to_tsvector('english', COALESCE(array_to_string(NEW.tags, ' '), '')), 'B');
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER recording_index_search_vector_update
    BEFORE INSERT OR UPDATE ON recording_index
    FOR EACH ROW
    EXECUTE FUNCTION update_recording_search_vector();

CREATE OR REPLACE FUNCTION update_event_search_vector()
RETURNS TRIGGER AS $$
BEGIN
    NEW.search_vector :=
        setweight(to_tsvector('english', COALESCE(NEW.event_type, '')), 'A') ||
        setweight(to_tsvector('english', COALESCE(NEW.device_name, '')), 'B') ||
        setweight(to_tsvector('english', COALESCE(NEW.zone, '')), 'B') ||
        setweight(to_tsvector('english', COALESCE(array_to_string(NEW.detected_objects, ' '), '')), 'A') ||
        setweight(to_tsvector('english', COALESCE(array_to_string(NEW.tags, ' '), '')), 'C');
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER event_index_search_vector_update
    BEFORE INSERT OR UPDATE ON event_index
    FOR EACH ROW
    EXECUTE FUNCTION update_event_search_vector();

-- Trigger to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_index_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER recording_index_updated_at
    BEFORE UPDATE ON recording_index
    FOR EACH ROW
    EXECUTE FUNCTION update_index_updated_at();

CREATE TRIGGER event_index_updated_at
    BEFORE UPDATE ON event_index
    FOR EACH ROW
    EXECUTE FUNCTION update_index_updated_at();

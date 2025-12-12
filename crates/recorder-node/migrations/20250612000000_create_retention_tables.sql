-- Retention Policies Table
CREATE TABLE IF NOT EXISTS retention_policies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    enabled BOOLEAN NOT NULL DEFAULT true,

    -- Policy type: time_based, storage_quota, conditional
    policy_type VARCHAR(50) NOT NULL,

    -- Time-based retention settings
    retention_days INTEGER, -- Keep recordings for N days

    -- Storage quota settings (in bytes)
    max_storage_bytes BIGINT, -- Maximum storage for recordings matching this policy

    -- Conditional settings (JSON)
    -- Examples:
    -- {"device_id": "uuid"} - Apply only to specific device
    -- {"zone": "entrance"} - Apply to recordings from specific zone
    -- {"tags": ["important"]} - Apply to recordings with specific tags
    -- {"min_duration_secs": 30} - Only delete recordings shorter than X seconds
    condition_json JSONB NOT NULL DEFAULT '{}',

    -- Tiered storage settings
    enable_tiered_storage BOOLEAN NOT NULL DEFAULT false,
    cold_storage_after_days INTEGER, -- Move to cold storage after N days
    cold_storage_path VARCHAR(512), -- Path to cold storage location

    -- Execution settings
    priority INTEGER NOT NULL DEFAULT 0, -- Higher priority policies execute first
    dry_run BOOLEAN NOT NULL DEFAULT false, -- Don't actually delete, just log

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_by UUID,

    UNIQUE(tenant_id, name)
);

CREATE INDEX idx_retention_policies_tenant ON retention_policies(tenant_id);
CREATE INDEX idx_retention_policies_enabled ON retention_policies(enabled);
CREATE INDEX idx_retention_policies_type ON retention_policies(policy_type);
CREATE INDEX idx_retention_policies_priority ON retention_policies(priority DESC);
CREATE INDEX idx_retention_policies_condition ON retention_policies USING GIN(condition_json);

-- Retention Executions Table (audit trail of policy runs)
CREATE TABLE IF NOT EXISTS retention_executions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    policy_id UUID NOT NULL REFERENCES retention_policies(id) ON DELETE CASCADE,

    -- Execution status: running, completed, failed
    status VARCHAR(20) NOT NULL DEFAULT 'running',

    -- Statistics
    recordings_scanned INTEGER NOT NULL DEFAULT 0,
    recordings_deleted INTEGER NOT NULL DEFAULT 0,
    recordings_moved_to_cold INTEGER NOT NULL DEFAULT 0,
    bytes_freed BIGINT NOT NULL DEFAULT 0,
    bytes_moved BIGINT NOT NULL DEFAULT 0,

    -- Timing
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    duration_secs INTEGER,

    -- Error tracking
    error_message TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_retention_executions_policy ON retention_executions(policy_id);
CREATE INDEX idx_retention_executions_status ON retention_executions(status);
CREATE INDEX idx_retention_executions_started_at ON retention_executions(started_at DESC);

-- Retention Actions Table (individual recording actions)
CREATE TABLE IF NOT EXISTS retention_actions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    execution_id UUID NOT NULL REFERENCES retention_executions(id) ON DELETE CASCADE,
    recording_id VARCHAR(255) NOT NULL,

    -- Action type: delete, move_to_cold, skip
    action_type VARCHAR(20) NOT NULL,

    -- Action status: pending, completed, failed
    status VARCHAR(20) NOT NULL DEFAULT 'pending',

    -- Recording details at time of action
    recording_path VARCHAR(512),
    recording_size_bytes BIGINT,
    recording_duration_secs BIGINT,
    recording_created_at TIMESTAMPTZ,

    -- Action result
    performed_at TIMESTAMPTZ,
    error_message TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_retention_actions_execution ON retention_actions(execution_id);
CREATE INDEX idx_retention_actions_recording ON retention_actions(recording_id);
CREATE INDEX idx_retention_actions_status ON retention_actions(status);
CREATE INDEX idx_retention_actions_type ON retention_actions(action_type);

-- Storage Statistics Table (for quota management)
CREATE TABLE IF NOT EXISTS storage_statistics (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Grouping criteria
    tenant_id UUID,
    device_id UUID,
    zone VARCHAR(255),

    -- Statistics
    total_recordings INTEGER NOT NULL DEFAULT 0,
    total_bytes BIGINT NOT NULL DEFAULT 0,
    oldest_recording_at TIMESTAMPTZ,
    newest_recording_at TIMESTAMPTZ,

    -- Timing
    calculated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(tenant_id, device_id, zone)
);

CREATE INDEX idx_storage_stats_tenant ON storage_statistics(tenant_id);
CREATE INDEX idx_storage_stats_device ON storage_statistics(device_id);
CREATE INDEX idx_storage_stats_zone ON storage_statistics(zone);
CREATE INDEX idx_storage_stats_calculated_at ON storage_statistics(calculated_at DESC);

-- Trigger to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_retention_policies_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER retention_policies_updated_at
    BEFORE UPDATE ON retention_policies
    FOR EACH ROW
    EXECUTE FUNCTION update_retention_policies_updated_at();

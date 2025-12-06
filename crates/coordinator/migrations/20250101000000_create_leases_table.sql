-- Create leases table for persistent lease storage
CREATE TABLE IF NOT EXISTS leases (
    lease_id TEXT PRIMARY KEY,
    resource_id TEXT NOT NULL UNIQUE,
    holder_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    expires_at_epoch_secs BIGINT NOT NULL,
    version BIGINT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Index for efficient expiration queries
CREATE INDEX IF NOT EXISTS idx_leases_expires_at ON leases(expires_at_epoch_secs);

-- Index for filtering by kind
CREATE INDEX IF NOT EXISTS idx_leases_kind ON leases(kind);

-- Index for holder queries
CREATE INDEX IF NOT EXISTS idx_leases_holder_id ON leases(holder_id);

-- Discovery scans table
CREATE TABLE IF NOT EXISTS discovery_scans (
    scan_id VARCHAR(255) PRIMARY KEY,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    devices_found INTEGER NOT NULL DEFAULT 0,
    status VARCHAR(50) NOT NULL DEFAULT 'running',
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Discovered devices table (temporary staging before import)
CREATE TABLE IF NOT EXISTS discovered_devices (
    discovery_id VARCHAR(255) PRIMARY KEY DEFAULT gen_random_uuid()::TEXT,
    scan_id VARCHAR(255) NOT NULL REFERENCES discovery_scans(scan_id) ON DELETE CASCADE,
    device_service_url TEXT NOT NULL,
    scopes TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    types TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    xaddrs TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    manufacturer TEXT,
    model TEXT,
    hardware_id TEXT,
    name TEXT,
    location TEXT,
    discovered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    imported BOOLEAN NOT NULL DEFAULT FALSE,
    imported_device_id VARCHAR(255) REFERENCES devices(device_id) ON DELETE SET NULL,
    imported_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_discovery_scans_started_at ON discovery_scans(started_at DESC);
CREATE INDEX IF NOT EXISTS idx_discovery_scans_status ON discovery_scans(status);
CREATE INDEX IF NOT EXISTS idx_discovered_devices_scan_id ON discovered_devices(scan_id);
CREATE INDEX IF NOT EXISTS idx_discovered_devices_imported ON discovered_devices(imported);
CREATE INDEX IF NOT EXISTS idx_discovered_devices_discovered_at ON discovered_devices(discovered_at DESC);

-- GIN index for array searches
CREATE INDEX IF NOT EXISTS idx_discovered_devices_scopes ON discovered_devices USING GIN(scopes);
CREATE INDEX IF NOT EXISTS idx_discovered_devices_types ON discovered_devices USING GIN(types);

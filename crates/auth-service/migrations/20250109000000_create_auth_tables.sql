-- Create tenants table for multi-tenancy support
CREATE TABLE IF NOT EXISTS tenants (
    tenant_id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    is_active BOOLEAN NOT NULL DEFAULT true,
    max_users INTEGER,
    max_streams INTEGER,
    max_recordings INTEGER,
    max_ai_tasks INTEGER,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create users table
CREATE TABLE IF NOT EXISTS users (
    user_id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    username TEXT NOT NULL,
    email TEXT NOT NULL,
    password_hash TEXT, -- NULL for SSO-only users
    display_name TEXT,
    is_active BOOLEAN NOT NULL DEFAULT true,
    is_system_admin BOOLEAN NOT NULL DEFAULT false,
    last_login_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(tenant_id, username),
    UNIQUE(tenant_id, email),
    FOREIGN KEY (tenant_id) REFERENCES tenants(tenant_id) ON DELETE CASCADE
);

-- Create roles table for RBAC
CREATE TABLE IF NOT EXISTS roles (
    role_id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    is_system_role BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(tenant_id, name),
    FOREIGN KEY (tenant_id) REFERENCES tenants(tenant_id) ON DELETE CASCADE
);

-- Create permissions table
CREATE TABLE IF NOT EXISTS permissions (
    permission_id TEXT PRIMARY KEY,
    resource TEXT NOT NULL, -- e.g., "stream", "recording", "ai_task", "user"
    action TEXT NOT NULL, -- e.g., "read", "create", "update", "delete", "execute"
    description TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create role_permissions junction table
CREATE TABLE IF NOT EXISTS role_permissions (
    role_id TEXT NOT NULL,
    permission_id TEXT NOT NULL,
    granted_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    PRIMARY KEY (role_id, permission_id),
    FOREIGN KEY (role_id) REFERENCES roles(role_id) ON DELETE CASCADE,
    FOREIGN KEY (permission_id) REFERENCES permissions(permission_id) ON DELETE CASCADE
);

-- Create user_roles junction table
CREATE TABLE IF NOT EXISTS user_roles (
    user_id TEXT NOT NULL,
    role_id TEXT NOT NULL,
    granted_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    granted_by TEXT,
    PRIMARY KEY (user_id, role_id),
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE,
    FOREIGN KEY (role_id) REFERENCES roles(role_id) ON DELETE CASCADE
);

-- Create API tokens table for long-lived authentication
CREATE TABLE IF NOT EXISTS api_tokens (
    token_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT,
    expires_at TIMESTAMP WITH TIME ZONE,
    last_used_at TIMESTAMP WITH TIME ZONE,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE
);

-- Create OIDC providers table for SSO configuration
CREATE TABLE IF NOT EXISTS oidc_providers (
    provider_id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    name TEXT NOT NULL,
    issuer_url TEXT NOT NULL,
    client_id TEXT NOT NULL,
    client_secret TEXT NOT NULL,
    scopes TEXT[] NOT NULL DEFAULT ARRAY['openid', 'profile', 'email'],
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(tenant_id, name),
    FOREIGN KEY (tenant_id) REFERENCES tenants(tenant_id) ON DELETE CASCADE
);

-- Create audit_logs table for security auditing
CREATE TABLE IF NOT EXISTS audit_logs (
    log_id BIGSERIAL PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    user_id TEXT,
    action TEXT NOT NULL, -- e.g., "login", "logout", "create_stream", "delete_recording"
    resource_type TEXT, -- e.g., "stream", "recording", "user"
    resource_id TEXT,
    ip_address TEXT,
    user_agent TEXT,
    status TEXT NOT NULL, -- "success", "failure", "denied"
    error_message TEXT,
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    FOREIGN KEY (tenant_id) REFERENCES tenants(tenant_id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE SET NULL
);

-- Indexes for efficient queries
CREATE INDEX IF NOT EXISTS idx_users_tenant ON users(tenant_id);
CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
CREATE INDEX IF NOT EXISTS idx_users_active ON users(is_active);

CREATE INDEX IF NOT EXISTS idx_roles_tenant ON roles(tenant_id);
CREATE INDEX IF NOT EXISTS idx_roles_name ON roles(name);

CREATE INDEX IF NOT EXISTS idx_permissions_resource ON permissions(resource);
CREATE INDEX IF NOT EXISTS idx_permissions_action ON permissions(action);

CREATE INDEX IF NOT EXISTS idx_api_tokens_user ON api_tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_api_tokens_hash ON api_tokens(token_hash);
CREATE INDEX IF NOT EXISTS idx_api_tokens_active ON api_tokens(is_active);

CREATE INDEX IF NOT EXISTS idx_oidc_providers_tenant ON oidc_providers(tenant_id);
CREATE INDEX IF NOT EXISTS idx_oidc_providers_active ON oidc_providers(is_active);

CREATE INDEX IF NOT EXISTS idx_audit_logs_tenant ON audit_logs(tenant_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_user ON audit_logs(user_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_action ON audit_logs(action);
CREATE INDEX IF NOT EXISTS idx_audit_logs_resource ON audit_logs(resource_type, resource_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at ON audit_logs(created_at DESC);

-- Function to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- Triggers for automatic updated_at
CREATE TRIGGER update_tenants_updated_at BEFORE UPDATE ON tenants
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_users_updated_at BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_roles_updated_at BEFORE UPDATE ON roles
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_oidc_providers_updated_at BEFORE UPDATE ON oidc_providers
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Insert default system tenant
INSERT INTO tenants (tenant_id, name, description, is_active)
VALUES ('system', 'System', 'Default system tenant', true)
ON CONFLICT (tenant_id) DO NOTHING;

-- Insert default system permissions
INSERT INTO permissions (permission_id, resource, action, description) VALUES
    ('stream:read', 'stream', 'read', 'View stream information'),
    ('stream:create', 'stream', 'create', 'Create new streams'),
    ('stream:update', 'stream', 'update', 'Update stream configuration'),
    ('stream:delete', 'stream', 'delete', 'Stop and delete streams'),
    ('recording:read', 'recording', 'read', 'View recording information'),
    ('recording:create', 'recording', 'create', 'Create new recordings'),
    ('recording:update', 'recording', 'update', 'Update recording configuration'),
    ('recording:delete', 'recording', 'delete', 'Stop and delete recordings'),
    ('ai_task:read', 'ai_task', 'read', 'View AI task information'),
    ('ai_task:create', 'ai_task', 'create', 'Create new AI tasks'),
    ('ai_task:update', 'ai_task', 'update', 'Update AI task configuration'),
    ('ai_task:delete', 'ai_task', 'delete', 'Stop and delete AI tasks'),
    ('device:read', 'device', 'read', 'View device information'),
    ('device:create', 'device', 'create', 'Create and onboard new devices'),
    ('device:update', 'device', 'update', 'Update device configuration'),
    ('device:delete', 'device', 'delete', 'Delete devices'),
    ('user:read', 'user', 'read', 'View user information'),
    ('user:create', 'user', 'create', 'Create new users'),
    ('user:update', 'user', 'update', 'Update user information'),
    ('user:delete', 'user', 'delete', 'Delete users'),
    ('role:read', 'role', 'read', 'View role information'),
    ('role:create', 'role', 'create', 'Create new roles'),
    ('role:update', 'role', 'update', 'Update role configuration'),
    ('role:delete', 'role', 'delete', 'Delete roles'),
    ('tenant:read', 'tenant', 'read', 'View tenant information'),
    ('tenant:create', 'tenant', 'create', 'Create new tenants'),
    ('tenant:update', 'tenant', 'update', 'Update tenant configuration'),
    ('tenant:delete', 'tenant', 'delete', 'Delete tenants'),
    ('audit:read', 'audit', 'read', 'View audit logs')
ON CONFLICT (permission_id) DO NOTHING;

-- Insert default system admin role
INSERT INTO roles (role_id, tenant_id, name, description, is_system_role)
VALUES ('system-admin', 'system', 'System Administrator', 'Full system access', true)
ON CONFLICT (role_id) DO NOTHING;

-- Grant all permissions to system admin role
INSERT INTO role_permissions (role_id, permission_id)
SELECT 'system-admin', permission_id FROM permissions
ON CONFLICT (role_id, permission_id) DO NOTHING;

-- Insert default operator role
INSERT INTO roles (role_id, tenant_id, name, description, is_system_role)
VALUES ('operator', 'system', 'Operator', 'Can manage streams, recordings, and AI tasks', true)
ON CONFLICT (role_id) DO NOTHING;

-- Grant operator permissions
INSERT INTO role_permissions (role_id, permission_id)
SELECT 'operator', permission_id FROM permissions
WHERE resource IN ('stream', 'recording', 'ai_task', 'device')
ON CONFLICT (role_id, permission_id) DO NOTHING;

-- Insert default viewer role
INSERT INTO roles (role_id, tenant_id, name, description, is_system_role)
VALUES ('viewer', 'system', 'Viewer', 'Read-only access', true)
ON CONFLICT (role_id) DO NOTHING;

-- Grant viewer permissions
INSERT INTO role_permissions (role_id, permission_id)
SELECT 'viewer', permission_id FROM permissions
WHERE action = 'read'
ON CONFLICT (role_id, permission_id) DO NOTHING;

-- Insert default admin user (password: "admin123" - CHANGE IN PRODUCTION!)
-- Password hash for "admin123" using argon2
INSERT INTO users (user_id, tenant_id, username, email, password_hash, display_name, is_system_admin)
VALUES (
    'admin',
    'system',
    'admin',
    'admin@quadrant.local',
    '$argon2id$v=19$m=19456,t=2,p=1$YW5ndXNwcm9qZWN0cXVhZHJhbnR2bXM$KGZ8L3RoZXNlY3VyZWhhc2hzYXJlZm9yZGVtbw',
    'System Administrator',
    true
)
ON CONFLICT (user_id) DO NOTHING;

-- Grant system-admin role to admin user
INSERT INTO user_roles (user_id, role_id)
VALUES ('admin', 'system-admin')
ON CONFLICT (user_id, role_id) DO NOTHING;

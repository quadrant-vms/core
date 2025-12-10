-- Create OIDC user identities table to link users with external identity providers
CREATE TABLE IF NOT EXISTS oidc_user_identities (
    identity_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    provider_user_id TEXT NOT NULL, -- The 'sub' claim from ID token
    provider_email TEXT, -- Email from provider (may differ from local email)
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(provider_id, provider_user_id),
    FOREIGN KEY (user_id) REFERENCES users(user_id) ON DELETE CASCADE,
    FOREIGN KEY (provider_id) REFERENCES oidc_providers(provider_id) ON DELETE CASCADE
);

-- Indexes for efficient lookups
CREATE INDEX IF NOT EXISTS idx_oidc_identities_user ON oidc_user_identities(user_id);
CREATE INDEX IF NOT EXISTS idx_oidc_identities_provider ON oidc_user_identities(provider_id);
CREATE INDEX IF NOT EXISTS idx_oidc_identities_provider_user ON oidc_user_identities(provider_id, provider_user_id);

-- Trigger for automatic updated_at
CREATE TRIGGER update_oidc_user_identities_updated_at BEFORE UPDATE ON oidc_user_identities
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

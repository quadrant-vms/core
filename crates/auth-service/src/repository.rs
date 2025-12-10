use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::{Pool, Postgres};

use crate::models::*;

pub struct AuthRepository {
    pool: Pool<Postgres>,
}

impl AuthRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    // ===== User Operations =====

    pub async fn create_user(
        &self,
        user_id: String,
        tenant_id: String,
        username: String,
        email: String,
        password_hash: Option<String>,
        display_name: Option<String>,
        is_system_admin: bool,
    ) -> Result<User> {
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (user_id, tenant_id, username, email, password_hash, display_name, is_system_admin)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(tenant_id)
        .bind(username)
        .bind(email)
        .bind(password_hash)
        .bind(display_name)
        .bind(is_system_admin)
        .fetch_one(&self.pool)
        .await
        .context("failed to create user")?;

        Ok(user)
    }

    pub async fn get_user_by_id(&self, user_id: &str) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .context("failed to get user by id")?;

        Ok(user)
    }

    pub async fn get_user_by_username(&self, tenant_id: &str, username: &str) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>(
            "SELECT * FROM users WHERE tenant_id = $1 AND username = $2",
        )
        .bind(tenant_id)
        .bind(username)
        .fetch_optional(&self.pool)
        .await
        .context("failed to get user by username")?;

        Ok(user)
    }

    pub async fn list_users(&self, tenant_id: &str) -> Result<Vec<User>> {
        let users = sqlx::query_as::<_, User>("SELECT * FROM users WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await
            .context("failed to list users")?;

        Ok(users)
    }

    pub async fn update_user_login(&self, user_id: &str) -> Result<()> {
        sqlx::query("UPDATE users SET last_login_at = $1 WHERE user_id = $2")
            .bind(Utc::now())
            .bind(user_id)
            .execute(&self.pool)
            .await
            .context("failed to update user login time")?;

        Ok(())
    }

    pub async fn update_user(
        &self,
        user_id: &str,
        email: Option<String>,
        password_hash: Option<String>,
        display_name: Option<String>,
        is_active: Option<bool>,
    ) -> Result<User> {
        // Build dynamic query based on provided fields
        let mut query = String::from("UPDATE users SET ");
        let mut updates = Vec::new();
        let mut param_count = 1;

        if email.is_some() {
            updates.push(format!("email = ${}", param_count));
            param_count += 1;
        }
        if password_hash.is_some() {
            updates.push(format!("password_hash = ${}", param_count));
            param_count += 1;
        }
        if display_name.is_some() {
            updates.push(format!("display_name = ${}", param_count));
            param_count += 1;
        }
        if is_active.is_some() {
            updates.push(format!("is_active = ${}", param_count));
            param_count += 1;
        }

        if updates.is_empty() {
            return self.get_user_by_id(user_id).await?.context("user not found");
        }

        query.push_str(&updates.join(", "));
        query.push_str(&format!(" WHERE user_id = ${} RETURNING *", param_count));

        let mut q = sqlx::query_as::<_, User>(&query);
        if let Some(e) = email {
            q = q.bind(e);
        }
        if let Some(ph) = password_hash {
            q = q.bind(ph);
        }
        if let Some(dn) = display_name {
            q = q.bind(dn);
        }
        if let Some(ia) = is_active {
            q = q.bind(ia);
        }
        q = q.bind(user_id);

        let user = q.fetch_one(&self.pool).await.context("failed to update user")?;
        Ok(user)
    }

    pub async fn delete_user(&self, user_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM users WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .context("failed to delete user")?;

        Ok(())
    }

    // ===== Role Operations =====

    pub async fn get_user_roles(&self, user_id: &str) -> Result<Vec<Role>> {
        let roles = sqlx::query_as::<_, Role>(
            r#"
            SELECT r.* FROM roles r
            INNER JOIN user_roles ur ON r.role_id = ur.role_id
            WHERE ur.user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .context("failed to get user roles")?;

        Ok(roles)
    }

    pub async fn get_role_by_id(&self, role_id: &str) -> Result<Option<Role>> {
        let role = sqlx::query_as::<_, Role>("SELECT * FROM roles WHERE role_id = $1")
            .bind(role_id)
            .fetch_optional(&self.pool)
            .await
            .context("failed to get role by id")?;

        Ok(role)
    }

    pub async fn list_roles(&self, tenant_id: &str) -> Result<Vec<Role>> {
        let roles = sqlx::query_as::<_, Role>("SELECT * FROM roles WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await
            .context("failed to list roles")?;

        Ok(roles)
    }

    pub async fn create_role(
        &self,
        role_id: String,
        tenant_id: String,
        name: String,
        description: Option<String>,
    ) -> Result<Role> {
        let role = sqlx::query_as::<_, Role>(
            r#"
            INSERT INTO roles (role_id, tenant_id, name, description, is_system_role)
            VALUES ($1, $2, $3, $4, false)
            RETURNING *
            "#,
        )
        .bind(role_id)
        .bind(tenant_id)
        .bind(name)
        .bind(description)
        .fetch_one(&self.pool)
        .await
        .context("failed to create role")?;

        Ok(role)
    }

    pub async fn delete_role(&self, role_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM roles WHERE role_id = $1 AND is_system_role = false")
            .bind(role_id)
            .execute(&self.pool)
            .await
            .context("failed to delete role")?;

        Ok(())
    }

    pub async fn assign_roles_to_user(&self, user_id: &str, role_ids: Vec<String>) -> Result<()> {
        for role_id in role_ids {
            sqlx::query(
                r#"
                INSERT INTO user_roles (user_id, role_id)
                VALUES ($1, $2)
                ON CONFLICT (user_id, role_id) DO NOTHING
                "#,
            )
            .bind(user_id)
            .bind(role_id)
            .execute(&self.pool)
            .await
            .context("failed to assign role to user")?;
        }

        Ok(())
    }

    pub async fn remove_roles_from_user(&self, user_id: &str, role_ids: Vec<String>) -> Result<()> {
        for role_id in role_ids {
            sqlx::query("DELETE FROM user_roles WHERE user_id = $1 AND role_id = $2")
                .bind(user_id)
                .bind(role_id)
                .execute(&self.pool)
                .await
                .context("failed to remove role from user")?;
        }

        Ok(())
    }

    // ===== Permission Operations =====

    pub async fn get_user_permissions(&self, user_id: &str) -> Result<Vec<Permission>> {
        let permissions = sqlx::query_as::<_, Permission>(
            r#"
            SELECT DISTINCT p.* FROM permissions p
            INNER JOIN role_permissions rp ON p.permission_id = rp.permission_id
            INNER JOIN user_roles ur ON rp.role_id = ur.role_id
            WHERE ur.user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .context("failed to get user permissions")?;

        Ok(permissions)
    }

    pub async fn get_role_permissions(&self, role_id: &str) -> Result<Vec<Permission>> {
        let permissions = sqlx::query_as::<_, Permission>(
            r#"
            SELECT p.* FROM permissions p
            INNER JOIN role_permissions rp ON p.permission_id = rp.permission_id
            WHERE rp.role_id = $1
            "#,
        )
        .bind(role_id)
        .fetch_all(&self.pool)
        .await
        .context("failed to get role permissions")?;

        Ok(permissions)
    }

    pub async fn list_permissions(&self) -> Result<Vec<Permission>> {
        let permissions = sqlx::query_as::<_, Permission>("SELECT * FROM permissions ORDER BY resource, action")
            .fetch_all(&self.pool)
            .await
            .context("failed to list permissions")?;

        Ok(permissions)
    }

    pub async fn assign_permissions_to_role(&self, role_id: &str, permission_ids: Vec<String>) -> Result<()> {
        for permission_id in permission_ids {
            sqlx::query(
                r#"
                INSERT INTO role_permissions (role_id, permission_id)
                VALUES ($1, $2)
                ON CONFLICT (role_id, permission_id) DO NOTHING
                "#,
            )
            .bind(role_id)
            .bind(permission_id)
            .execute(&self.pool)
            .await
            .context("failed to assign permission to role")?;
        }

        Ok(())
    }

    pub async fn remove_permissions_from_role(&self, role_id: &str, permission_ids: Vec<String>) -> Result<()> {
        for permission_id in permission_ids {
            sqlx::query("DELETE FROM role_permissions WHERE role_id = $1 AND permission_id = $2")
                .bind(role_id)
                .bind(permission_id)
                .execute(&self.pool)
                .await
                .context("failed to remove permission from role")?;
        }

        Ok(())
    }

    // ===== API Token Operations =====

    pub async fn create_api_token(
        &self,
        token_id: String,
        user_id: String,
        token_hash: String,
        name: String,
        description: Option<String>,
        expires_at: Option<chrono::DateTime<Utc>>,
    ) -> Result<ApiToken> {
        let token = sqlx::query_as::<_, ApiToken>(
            r#"
            INSERT INTO api_tokens (token_id, user_id, token_hash, name, description, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#,
        )
        .bind(token_id)
        .bind(user_id)
        .bind(token_hash)
        .bind(name)
        .bind(description)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await
        .context("failed to create API token")?;

        Ok(token)
    }

    pub async fn get_api_token_by_hash(&self, token_hash: &str) -> Result<Option<ApiToken>> {
        let token = sqlx::query_as::<_, ApiToken>(
            "SELECT * FROM api_tokens WHERE token_hash = $1 AND is_active = true",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
        .context("failed to get API token by hash")?;

        Ok(token)
    }

    pub async fn list_user_api_tokens(&self, user_id: &str) -> Result<Vec<ApiToken>> {
        let tokens = sqlx::query_as::<_, ApiToken>(
            "SELECT * FROM api_tokens WHERE user_id = $1 ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .context("failed to list user API tokens")?;

        Ok(tokens)
    }

    pub async fn update_api_token_last_used(&self, token_id: &str) -> Result<()> {
        sqlx::query("UPDATE api_tokens SET last_used_at = $1 WHERE token_id = $2")
            .bind(Utc::now())
            .bind(token_id)
            .execute(&self.pool)
            .await
            .context("failed to update API token last used time")?;

        Ok(())
    }

    pub async fn revoke_api_token(&self, token_id: &str) -> Result<()> {
        sqlx::query("UPDATE api_tokens SET is_active = false WHERE token_id = $1")
            .bind(token_id)
            .execute(&self.pool)
            .await
            .context("failed to revoke API token")?;

        Ok(())
    }

    // ===== Tenant Operations =====

    pub async fn get_tenant_by_id(&self, tenant_id: &str) -> Result<Option<Tenant>> {
        let tenant = sqlx::query_as::<_, Tenant>("SELECT * FROM tenants WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_optional(&self.pool)
            .await
            .context("failed to get tenant by id")?;

        Ok(tenant)
    }

    pub async fn list_tenants(&self) -> Result<Vec<Tenant>> {
        let tenants = sqlx::query_as::<_, Tenant>("SELECT * FROM tenants ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await
            .context("failed to list tenants")?;

        Ok(tenants)
    }

    pub async fn create_tenant(
        &self,
        tenant_id: String,
        name: String,
        description: Option<String>,
        max_users: Option<i32>,
        max_streams: Option<i32>,
        max_recordings: Option<i32>,
        max_ai_tasks: Option<i32>,
    ) -> Result<Tenant> {
        let tenant = sqlx::query_as::<_, Tenant>(
            r#"
            INSERT INTO tenants (tenant_id, name, description, max_users, max_streams, max_recordings, max_ai_tasks)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *
            "#,
        )
        .bind(tenant_id)
        .bind(name)
        .bind(description)
        .bind(max_users)
        .bind(max_streams)
        .bind(max_recordings)
        .bind(max_ai_tasks)
        .fetch_one(&self.pool)
        .await
        .context("failed to create tenant")?;

        Ok(tenant)
    }

    // ===== Audit Log Operations =====

    pub async fn create_audit_log(&self, req: CreateAuditLogRequest) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO audit_logs (tenant_id, user_id, action, resource_type, resource_id, ip_address, user_agent, status, error_message, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(req.tenant_id)
        .bind(req.user_id)
        .bind(req.action)
        .bind(req.resource_type)
        .bind(req.resource_id)
        .bind(req.ip_address)
        .bind(req.user_agent)
        .bind(req.status)
        .bind(req.error_message)
        .bind(req.metadata)
        .execute(&self.pool)
        .await
        .context("failed to create audit log")?;

        Ok(())
    }

    pub async fn list_audit_logs(&self, tenant_id: &str, limit: i64) -> Result<Vec<AuditLog>> {
        let logs = sqlx::query_as::<_, AuditLog>(
            "SELECT * FROM audit_logs WHERE tenant_id = $1 ORDER BY created_at DESC LIMIT $2",
        )
        .bind(tenant_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .context("failed to list audit logs")?;

        Ok(logs)
    }

    // ===== OIDC Provider Operations =====

    pub async fn create_oidc_provider(
        &self,
        provider_id: String,
        tenant_id: String,
        name: String,
        issuer_url: String,
        client_id: String,
        client_secret: String,
        scopes: Vec<String>,
    ) -> Result<OidcProvider> {
        let provider = sqlx::query_as::<_, OidcProvider>(
            r#"
            INSERT INTO oidc_providers (provider_id, tenant_id, name, issuer_url, client_id, client_secret, scopes)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *
            "#,
        )
        .bind(provider_id)
        .bind(tenant_id)
        .bind(name)
        .bind(issuer_url)
        .bind(client_id)
        .bind(client_secret)
        .bind(&scopes)
        .fetch_one(&self.pool)
        .await
        .context("failed to create OIDC provider")?;

        Ok(provider)
    }

    pub async fn get_oidc_provider_by_id(&self, provider_id: &str) -> Result<Option<OidcProvider>> {
        let provider = sqlx::query_as::<_, OidcProvider>(
            "SELECT * FROM oidc_providers WHERE provider_id = $1",
        )
        .bind(provider_id)
        .fetch_optional(&self.pool)
        .await
        .context("failed to get OIDC provider by id")?;

        Ok(provider)
    }

    pub async fn list_oidc_providers(&self, tenant_id: &str) -> Result<Vec<OidcProvider>> {
        let providers = sqlx::query_as::<_, OidcProvider>(
            "SELECT * FROM oidc_providers WHERE tenant_id = $1 ORDER BY created_at DESC",
        )
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await
        .context("failed to list OIDC providers")?;

        Ok(providers)
    }

    pub async fn update_oidc_provider(
        &self,
        provider_id: &str,
        name: Option<String>,
        issuer_url: Option<String>,
        client_id: Option<String>,
        client_secret: Option<String>,
        scopes: Option<Vec<String>>,
        is_active: Option<bool>,
    ) -> Result<OidcProvider> {
        let provider = sqlx::query_as::<_, OidcProvider>(
            r#"
            UPDATE oidc_providers
            SET
                name = COALESCE($2, name),
                issuer_url = COALESCE($3, issuer_url),
                client_id = COALESCE($4, client_id),
                client_secret = COALESCE($5, client_secret),
                scopes = COALESCE($6, scopes),
                is_active = COALESCE($7, is_active)
            WHERE provider_id = $1
            RETURNING *
            "#,
        )
        .bind(provider_id)
        .bind(name)
        .bind(issuer_url)
        .bind(client_id)
        .bind(client_secret)
        .bind(&scopes)
        .bind(is_active)
        .fetch_one(&self.pool)
        .await
        .context("failed to update OIDC provider")?;

        Ok(provider)
    }

    pub async fn delete_oidc_provider(&self, provider_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM oidc_providers WHERE provider_id = $1")
            .bind(provider_id)
            .execute(&self.pool)
            .await
            .context("failed to delete OIDC provider")?;

        Ok(())
    }

    // ===== OIDC User Identity Operations =====

    pub async fn create_oidc_identity(
        &self,
        identity_id: String,
        user_id: String,
        provider_id: String,
        provider_user_id: String,
        provider_email: Option<String>,
    ) -> Result<OidcUserIdentity> {
        let identity = sqlx::query_as::<_, OidcUserIdentity>(
            r#"
            INSERT INTO oidc_user_identities (identity_id, user_id, provider_id, provider_user_id, provider_email)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(identity_id)
        .bind(user_id)
        .bind(provider_id)
        .bind(provider_user_id)
        .bind(provider_email)
        .fetch_one(&self.pool)
        .await
        .context("failed to create OIDC user identity")?;

        Ok(identity)
    }

    pub async fn get_oidc_identity_by_provider_user(
        &self,
        provider_id: &str,
        provider_user_id: &str,
    ) -> Result<Option<OidcUserIdentity>> {
        let identity = sqlx::query_as::<_, OidcUserIdentity>(
            "SELECT * FROM oidc_user_identities WHERE provider_id = $1 AND provider_user_id = $2",
        )
        .bind(provider_id)
        .bind(provider_user_id)
        .fetch_optional(&self.pool)
        .await
        .context("failed to get OIDC identity by provider user")?;

        Ok(identity)
    }

    pub async fn list_user_oidc_identities(&self, user_id: &str) -> Result<Vec<OidcUserIdentity>> {
        let identities = sqlx::query_as::<_, OidcUserIdentity>(
            "SELECT * FROM oidc_user_identities WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .context("failed to list user OIDC identities")?;

        Ok(identities)
    }

    pub async fn delete_oidc_identity(&self, identity_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM oidc_user_identities WHERE identity_id = $1")
            .bind(identity_id)
            .execute(&self.pool)
            .await
            .context("failed to delete OIDC identity")?;

        Ok(())
    }
}

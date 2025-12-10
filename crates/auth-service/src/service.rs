use anyhow::Result;
use uuid::Uuid;

use crate::{
    config::AuthConfig,
    crypto,
    error::ApiError,
    models::*,
    oidc::{OidcClientManager, OidcUserInfo},
    repository::AuthRepository,
};

pub struct AuthService {
    repo: AuthRepository,
    config: AuthConfig,
    oidc_manager: OidcClientManager,
}

impl AuthService {
    pub fn new(repo: AuthRepository, config: AuthConfig) -> Self {
        Self {
            repo,
            config,
            oidc_manager: OidcClientManager::new(),
        }
    }

    // ===== Authentication =====

    pub async fn login(&self, req: LoginRequest) -> Result<LoginResponse, ApiError> {
        let tenant_id = req.tenant_id.unwrap_or_else(|| "system".to_string());

        // Get user by username
        let user = self
            .repo
            .get_user_by_username(&tenant_id, &req.username)
            .await?
            .ok_or_else(|| ApiError::unauthorized("invalid credentials"))?;

        // Check if user is active
        if !user.is_active {
            return Err(ApiError::unauthorized("user account is disabled"));
        }

        // Verify password
        if let Some(password_hash) = &user.password_hash {
            if !crypto::verify_password(&req.password, password_hash)
                .map_err(|e| ApiError::internal(format!("password verification failed: {}", e)))?
            {
                return Err(ApiError::unauthorized("invalid credentials"));
            }
        } else {
            return Err(ApiError::unauthorized("password authentication not available for this user"));
        }

        // Update last login time
        self.repo.update_user_login(&user.user_id).await?;

        // Get user roles and permissions
        let roles = self.repo.get_user_roles(&user.user_id).await?;
        let permissions = self.repo.get_user_permissions(&user.user_id).await?;

        let role_names: Vec<String> = roles.iter().map(|r| r.name.clone()).collect();
        let permission_ids: Vec<String> = permissions.iter().map(|p| p.permission_id.clone()).collect();

        // Generate JWT token
        let access_token = crypto::generate_jwt(
            &user.user_id,
            &user.tenant_id,
            &user.username,
            user.is_system_admin,
            role_names.clone(),
            permission_ids.clone(),
            &self.config.jwt_secret,
            self.config.jwt_expiration_secs,
        )
        .map_err(|e| ApiError::internal(format!("failed to generate JWT: {}", e)))?;

        Ok(LoginResponse {
            access_token,
            token_type: "Bearer".to_string(),
            expires_in: self.config.jwt_expiration_secs,
            user: UserInfo {
                user_id: user.user_id,
                tenant_id: user.tenant_id,
                username: user.username,
                email: user.email,
                display_name: user.display_name,
                is_system_admin: user.is_system_admin,
                roles: role_names,
                permissions: permission_ids,
            },
        })
    }

    pub async fn verify_token(&self, token: &str) -> Result<JwtClaims, ApiError> {
        crypto::verify_jwt(token, &self.config.jwt_secret)
            .map_err(|_| ApiError::unauthorized("invalid or expired token"))
    }

    pub async fn verify_api_token(&self, token: &str) -> Result<User, ApiError> {
        // Try to find token in database (we need to check all hashes)
        // This is not efficient for large number of tokens, but works for now
        // In production, consider using a token prefix or indexing strategy

        // For simplicity, we'll hash the provided token and look it up
        let token_hash = crypto::hash_api_token(token)
            .map_err(|e| ApiError::internal(format!("failed to hash token: {}", e)))?;

        let api_token = self
            .repo
            .get_api_token_by_hash(&token_hash)
            .await?
            .ok_or_else(|| ApiError::unauthorized("invalid API token"))?;

        // Check expiration
        if let Some(expires_at) = api_token.expires_at {
            if expires_at < chrono::Utc::now() {
                return Err(ApiError::unauthorized("API token expired"));
            }
        }

        // Update last used time
        self.repo.update_api_token_last_used(&api_token.token_id).await?;

        // Get user
        let user = self
            .repo
            .get_user_by_id(&api_token.user_id)
            .await?
            .ok_or_else(|| ApiError::internal("user not found for API token"))?;

        if !user.is_active {
            return Err(ApiError::unauthorized("user account is disabled"));
        }

        Ok(user)
    }

    // ===== User Management =====

    pub async fn create_user(&self, req: CreateUserRequest) -> Result<User, ApiError> {
        let user_id = Uuid::new_v4().to_string();

        let password_hash = if let Some(password) = req.password {
            Some(crypto::hash_password(&password)
                .map_err(|e| ApiError::internal(format!("failed to hash password: {}", e)))?)
        } else {
            None
        };

        let user = self
            .repo
            .create_user(
                user_id,
                req.tenant_id,
                req.username,
                req.email,
                password_hash,
                req.display_name,
                req.is_system_admin.unwrap_or(false),
            )
            .await?;

        Ok(user)
    }

    pub async fn get_user(&self, user_id: &str) -> Result<User, ApiError> {
        self.repo
            .get_user_by_id(user_id)
            .await?
            .ok_or_else(|| ApiError::not_found("user not found"))
    }

    pub async fn list_users(&self, tenant_id: &str) -> Result<Vec<User>, ApiError> {
        self.repo.list_users(tenant_id).await.map_err(Into::into)
    }

    pub async fn update_user(&self, user_id: &str, req: UpdateUserRequest) -> Result<User, ApiError> {
        let password_hash = if let Some(password) = req.password {
            Some(crypto::hash_password(&password)
                .map_err(|e| ApiError::internal(format!("failed to hash password: {}", e)))?)
        } else {
            None
        };

        self.repo
            .update_user(user_id, req.email, password_hash, req.display_name, req.is_active)
            .await
            .map_err(Into::into)
    }

    pub async fn delete_user(&self, user_id: &str) -> Result<(), ApiError> {
        self.repo.delete_user(user_id).await.map_err(Into::into)
    }

    // ===== Role Management =====

    pub async fn create_role(&self, req: CreateRoleRequest) -> Result<Role, ApiError> {
        self.repo
            .create_role(req.role_id, req.tenant_id, req.name, req.description)
            .await
            .map_err(Into::into)
    }

    pub async fn get_role(&self, role_id: &str) -> Result<Role, ApiError> {
        self.repo
            .get_role_by_id(role_id)
            .await?
            .ok_or_else(|| ApiError::not_found("role not found"))
    }

    pub async fn list_roles(&self, tenant_id: &str) -> Result<Vec<Role>, ApiError> {
        self.repo.list_roles(tenant_id).await.map_err(Into::into)
    }

    pub async fn delete_role(&self, role_id: &str) -> Result<(), ApiError> {
        self.repo.delete_role(role_id).await.map_err(Into::into)
    }

    pub async fn get_role_with_permissions(&self, role_id: &str) -> Result<RoleWithPermissions, ApiError> {
        let role = self.get_role(role_id).await?;
        let permissions = self.repo.get_role_permissions(role_id).await?;

        Ok(RoleWithPermissions { role, permissions })
    }

    pub async fn assign_permissions_to_role(&self, role_id: &str, permission_ids: Vec<String>) -> Result<(), ApiError> {
        self.repo
            .assign_permissions_to_role(role_id, permission_ids)
            .await
            .map_err(Into::into)
    }

    pub async fn remove_permissions_from_role(&self, role_id: &str, permission_ids: Vec<String>) -> Result<(), ApiError> {
        self.repo
            .remove_permissions_from_role(role_id, permission_ids)
            .await
            .map_err(Into::into)
    }

    // ===== User-Role Assignment =====

    pub async fn assign_roles_to_user(&self, user_id: &str, role_ids: Vec<String>) -> Result<(), ApiError> {
        self.repo
            .assign_roles_to_user(user_id, role_ids)
            .await
            .map_err(Into::into)
    }

    pub async fn remove_roles_from_user(&self, user_id: &str, role_ids: Vec<String>) -> Result<(), ApiError> {
        self.repo
            .remove_roles_from_user(user_id, role_ids)
            .await
            .map_err(Into::into)
    }

    pub async fn get_user_roles(&self, user_id: &str) -> Result<Vec<Role>, ApiError> {
        self.repo.get_user_roles(user_id).await.map_err(Into::into)
    }

    // ===== Permission Management =====

    pub async fn list_permissions(&self) -> Result<Vec<Permission>, ApiError> {
        self.repo.list_permissions().await.map_err(Into::into)
    }

    // ===== API Token Management =====

    pub async fn create_api_token(
        &self,
        user_id: &str,
        req: CreateApiTokenRequest,
    ) -> Result<CreateApiTokenResponse, ApiError> {
        let token_id = Uuid::new_v4().to_string();
        let token = crypto::generate_api_token();
        let token_hash = crypto::hash_api_token(&token)
            .map_err(|e| ApiError::internal(format!("failed to hash token: {}", e)))?;

        let api_token = self
            .repo
            .create_api_token(
                token_id.clone(),
                user_id.to_string(),
                token_hash,
                req.name,
                req.description,
                req.expires_at,
            )
            .await?;

        Ok(CreateApiTokenResponse {
            token_id: api_token.token_id,
            token, // Return plain text token (only time it's visible)
            expires_at: api_token.expires_at,
        })
    }

    pub async fn list_user_api_tokens(&self, user_id: &str) -> Result<Vec<ApiToken>, ApiError> {
        self.repo.list_user_api_tokens(user_id).await.map_err(Into::into)
    }

    pub async fn revoke_api_token(&self, token_id: &str) -> Result<(), ApiError> {
        self.repo.revoke_api_token(token_id).await.map_err(Into::into)
    }

    // ===== Tenant Management =====

    pub async fn create_tenant(&self, req: CreateTenantRequest) -> Result<Tenant, ApiError> {
        self.repo
            .create_tenant(
                req.tenant_id,
                req.name,
                req.description,
                req.max_users,
                req.max_streams,
                req.max_recordings,
                req.max_ai_tasks,
            )
            .await
            .map_err(Into::into)
    }

    pub async fn get_tenant(&self, tenant_id: &str) -> Result<Tenant, ApiError> {
        self.repo
            .get_tenant_by_id(tenant_id)
            .await?
            .ok_or_else(|| ApiError::not_found("tenant not found"))
    }

    pub async fn list_tenants(&self) -> Result<Vec<Tenant>, ApiError> {
        self.repo.list_tenants().await.map_err(Into::into)
    }

    // ===== Audit Logging =====

    pub async fn log_audit(&self, req: CreateAuditLogRequest) -> Result<(), ApiError> {
        self.repo.create_audit_log(req).await.map_err(Into::into)
    }

    pub async fn list_audit_logs(&self, tenant_id: &str, limit: i64) -> Result<Vec<AuditLog>, ApiError> {
        self.repo.list_audit_logs(tenant_id, limit).await.map_err(Into::into)
    }

    // ===== OIDC Provider Management =====

    pub async fn create_oidc_provider(&self, req: CreateOidcProviderRequest) -> Result<OidcProvider, ApiError> {
        let scopes = req.scopes.unwrap_or_else(|| {
            vec!["openid".to_string(), "profile".to_string(), "email".to_string()]
        });

        self.repo
            .create_oidc_provider(
                req.provider_id,
                req.tenant_id,
                req.name,
                req.issuer_url,
                req.client_id,
                req.client_secret,
                scopes,
            )
            .await
            .map_err(Into::into)
    }

    pub async fn get_oidc_provider(&self, provider_id: &str) -> Result<OidcProvider, ApiError> {
        self.repo
            .get_oidc_provider_by_id(provider_id)
            .await?
            .ok_or_else(|| ApiError::not_found("OIDC provider not found"))
    }

    pub async fn list_oidc_providers(&self, tenant_id: &str) -> Result<Vec<OidcProvider>, ApiError> {
        self.repo.list_oidc_providers(tenant_id).await.map_err(Into::into)
    }

    pub async fn update_oidc_provider(
        &self,
        provider_id: &str,
        req: UpdateOidcProviderRequest,
    ) -> Result<OidcProvider, ApiError> {
        let provider = self.repo
            .update_oidc_provider(
                provider_id,
                req.name,
                req.issuer_url,
                req.client_id,
                req.client_secret,
                req.scopes,
                req.is_active,
            )
            .await?;

        // Invalidate cached client when provider config changes
        self.oidc_manager.invalidate_client(provider_id);

        Ok(provider)
    }

    pub async fn delete_oidc_provider(&self, provider_id: &str) -> Result<(), ApiError> {
        self.repo.delete_oidc_provider(provider_id).await?;
        self.oidc_manager.invalidate_client(provider_id);
        Ok(())
    }

    // ===== OIDC Authentication Flow =====

    pub async fn initiate_oidc_login(
        &self,
        provider_id: &str,
        redirect_uri: &str,
    ) -> Result<OidcLoginResponse, ApiError> {
        let provider = self.get_oidc_provider(provider_id).await?;

        if !provider.is_active {
            return Err(ApiError::bad_request("OIDC provider is not active"));
        }

        let (auth_url, state) = self
            .oidc_manager
            .generate_authorization_url(&provider, redirect_uri)
            .await?;

        Ok(OidcLoginResponse {
            authorization_url: auth_url,
            state,
        })
    }

    pub async fn handle_oidc_callback(
        &self,
        provider_id: &str,
        code: &str,
        state: &str,
        redirect_uri: &str,
    ) -> Result<LoginResponse, ApiError> {
        let provider = self.get_oidc_provider(provider_id).await?;

        if !provider.is_active {
            return Err(ApiError::bad_request("OIDC provider is not active"));
        }

        // Exchange code for tokens and get user info
        let user_info = self
            .oidc_manager
            .exchange_code(&provider, code, state, redirect_uri)
            .await?;

        // Provision or find user
        let user = self.provision_oidc_user(&provider, &user_info).await?;

        // Get user roles and permissions
        let roles = self.repo.get_user_roles(&user.user_id).await?;
        let permissions = self.repo.get_user_permissions(&user.user_id).await?;

        let role_names: Vec<String> = roles.iter().map(|r| r.name.clone()).collect();
        let permission_ids: Vec<String> = permissions.iter().map(|p| p.permission_id.clone()).collect();

        // Generate JWT token
        let access_token = crypto::generate_jwt(
            &user.user_id,
            &user.tenant_id,
            &user.username,
            user.is_system_admin,
            role_names.clone(),
            permission_ids.clone(),
            &self.config.jwt_secret,
            self.config.jwt_expiration_secs,
        )
        .map_err(|e| ApiError::internal(format!("failed to generate JWT: {}", e)))?;

        // Update last login time
        self.repo.update_user_login(&user.user_id).await?;

        Ok(LoginResponse {
            access_token,
            token_type: "Bearer".to_string(),
            expires_in: self.config.jwt_expiration_secs,
            user: UserInfo {
                user_id: user.user_id,
                tenant_id: user.tenant_id,
                username: user.username,
                email: user.email,
                display_name: user.display_name,
                is_system_admin: user.is_system_admin,
                roles: role_names,
                permissions: permission_ids,
            },
        })
    }

    // ===== OIDC User Provisioning =====

    async fn provision_oidc_user(
        &self,
        provider: &OidcProvider,
        user_info: &OidcUserInfo,
    ) -> Result<User, ApiError> {
        // Check if user identity already exists
        if let Some(identity) = self
            .repo
            .get_oidc_identity_by_provider_user(&provider.provider_id, &user_info.sub)
            .await?
        {
            // User already exists, return it
            return self
                .repo
                .get_user_by_id(&identity.user_id)
                .await?
                .ok_or_else(|| ApiError::internal("user not found for OIDC identity"));
        }

        // Auto-provision new user
        let user_id = Uuid::new_v4().to_string();
        let username = user_info.email.clone().unwrap_or_else(|| {
            format!("{}_{}", provider.provider_id, user_info.sub)
        });
        let email = user_info.email.clone().unwrap_or_else(|| {
            format!("{}@{}.oidc", user_info.sub, provider.provider_id)
        });
        let display_name = user_info.name.clone();

        // Create user without password (SSO-only)
        let user = self
            .repo
            .create_user(
                user_id.clone(),
                provider.tenant_id.clone(),
                username,
                email.clone(),
                None, // No password for SSO users
                display_name,
                false, // Not a system admin by default
            )
            .await?;

        // Create OIDC identity link
        let identity_id = Uuid::new_v4().to_string();
        self.repo
            .create_oidc_identity(
                identity_id,
                user_id.clone(),
                provider.provider_id.clone(),
                user_info.sub.clone(),
                user_info.email.clone(),
            )
            .await?;

        // Assign default viewer role
        self.repo
            .assign_roles_to_user(&user_id, vec!["viewer".to_string()])
            .await?;

        Ok(user)
    }

    // ===== OIDC Identity Management =====

    pub async fn list_user_oidc_identities(&self, user_id: &str) -> Result<Vec<OidcUserIdentity>, ApiError> {
        self.repo
            .list_user_oidc_identities(user_id)
            .await
            .map_err(Into::into)
    }

    pub async fn delete_oidc_identity(&self, identity_id: &str) -> Result<(), ApiError> {
        self.repo.delete_oidc_identity(identity_id).await.map_err(Into::into)
    }
}

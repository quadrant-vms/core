use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

// ===== Tenant Models =====

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Tenant {
    pub tenant_id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub max_users: Option<i32>,
    pub max_streams: Option<i32>,
    pub max_recordings: Option<i32>,
    pub max_ai_tasks: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTenantRequest {
    pub tenant_id: String,
    pub name: String,
    pub description: Option<String>,
    pub max_users: Option<i32>,
    pub max_streams: Option<i32>,
    pub max_recordings: Option<i32>,
    pub max_ai_tasks: Option<i32>,
}

// ===== User Models =====

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub user_id: String,
    pub tenant_id: String,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: Option<String>,
    pub display_name: Option<String>,
    pub is_active: bool,
    pub is_system_admin: bool,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub tenant_id: String,
    pub username: String,
    pub email: String,
    pub password: Option<String>,
    pub display_name: Option<String>,
    pub is_system_admin: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub email: Option<String>,
    pub password: Option<String>,
    pub display_name: Option<String>,
    pub is_active: Option<bool>,
}

// ===== Role Models =====

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Role {
    pub role_id: String,
    pub tenant_id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_system_role: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRoleRequest {
    pub role_id: String,
    pub tenant_id: String,
    pub name: String,
    pub description: Option<String>,
}

// ===== Permission Models =====

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Permission {
    pub permission_id: String,
    pub resource: String,
    pub action: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RoleWithPermissions {
    #[serde(flatten)]
    pub role: Role,
    pub permissions: Vec<Permission>,
}

#[derive(Debug, Deserialize)]
pub struct AssignPermissionsRequest {
    pub permission_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct AssignRolesRequest {
    pub role_ids: Vec<String>,
}

// ===== API Token Models =====

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApiToken {
    pub token_id: String,
    pub user_id: String,
    #[serde(skip_serializing)]
    pub token_hash: String,
    pub name: String,
    pub description: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateApiTokenRequest {
    pub name: String,
    pub description: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct CreateApiTokenResponse {
    pub token_id: String,
    pub token: String, // Plain text token (only returned once)
    pub expires_at: Option<DateTime<Utc>>,
}

// ===== OIDC Provider Models =====

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OidcProvider {
    pub provider_id: String,
    pub tenant_id: String,
    pub name: String,
    pub issuer_url: String,
    pub client_id: String,
    #[serde(skip_serializing)]
    pub client_secret: String,
    pub scopes: Vec<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateOidcProviderRequest {
    pub provider_id: String,
    pub tenant_id: String,
    pub name: String,
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub scopes: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateOidcProviderRequest {
    pub name: Option<String>,
    pub issuer_url: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub scopes: Option<Vec<String>>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OidcUserIdentity {
    pub identity_id: String,
    pub user_id: String,
    pub provider_id: String,
    pub provider_user_id: String,
    pub provider_email: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct OidcLoginRequest {
    pub provider_id: String,
    pub redirect_uri: String,
}

#[derive(Debug, Serialize)]
pub struct OidcLoginResponse {
    pub authorization_url: String,
    pub state: String, // CSRF protection token
}

#[derive(Debug, Deserialize)]
pub struct OidcCallbackRequest {
    pub code: String,
    pub state: String,
    pub redirect_uri: String,
}

// ===== Authentication Models =====

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    pub tenant_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub user: UserInfo,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub user_id: String,
    pub tenant_id: String,
    pub username: String,
    pub email: String,
    pub display_name: Option<String>,
    pub is_system_admin: bool,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
}

// ===== JWT Claims =====

#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: String,      // user_id
    pub tenant_id: String,
    pub username: String,
    pub is_system_admin: bool,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub exp: i64,         // Expiration time (UNIX timestamp)
    pub iat: i64,         // Issued at (UNIX timestamp)
}

// ===== Audit Log Models =====

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuditLog {
    pub log_id: i64,
    pub tenant_id: String,
    pub user_id: Option<String>,
    pub action: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAuditLogRequest {
    pub tenant_id: String,
    pub user_id: Option<String>,
    pub action: String,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

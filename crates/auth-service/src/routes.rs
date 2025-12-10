use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};

use crate::{
    error::ApiError,
    models::*,
    state::AuthState,
};

pub fn router(state: AuthState) -> Router {
    Router::new()
        // Health and metrics
        .route("/healthz", get(healthz))
        .route("/metrics", get(metrics))
        // Authentication
        .route("/v1/auth/login", post(login))
        .route("/v1/auth/verify", post(verify_token))
        // OIDC Authentication
        .route("/v1/auth/oidc/:provider_id/login", get(oidc_login))
        .route("/v1/auth/oidc/:provider_id/callback", post(oidc_callback))
        // Users
        .route("/v1/users", get(list_users).post(create_user))
        .route("/v1/users/:id", get(get_user).put(update_user).delete(delete_user))
        .route("/v1/users/:id/roles", get(get_user_roles).post(assign_user_roles).delete(remove_user_roles))
        .route("/v1/users/:id/tokens", get(list_user_tokens).post(create_user_token))
        .route("/v1/users/:id/oidc-identities", get(list_user_oidc_identities))
        // Roles
        .route("/v1/roles", get(list_roles).post(create_role))
        .route("/v1/roles/:id", get(get_role).delete(delete_role))
        .route("/v1/roles/:id/permissions", get(get_role_permissions).post(assign_role_permissions).delete(remove_role_permissions))
        // Permissions
        .route("/v1/permissions", get(list_permissions))
        // Tenants
        .route("/v1/tenants", get(list_tenants).post(create_tenant))
        .route("/v1/tenants/:id", get(get_tenant))
        // API Tokens
        .route("/v1/tokens/:id/revoke", post(revoke_token))
        // OIDC Providers
        .route("/v1/oidc/providers", get(list_oidc_providers).post(create_oidc_provider))
        .route("/v1/oidc/providers/:id", get(get_oidc_provider).put(update_oidc_provider).delete(delete_oidc_provider))
        // Audit logs
        .route("/v1/audit-logs", get(list_audit_logs))
        .with_state(state)
}

// ===== Health & Metrics =====

async fn healthz() -> &'static str {
    "ok"
}

async fn metrics() -> Result<String, ApiError> {
    telemetry::metrics::encode_metrics()
        .map_err(|e| ApiError::internal(format!("failed to encode metrics: {}", e)))
}

// ===== Authentication =====

async fn login(
    State(state): State<AuthState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    let service = state.service();
    let response = service.login(req).await?;
    Ok(Json(response))
}

#[derive(serde::Deserialize)]
struct VerifyTokenRequest {
    token: String,
}

#[derive(serde::Serialize)]
struct VerifyTokenResponse {
    valid: bool,
    claims: Option<JwtClaims>,
}

async fn verify_token(
    State(state): State<AuthState>,
    Json(req): Json<VerifyTokenRequest>,
) -> Result<Json<VerifyTokenResponse>, ApiError> {
    let service = state.service();
    match service.verify_token(&req.token).await {
        Ok(claims) => Ok(Json(VerifyTokenResponse {
            valid: true,
            claims: Some(claims),
        })),
        Err(_) => Ok(Json(VerifyTokenResponse {
            valid: false,
            claims: None,
        })),
    }
}

// ===== User Management =====

#[derive(serde::Deserialize)]
struct ListUsersQuery {
    tenant_id: Option<String>,
}

async fn list_users(
    State(state): State<AuthState>,
    axum::extract::Query(query): axum::extract::Query<ListUsersQuery>,
) -> Result<Json<Vec<User>>, ApiError> {
    let tenant_id = query.tenant_id.unwrap_or_else(|| "system".to_string());
    let service = state.service();
    let users = service.list_users(&tenant_id).await?;
    Ok(Json(users))
}

async fn create_user(
    State(state): State<AuthState>,
    Json(req): Json<CreateUserRequest>,
) -> Result<Json<User>, ApiError> {
    let service = state.service();
    let user = service.create_user(req).await?;
    Ok(Json(user))
}

async fn get_user(
    State(state): State<AuthState>,
    Path(user_id): Path<String>,
) -> Result<Json<User>, ApiError> {
    let service = state.service();
    let user = service.get_user(&user_id).await?;
    Ok(Json(user))
}

async fn update_user(
    State(state): State<AuthState>,
    Path(user_id): Path<String>,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<User>, ApiError> {
    let service = state.service();
    let user = service.update_user(&user_id, req).await?;
    Ok(Json(user))
}

async fn delete_user(
    State(state): State<AuthState>,
    Path(user_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let service = state.service();
    service.delete_user(&user_id).await?;
    Ok(Json(serde_json::json!({"status": "deleted"})))
}

// ===== Role Management =====

#[derive(serde::Deserialize)]
struct ListRolesQuery {
    tenant_id: Option<String>,
}

async fn list_roles(
    State(state): State<AuthState>,
    axum::extract::Query(query): axum::extract::Query<ListRolesQuery>,
) -> Result<Json<Vec<Role>>, ApiError> {
    let tenant_id = query.tenant_id.unwrap_or_else(|| "system".to_string());
    let service = state.service();
    let roles = service.list_roles(&tenant_id).await?;
    Ok(Json(roles))
}

async fn create_role(
    State(state): State<AuthState>,
    Json(req): Json<CreateRoleRequest>,
) -> Result<Json<Role>, ApiError> {
    let service = state.service();
    let role = service.create_role(req).await?;
    Ok(Json(role))
}

async fn get_role(
    State(state): State<AuthState>,
    Path(role_id): Path<String>,
) -> Result<Json<RoleWithPermissions>, ApiError> {
    let service = state.service();
    let role = service.get_role_with_permissions(&role_id).await?;
    Ok(Json(role))
}

async fn delete_role(
    State(state): State<AuthState>,
    Path(role_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let service = state.service();
    service.delete_role(&role_id).await?;
    Ok(Json(serde_json::json!({"status": "deleted"})))
}

// ===== User-Role Assignment =====

async fn get_user_roles(
    State(state): State<AuthState>,
    Path(user_id): Path<String>,
) -> Result<Json<Vec<Role>>, ApiError> {
    let service = state.service();
    let roles = service.get_user_roles(&user_id).await?;
    Ok(Json(roles))
}

async fn assign_user_roles(
    State(state): State<AuthState>,
    Path(user_id): Path<String>,
    Json(req): Json<AssignRolesRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let service = state.service();
    service.assign_roles_to_user(&user_id, req.role_ids).await?;
    Ok(Json(serde_json::json!({"status": "assigned"})))
}

async fn remove_user_roles(
    State(state): State<AuthState>,
    Path(user_id): Path<String>,
    Json(req): Json<AssignRolesRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let service = state.service();
    service.remove_roles_from_user(&user_id, req.role_ids).await?;
    Ok(Json(serde_json::json!({"status": "removed"})))
}

// ===== Role-Permission Assignment =====

async fn get_role_permissions(
    State(state): State<AuthState>,
    Path(role_id): Path<String>,
) -> Result<Json<Vec<Permission>>, ApiError> {
    let service = state.service();
    let role = service.get_role_with_permissions(&role_id).await?;
    Ok(Json(role.permissions))
}

async fn assign_role_permissions(
    State(state): State<AuthState>,
    Path(role_id): Path<String>,
    Json(req): Json<AssignPermissionsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let service = state.service();
    service.assign_permissions_to_role(&role_id, req.permission_ids).await?;
    Ok(Json(serde_json::json!({"status": "assigned"})))
}

async fn remove_role_permissions(
    State(state): State<AuthState>,
    Path(role_id): Path<String>,
    Json(req): Json<AssignPermissionsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let service = state.service();
    service.remove_permissions_from_role(&role_id, req.permission_ids).await?;
    Ok(Json(serde_json::json!({"status": "removed"})))
}

// ===== Permission Management =====

async fn list_permissions(State(state): State<AuthState>) -> Result<Json<Vec<Permission>>, ApiError> {
    let service = state.service();
    let permissions = service.list_permissions().await?;
    Ok(Json(permissions))
}

// ===== API Token Management =====

async fn list_user_tokens(
    State(state): State<AuthState>,
    Path(user_id): Path<String>,
) -> Result<Json<Vec<ApiToken>>, ApiError> {
    let service = state.service();
    let tokens = service.list_user_api_tokens(&user_id).await?;
    Ok(Json(tokens))
}

async fn create_user_token(
    State(state): State<AuthState>,
    Path(user_id): Path<String>,
    Json(req): Json<CreateApiTokenRequest>,
) -> Result<Json<CreateApiTokenResponse>, ApiError> {
    let service = state.service();
    let response = service.create_api_token(&user_id, req).await?;
    Ok(Json(response))
}

async fn revoke_token(
    State(state): State<AuthState>,
    Path(token_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let service = state.service();
    service.revoke_api_token(&token_id).await?;
    Ok(Json(serde_json::json!({"status": "revoked"})))
}

// ===== Tenant Management =====

async fn list_tenants(State(state): State<AuthState>) -> Result<Json<Vec<Tenant>>, ApiError> {
    let service = state.service();
    let tenants = service.list_tenants().await?;
    Ok(Json(tenants))
}

async fn create_tenant(
    State(state): State<AuthState>,
    Json(req): Json<CreateTenantRequest>,
) -> Result<Json<Tenant>, ApiError> {
    let service = state.service();
    let tenant = service.create_tenant(req).await?;
    Ok(Json(tenant))
}

async fn get_tenant(
    State(state): State<AuthState>,
    Path(tenant_id): Path<String>,
) -> Result<Json<Tenant>, ApiError> {
    let service = state.service();
    let tenant = service.get_tenant(&tenant_id).await?;
    Ok(Json(tenant))
}

// ===== Audit Logs =====

#[derive(serde::Deserialize)]
struct ListAuditLogsQuery {
    tenant_id: Option<String>,
    limit: Option<i64>,
}

async fn list_audit_logs(
    State(state): State<AuthState>,
    axum::extract::Query(query): axum::extract::Query<ListAuditLogsQuery>,
) -> Result<Json<Vec<AuditLog>>, ApiError> {
    let tenant_id = query.tenant_id.unwrap_or_else(|| "system".to_string());
    let limit = query.limit.unwrap_or(100).min(1000); // Max 1000 logs
    let service = state.service();
    let logs = service.list_audit_logs(&tenant_id, limit).await?;
    Ok(Json(logs))
}

// ===== OIDC Provider Management =====

#[derive(serde::Deserialize)]
struct ListOidcProvidersQuery {
    tenant_id: Option<String>,
}

async fn list_oidc_providers(
    State(state): State<AuthState>,
    axum::extract::Query(query): axum::extract::Query<ListOidcProvidersQuery>,
) -> Result<Json<Vec<OidcProvider>>, ApiError> {
    let tenant_id = query.tenant_id.unwrap_or_else(|| "system".to_string());
    let service = state.service();
    let providers = service.list_oidc_providers(&tenant_id).await?;
    Ok(Json(providers))
}

async fn create_oidc_provider(
    State(state): State<AuthState>,
    Json(req): Json<CreateOidcProviderRequest>,
) -> Result<Json<OidcProvider>, ApiError> {
    let service = state.service();
    let provider = service.create_oidc_provider(req).await?;
    Ok(Json(provider))
}

async fn get_oidc_provider(
    State(state): State<AuthState>,
    Path(provider_id): Path<String>,
) -> Result<Json<OidcProvider>, ApiError> {
    let service = state.service();
    let provider = service.get_oidc_provider(&provider_id).await?;
    Ok(Json(provider))
}

async fn update_oidc_provider(
    State(state): State<AuthState>,
    Path(provider_id): Path<String>,
    Json(req): Json<UpdateOidcProviderRequest>,
) -> Result<Json<OidcProvider>, ApiError> {
    let service = state.service();
    let provider = service.update_oidc_provider(&provider_id, req).await?;
    Ok(Json(provider))
}

async fn delete_oidc_provider(
    State(state): State<AuthState>,
    Path(provider_id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let service = state.service();
    service.delete_oidc_provider(&provider_id).await?;
    Ok(Json(serde_json::json!({"message": "OIDC provider deleted"})))
}

// ===== OIDC Authentication Flow =====

#[derive(serde::Deserialize)]
struct OidcLoginQuery {
    redirect_uri: String,
}

async fn oidc_login(
    State(state): State<AuthState>,
    Path(provider_id): Path<String>,
    axum::extract::Query(query): axum::extract::Query<OidcLoginQuery>,
) -> Result<Json<OidcLoginResponse>, ApiError> {
    let service = state.service();
    let response = service
        .initiate_oidc_login(&provider_id, &query.redirect_uri)
        .await?;
    Ok(Json(response))
}

async fn oidc_callback(
    State(state): State<AuthState>,
    Path(provider_id): Path<String>,
    Json(req): Json<OidcCallbackRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    let service = state.service();
    let response = service
        .handle_oidc_callback(&provider_id, &req.code, &req.state, &req.redirect_uri)
        .await?;
    Ok(Json(response))
}

// ===== OIDC Identity Management =====

async fn list_user_oidc_identities(
    State(state): State<AuthState>,
    Path(user_id): Path<String>,
) -> Result<Json<Vec<OidcUserIdentity>>, ApiError> {
    let service = state.service();
    let identities = service.list_user_oidc_identities(&user_id).await?;
    Ok(Json(identities))
}

use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// JWT Claims structure matching auth-service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthClaims {
    pub sub: String,      // user_id
    pub tenant_id: String,
    pub username: String,
    pub is_system_admin: bool,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub exp: i64,
    pub iat: i64,
}

/// Authentication context passed to request handlers
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: String,
    pub tenant_id: String,
    pub username: String,
    pub is_system_admin: bool,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
}

impl AuthContext {
    /// Check if user has a specific permission
    pub fn has_permission(&self, permission: &str) -> bool {
        self.is_system_admin || self.permissions.iter().any(|p| p == permission)
    }

    /// Check if user has a specific role
    pub fn has_role(&self, role: &str) -> bool {
        self.is_system_admin || self.roles.iter().any(|r| r == role)
    }

    /// Check if user has any of the specified permissions
    pub fn has_any_permission(&self, permissions: &[&str]) -> bool {
        self.is_system_admin || permissions.iter().any(|p| self.has_permission(p))
    }

    /// Check if user has all of the specified permissions
    pub fn has_all_permissions(&self, permissions: &[&str]) -> bool {
        self.is_system_admin || permissions.iter().all(|p| self.has_permission(p))
    }
}

/// Auth middleware configuration
#[derive(Clone)]
pub struct AuthMiddlewareConfig {
    pub auth_service_url: String,
    pub jwt_secret: String,
    pub required_permissions: Vec<String>,
}

impl AuthMiddlewareConfig {
    pub fn new(auth_service_url: String, jwt_secret: String) -> Self {
        Self {
            auth_service_url,
            jwt_secret,
            required_permissions: Vec::new(),
        }
    }

    pub fn with_permissions(mut self, permissions: Vec<String>) -> Self {
        self.required_permissions = permissions;
        self
    }
}

/// Extract auth token from request headers
fn extract_token(req: &Request) -> Option<String> {
    let auth_header = req.headers().get(header::AUTHORIZATION)?;
    let auth_str = auth_header.to_str().ok()?;

    if auth_str.starts_with("Bearer ") {
        Some(auth_str[7..].to_string())
    } else {
        None
    }
}

/// Verify JWT token locally (without calling auth-service)
fn verify_jwt_local(token: &str, jwt_secret: &str) -> Result<AuthClaims, String> {
    use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};

    let token_data = decode::<AuthClaims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|e| format!("Invalid JWT: {}", e))?;

    Ok(token_data.claims)
}

/// Authentication middleware that verifies JWT tokens
pub async fn auth_middleware(
    State(config): State<Arc<AuthMiddlewareConfig>>,
    mut req: Request,
    next: Next,
) -> Result<Response, Response> {
    // Extract token from Authorization header
    let token = extract_token(&req).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Missing or invalid Authorization header" })),
        )
            .into_response()
    })?;

    // Verify JWT token locally
    let claims = verify_jwt_local(&token, &config.jwt_secret).map_err(|e| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response()
    })?;

    // Create auth context
    let auth_ctx = AuthContext {
        user_id: claims.sub.clone(),
        tenant_id: claims.tenant_id.clone(),
        username: claims.username.clone(),
        is_system_admin: claims.is_system_admin,
        roles: claims.roles.clone(),
        permissions: claims.permissions.clone(),
    };

    // Check required permissions
    if !config.required_permissions.is_empty() {
        let has_permission = auth_ctx.has_any_permission(
            &config
                .required_permissions
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>(),
        );

        if !has_permission {
            return Err((
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({ "error": "Insufficient permissions" })),
            )
                .into_response());
        }
    }

    // Add auth context to request extensions
    req.extensions_mut().insert(auth_ctx);

    Ok(next.run(req).await)
}

/// Extract AuthContext from request extensions
/// Use this in your route handlers to access authentication info
pub trait AuthContextExt {
    fn auth_context(&self) -> Option<&AuthContext>;
}

impl AuthContextExt for Request<Body> {
    fn auth_context(&self) -> Option<&AuthContext> {
        self.extensions().get::<AuthContext>()
    }
}

/// Helper macro to require authentication and extract context
#[macro_export]
macro_rules! require_auth {
    ($req:expr) => {{
        use $crate::auth_middleware::AuthContextExt;
        $req.auth_context()
            .ok_or_else(|| {
                axum::http::StatusCode::UNAUTHORIZED
            })?
    }};
}

/// Helper to check permission and return 403 if not authorized
pub fn require_permission(ctx: &AuthContext, permission: &str) -> Result<(), Response> {
    if !ctx.has_permission(permission) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": format!("Permission '{}' required", permission) })),
        )
            .into_response());
    }
    Ok(())
}

/// Axum extractor for requiring authentication
/// Usage: `RequireAuth(auth_ctx): RequireAuth` in route handlers
pub struct RequireAuth(pub AuthContext);

#[axum::async_trait]
impl<S> axum::extract::FromRequestParts<S> for RequireAuth
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthContext>()
            .cloned()
            .map(RequireAuth)
            .ok_or_else(|| {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({ "error": "Authentication required" })),
                )
                    .into_response()
            })
    }
}

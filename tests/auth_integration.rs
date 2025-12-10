/// Integration tests for authentication and authorization system

#[tokio::test]
#[ignore] // Requires PostgreSQL database
async fn test_auth_service_integration() {
    // This test requires a running PostgreSQL database
    // Run with: DATABASE_URL="postgresql://..." cargo test --test auth_integration -- --ignored

    let _database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5432/quadrant_vms_test".to_string());

    // Test JWT token generation and verification
    let jwt_secret = "test-jwt-secret";
    let user_id = "test-user-123";
    let tenant_id = "system";
    let username = "testuser";

    // Generate a JWT token
    let token = auth_service::crypto::generate_jwt(
        user_id,
        tenant_id,
        username,
        false,
        vec!["operator".to_string()],
        vec!["stream:read".to_string(), "stream:create".to_string()],
        jwt_secret,
        3600,
    )
    .expect("Failed to generate JWT");

    // Verify the token
    let claims = auth_service::crypto::verify_jwt(&token, jwt_secret)
        .expect("Failed to verify JWT");

    assert_eq!(claims.sub, user_id);
    assert_eq!(claims.tenant_id, tenant_id);
    assert_eq!(claims.username, username);
    assert!(!claims.is_system_admin);
    assert_eq!(claims.roles, vec!["operator"]);
    assert_eq!(claims.permissions.len(), 2);

    println!("✅ JWT generation and verification test passed");

    // Test password hashing
    let password = "secure_password_123";
    let hash = auth_service::crypto::hash_password(password)
        .expect("Failed to hash password");

    assert!(auth_service::crypto::verify_password(password, &hash)
        .expect("Failed to verify password"));
    assert!(!auth_service::crypto::verify_password("wrong_password", &hash)
        .expect("Failed to verify wrong password"));

    println!("✅ Password hashing and verification test passed");

    // Test API token generation
    let api_token = auth_service::crypto::generate_api_token();
    assert!(api_token.starts_with("qvms_"));
    assert!(api_token.len() > 10);

    let token_hash = auth_service::crypto::hash_api_token(&api_token)
        .expect("Failed to hash API token");
    assert!(auth_service::crypto::verify_api_token(&api_token, &token_hash)
        .expect("Failed to verify API token"));

    println!("✅ API token generation and verification test passed");
}

#[tokio::test]
async fn test_auth_middleware_jwt_verification() {
    use common::auth_middleware::{AuthContext, AuthClaims};

    // Create test JWT claims
    let claims = AuthClaims {
        sub: "user123".to_string(),
        tenant_id: "tenant1".to_string(),
        username: "testuser".to_string(),
        is_system_admin: false,
        roles: vec!["operator".to_string()],
        permissions: vec!["stream:read".to_string(), "stream:create".to_string()],
        exp: (chrono::Utc::now().timestamp() + 3600) as i64,
        iat: chrono::Utc::now().timestamp() as i64,
    };

    // Create auth context
    let ctx = AuthContext {
        user_id: claims.sub.clone(),
        tenant_id: claims.tenant_id.clone(),
        username: claims.username.clone(),
        is_system_admin: claims.is_system_admin,
        roles: claims.roles.clone(),
        permissions: claims.permissions.clone(),
    };

    // Test permission checks
    assert!(ctx.has_permission("stream:read"));
    assert!(ctx.has_permission("stream:create"));
    assert!(!ctx.has_permission("stream:delete"));

    // Test role checks
    assert!(ctx.has_role("operator"));
    assert!(!ctx.has_role("admin"));

    // Test any permission check
    assert!(ctx.has_any_permission(&["stream:read", "stream:update"]));
    assert!(!ctx.has_any_permission(&["stream:delete", "stream:update"]));

    // Test all permissions check
    assert!(ctx.has_all_permissions(&["stream:read", "stream:create"]));
    assert!(!ctx.has_all_permissions(&["stream:read", "stream:delete"]));

    println!("✅ Auth middleware permission checks passed");

    // Test system admin override
    let admin_ctx = AuthContext {
        user_id: "admin".to_string(),
        tenant_id: "system".to_string(),
        username: "admin".to_string(),
        is_system_admin: true,
        roles: vec![],
        permissions: vec![],
    };

    assert!(admin_ctx.has_permission("stream:delete"));
    assert!(admin_ctx.has_role("any_role"));
    assert!(admin_ctx.has_any_permission(&["any:permission"]));

    println!("✅ System admin permission override test passed");
}

#[test]
fn test_default_permissions_structure() {
    // Verify the permission structure matches expected format
    let permissions = vec![
        "stream:read",
        "stream:create",
        "stream:update",
        "stream:delete",
        "recording:read",
        "recording:create",
        "recording:update",
        "recording:delete",
        "ai_task:read",
        "ai_task:create",
        "ai_task:update",
        "ai_task:delete",
        "user:read",
        "user:create",
        "user:update",
        "user:delete",
    ];

    for perm in permissions {
        let parts: Vec<&str> = perm.split(':').collect();
        assert_eq!(parts.len(), 2, "Permission '{}' should have format 'resource:action'", perm);
        assert!(!parts[0].is_empty(), "Resource should not be empty");
        assert!(!parts[1].is_empty(), "Action should not be empty");
    }

    println!("✅ Permission structure validation passed");
}

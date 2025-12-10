# Authentication & Authorization Guide

This document describes the authentication and authorization system for Quadrant VMS.

## Overview

Quadrant VMS implements a comprehensive authentication and authorization system with:

- **JWT-based authentication** for API access
- **API tokens** for long-lived service authentication
- **Role-Based Access Control (RBAC)** for fine-grained permissions
- **Multi-tenancy support** for isolated customer environments
- **Audit logging** for security compliance
- **OIDC/OAuth2 integration** for SSO (Google, Azure AD, Keycloak, custom providers)

## Architecture

### Components

1. **auth-service**: Centralized authentication service
   - User management (create, update, delete)
   - Role and permission management
   - JWT token issuance and verification
   - API token generation and validation
   - Audit log storage

2. **auth_middleware** (in common crate): Shared middleware for all services
   - JWT token verification
   - Permission checking
   - Request context injection

3. **Database**: PostgreSQL-backed storage
   - Users, roles, permissions, tenants
   - API tokens (hashed)
   - Audit logs

## Quick Start

### 1. Start auth-service

```bash
# Set required environment variables
export DATABASE_URL="postgresql://postgres:postgres@localhost:5432/quadrant_vms"
export JWT_SECRET="your-secret-key-CHANGE-IN-PRODUCTION"
export AUTH_SERVICE_ADDR="127.0.0.1:8083"

# Run the service
cargo run -p auth-service
```

### 2. Login and Get JWT Token

```bash
# Login with default admin user
curl -X POST http://localhost:8083/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "username": "admin",
    "password": "admin123",
    "tenant_id": "system"
  }'
```

Response:
```json
{
  "access_token": "eyJ0eXAiOiJKV1QiLCJhbGc...",
  "token_type": "Bearer",
  "expires_in": 3600,
  "user": {
    "user_id": "admin",
    "tenant_id": "system",
    "username": "admin",
    "email": "admin@quadrant.local",
    "display_name": "System Administrator",
    "is_system_admin": true,
    "roles": ["System Administrator"],
    "permissions": ["stream:read", "stream:create", ...]
  }
}
```

### 3. Use JWT Token in API Requests

```bash
# Use the token in Authorization header
curl -H "Authorization: Bearer eyJ0eXAiOiJKV1QiLCJhbGc..." \
  http://localhost:8080/v1/streams
```

## User Management

### Create a User

```bash
curl -X POST http://localhost:8083/v1/users \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "tenant_id": "system",
    "username": "operator1",
    "email": "operator1@example.com",
    "password": "secure_password",
    "display_name": "Operator One"
  }'
```

### List Users

```bash
curl http://localhost:8083/v1/users?tenant_id=system \
  -H "Authorization: Bearer $TOKEN"
```

### Update User

```bash
curl -X PUT http://localhost:8083/v1/users/operator1 \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "email": "newemail@example.com",
    "is_active": true
  }'
```

### Delete User

```bash
curl -X DELETE http://localhost:8083/v1/users/operator1 \
  -H "Authorization: Bearer $TOKEN"
```

## Role Management

### Built-in Roles

- **System Administrator**: Full system access (all permissions)
- **Operator**: Can manage streams, recordings, and AI tasks
- **Viewer**: Read-only access

### List Roles

```bash
curl http://localhost:8083/v1/roles?tenant_id=system \
  -H "Authorization: Bearer $TOKEN"
```

### Create Custom Role

```bash
curl -X POST http://localhost:8083/v1/roles \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "role_id": "custom-operator",
    "tenant_id": "system",
    "name": "Custom Operator",
    "description": "Custom role with specific permissions"
  }'
```

### Assign Permissions to Role

```bash
curl -X POST http://localhost:8083/v1/roles/custom-operator/permissions \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "permission_ids": ["stream:read", "stream:create", "recording:read"]
  }'
```

### Assign Roles to User

```bash
curl -X POST http://localhost:8083/v1/users/operator1/roles \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "role_ids": ["operator", "custom-operator"]
  }'
```

## Permissions

### Permission Format

Permissions follow the format: `resource:action`

**Resources:**
- `stream`, `recording`, `ai_task`, `user`, `role`, `tenant`, `audit`

**Actions:**
- `read`, `create`, `update`, `delete`

**Examples:**
- `stream:read` - View stream information
- `stream:create` - Create new streams
- `recording:delete` - Delete recordings
- `user:update` - Update user information

### List All Permissions

```bash
curl http://localhost:8083/v1/permissions \
  -H "Authorization: Bearer $TOKEN"
```

## API Tokens

API tokens are long-lived authentication tokens for service-to-service communication.

### Create API Token

```bash
curl -X POST http://localhost:8083/v1/users/operator1/tokens \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "CI/CD Token",
    "description": "Token for automated deployments",
    "expires_at": "2026-12-31T23:59:59Z"
  }'
```

Response:
```json
{
  "token_id": "550e8400-e29b-41d4-a716-446655440000",
  "token": "qvms_a1b2c3d4e5f6...",
  "expires_at": "2026-12-31T23:59:59Z"
}
```

**Important:** Save the `token` value - it's only shown once!

### Use API Token

```bash
curl -H "Authorization: Bearer qvms_a1b2c3d4e5f6..." \
  http://localhost:8080/v1/streams
```

### List User's API Tokens

```bash
curl http://localhost:8083/v1/users/operator1/tokens \
  -H "Authorization: Bearer $TOKEN"
```

### Revoke API Token

```bash
curl -X POST http://localhost:8083/v1/tokens/550e8400-e29b-41d4-a716-446655440000/revoke \
  -H "Authorization: Bearer $TOKEN"
```

## Multi-Tenancy

### Create Tenant

```bash
curl -X POST http://localhost:8083/v1/tenants \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "tenant_id": "acme-corp",
    "name": "ACME Corporation",
    "description": "ACME Corp VMS Instance",
    "max_users": 50,
    "max_streams": 100,
    "max_recordings": 1000
  }'
```

### Tenant Isolation

Each tenant has:
- Isolated users and roles
- Resource quotas (max users, streams, recordings, AI tasks)
- Separate audit logs

Users can only access resources within their tenant (unless they're system admins).

## Integrating with Services

### Add Auth Middleware to Your Service

1. **Add dependency** (already in common crate):

```rust
use common::auth_middleware::{auth_middleware, AuthMiddlewareConfig, AuthContext};
```

2. **Configure middleware**:

```rust
use axum::{routing::get, Router, middleware};
use std::sync::Arc;

let auth_config = Arc::new(AuthMiddlewareConfig::new(
    env::var("AUTH_SERVICE_URL").unwrap(),
    env::var("JWT_SECRET").unwrap(),
));

let app = Router::new()
    .route("/v1/streams", get(list_streams).post(create_stream))
    .layer(middleware::from_fn_with_state(
        auth_config.clone(),
        auth_middleware
    ));
```

3. **Access auth context in handlers**:

```rust
use axum::extract::Extension;
use common::auth_middleware::{AuthContext, require_permission};

async fn create_stream(
    Extension(auth_ctx): Extension<AuthContext>,
    Json(payload): Json<CreateStreamRequest>,
) -> Result<Json<StreamResponse>, ApiError> {
    // Check specific permission
    require_permission(&auth_ctx, "stream:create")?;

    // Access user info
    println!("User {} creating stream", auth_ctx.username);
    println!("Tenant: {}", auth_ctx.tenant_id);

    // Your logic here
    Ok(Json(response))
}
```

### Permission Checking

```rust
// Check single permission
if auth_ctx.has_permission("stream:delete") {
    // User can delete streams
}

// Check role
if auth_ctx.has_role("operator") {
    // User has operator role
}

// Check any of multiple permissions
if auth_ctx.has_any_permission(&["stream:read", "stream:create"]) {
    // User has at least one of these permissions
}

// Check all permissions
if auth_ctx.has_all_permissions(&["stream:read", "stream:create"]) {
    // User has all these permissions
}

// System admins bypass all permission checks
if auth_ctx.is_system_admin {
    // User is system admin
}
```

## Audit Logging

All authentication events and user actions are logged for compliance and security auditing.

### View Audit Logs

```bash
curl "http://localhost:8083/v1/audit-logs?tenant_id=system&limit=100" \
  -H "Authorization: Bearer $TOKEN"
```

### Audit Log Fields

- `tenant_id`: Tenant the action occurred in
- `user_id`: User who performed the action
- `action`: Action performed (e.g., "login", "create_stream", "delete_recording")
- `resource_type`: Type of resource affected
- `resource_id`: ID of the resource
- `status`: "success", "failure", or "denied"
- `ip_address`: Client IP address
- `user_agent`: Client user agent
- `metadata`: Additional context (JSON)
- `created_at`: Timestamp

## Security Best Practices

### Production Deployment

1. **Change Default Credentials**
   ```bash
   # The default admin password is "admin123"
   # Change it immediately after first login!
   ```

2. **Set Strong JWT Secret**
   ```bash
   export JWT_SECRET="$(openssl rand -hex 32)"
   ```

3. **Use HTTPS**
   - Always use TLS/SSL in production
   - Configure reverse proxy (nginx, Traefik, etc.)

4. **Token Expiration**
   ```bash
   export JWT_EXPIRATION_SECS=3600  # 1 hour
   ```

5. **Database Security**
   - Use strong database passwords
   - Enable SSL/TLS for database connections
   - Regular backups of auth database

6. **API Token Management**
   - Rotate tokens regularly
   - Set expiration dates
   - Revoke unused tokens

### Password Requirements

Current implementation uses Argon2 for password hashing:
- Minimum length: (not enforced, implement in your frontend)
- Complexity: (not enforced, implement in your frontend)
- Hash cost: Configurable via `BCRYPT_COST` (default: 10)

### Rate Limiting

Consider adding rate limiting to:
- Login endpoints (prevent brute force)
- Token creation endpoints
- API endpoints (per-tenant/per-user)

(Rate limiting not yet implemented - add using middleware)

## Environment Variables

### auth-service

```bash
# Required
DATABASE_URL="postgresql://user:password@localhost:5432/quadrant_vms"

# Optional (with defaults)
AUTH_SERVICE_ADDR="127.0.0.1:8083"           # Bind address
JWT_SECRET="default-jwt-secret..."            # JWT signing secret
JWT_EXPIRATION_SECS="3600"                    # Token expiration (1 hour)
BCRYPT_COST="10"                              # Password hashing cost
```

### Other Services (admin-gateway, coordinator, etc.)

```bash
# For auth middleware
AUTH_SERVICE_URL="http://localhost:8083"      # auth-service URL
JWT_SECRET="same-as-auth-service"             # Must match auth-service
```

## Troubleshooting

### "Invalid or expired token"

- Token may have expired (default: 1 hour)
- JWT_SECRET mismatch between auth-service and other services
- Token was issued before user's permissions changed

Solution: Login again to get fresh token

### "Permission denied"

- User doesn't have required permission
- Check user's roles: `GET /v1/users/{id}/roles`
- Check role's permissions: `GET /v1/roles/{id}/permissions`

### "User account is disabled"

- User's `is_active` flag is false
- Reactivate: `PUT /v1/users/{id}` with `{"is_active": true}`

## API Reference

See [AUTH_API.md](./AUTH_API.md) for complete API documentation.

## OIDC/OAuth2 Single Sign-On (SSO)

Quadrant VMS supports OIDC/OAuth2 integration for Single Sign-On with external identity providers.

### Supported Providers

- **Google Workspace / Google Identity**
- **Microsoft Azure AD / Entra ID**
- **Keycloak**
- **Any custom OIDC-compliant provider**

### How OIDC SSO Works

1. User initiates login through an OIDC provider
2. User is redirected to the provider's login page
3. After authentication, the provider redirects back with an authorization code
4. auth-service exchanges the code for tokens (ID token, access token)
5. User information is extracted from the ID token
6. If the user doesn't exist, they are auto-provisioned with the "viewer" role
7. A JWT token is issued for API access

### Configure an OIDC Provider

#### Google Workspace

```bash
curl -X POST http://localhost:8083/v1/oidc/providers \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "provider_id": "google",
    "tenant_id": "system",
    "name": "Google Workspace",
    "issuer_url": "https://accounts.google.com",
    "client_id": "YOUR_GOOGLE_CLIENT_ID.apps.googleusercontent.com",
    "client_secret": "YOUR_GOOGLE_CLIENT_SECRET",
    "scopes": ["openid", "profile", "email"]
  }'
```

**Setup Google OAuth2 Credentials:**
1. Go to [Google Cloud Console](https://console.cloud.google.com/)
2. Create a new project or select existing
3. Enable "Google+ API"
4. Go to "Credentials" → "Create Credentials" → "OAuth 2.0 Client ID"
5. Set redirect URI: `https://your-domain.com/auth/callback`
6. Copy Client ID and Client Secret

#### Microsoft Azure AD

```bash
curl -X POST http://localhost:8083/v1/oidc/providers \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "provider_id": "azure",
    "tenant_id": "system",
    "name": "Microsoft Azure AD",
    "issuer_url": "https://login.microsoftonline.com/YOUR_TENANT_ID/v2.0",
    "client_id": "YOUR_AZURE_APPLICATION_ID",
    "client_secret": "YOUR_AZURE_CLIENT_SECRET",
    "scopes": ["openid", "profile", "email"]
  }'
```

**Setup Azure AD App Registration:**
1. Go to [Azure Portal](https://portal.azure.com/)
2. Navigate to "Azure Active Directory" → "App registrations"
3. Click "New registration"
4. Set redirect URI: `https://your-domain.com/auth/callback`
5. Go to "Certificates & secrets" → Create new client secret
6. Copy Application (client) ID and client secret
7. Replace `YOUR_TENANT_ID` with your Azure AD tenant ID or use `common` for multi-tenant

#### Keycloak

```bash
curl -X POST http://localhost:8083/v1/oidc/providers \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "provider_id": "keycloak",
    "tenant_id": "system",
    "name": "Keycloak",
    "issuer_url": "https://keycloak.example.com/realms/myrealm",
    "client_id": "quadrant-vms",
    "client_secret": "YOUR_KEYCLOAK_CLIENT_SECRET",
    "scopes": ["openid", "profile", "email"]
  }'
```

**Setup Keycloak Client:**
1. Login to Keycloak admin console
2. Select your realm
3. Go to "Clients" → "Create"
4. Set Client ID: `quadrant-vms`
5. Set "Access Type" to "confidential"
6. Set valid redirect URIs: `https://your-domain.com/auth/callback`
7. Save and copy the client secret from "Credentials" tab

#### Custom OIDC Provider

```bash
curl -X POST http://localhost:8083/v1/oidc/providers \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "provider_id": "custom-provider",
    "tenant_id": "system",
    "name": "My Custom OIDC Provider",
    "issuer_url": "https://auth.example.com",
    "client_id": "your-client-id",
    "client_secret": "your-client-secret",
    "scopes": ["openid", "profile", "email"]
  }'
```

### Initiate OIDC Login

To start the SSO login flow, redirect users to:

```
GET /v1/auth/oidc/{provider_id}/login?redirect_uri=https://your-app.com/callback
```

Response:
```json
{
  "authorization_url": "https://accounts.google.com/o/oauth2/v2/auth?client_id=...",
  "state": "random-csrf-token"
}
```

The frontend should:
1. Store the `state` token
2. Redirect the user to the `authorization_url`

### Handle OIDC Callback

After the user authenticates with the provider, they will be redirected back with a `code` and `state` parameter.

```bash
POST /v1/auth/oidc/{provider_id}/callback
Content-Type: application/json

{
  "code": "authorization-code-from-provider",
  "state": "csrf-token-from-login-response",
  "redirect_uri": "https://your-app.com/callback"
}
```

Response:
```json
{
  "access_token": "eyJ0eXAiOiJKV1QiLCJhbGc...",
  "token_type": "Bearer",
  "expires_in": 3600,
  "user": {
    "user_id": "uuid",
    "tenant_id": "system",
    "username": "user@example.com",
    "email": "user@example.com",
    "display_name": "John Doe",
    "is_system_admin": false,
    "roles": ["Viewer"],
    "permissions": ["stream:read", "recording:read", "ai_task:read", ...]
  }
}
```

### User Provisioning

When a user logs in via OIDC for the first time:

1. **User Creation**: A new user account is automatically created
   - Username: extracted from email (or provider sub + provider_id)
   - Email: from OIDC claims
   - Display name: from OIDC name claim
   - No password set (SSO-only)

2. **Default Role**: New users are assigned the "Viewer" role by default

3. **Identity Linking**: An OIDC identity is created linking the user to the provider

4. **Subsequent Logins**: The user is recognized by their provider user ID (sub claim)

### Manage OIDC Providers

#### List Providers

```bash
curl http://localhost:8083/v1/oidc/providers?tenant_id=system \
  -H "Authorization: Bearer $TOKEN"
```

#### Get Provider Details

```bash
curl http://localhost:8083/v1/oidc/providers/google \
  -H "Authorization: Bearer $TOKEN"
```

#### Update Provider

```bash
curl -X PUT http://localhost:8083/v1/oidc/providers/google \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Google Workspace (Updated)",
    "is_active": true
  }'
```

#### Delete Provider

```bash
curl -X DELETE http://localhost:8083/v1/oidc/providers/google \
  -H "Authorization: Bearer $TOKEN"
```

### View User's OIDC Identities

```bash
curl http://localhost:8083/v1/users/{user_id}/oidc-identities \
  -H "Authorization: Bearer $TOKEN"
```

Response:
```json
[
  {
    "identity_id": "uuid",
    "user_id": "user-uuid",
    "provider_id": "google",
    "provider_user_id": "1234567890",
    "provider_email": "user@example.com",
    "created_at": "2025-01-10T12:00:00Z",
    "updated_at": "2025-01-10T12:00:00Z"
  }
]
```

### Security Considerations

1. **CSRF Protection**: The `state` parameter prevents CSRF attacks. Always validate it!
2. **HTTPS Required**: OIDC should only be used over HTTPS in production
3. **Token Validation**: ID tokens are cryptographically verified using provider's public keys
4. **Client Secret Security**: Store client secrets securely (environment variables, secret managers)
5. **Redirect URI Validation**: Providers validate redirect URIs to prevent attacks

### Troubleshooting

#### "invalid or expired state"
- The CSRF state token has expired or doesn't match
- Restart the login flow from the beginning

#### "OIDC provider is not active"
- The provider has been disabled
- Re-enable it: `PUT /v1/oidc/providers/{id}` with `{"is_active": true}`

#### "failed to discover provider metadata"
- The issuer URL is incorrect
- The provider's `.well-known/openid-configuration` endpoint is unreachable
- Check network connectivity and firewall rules

#### "invalid_client" error from provider
- Client ID or client secret is incorrect
- Verify credentials in provider's admin console

#### "redirect_uri_mismatch"
- The redirect URI doesn't match what's registered with the provider
- Update the provider's configuration to include your callback URL

## Future Enhancements

### Additional Features (Planned)

- Two-factor authentication (2FA)
- Password reset flows
- Session management
- API rate limiting
- IP whitelisting
- Advanced audit log filtering and export
- Webhook notifications for security events

## Support

For issues or questions:
- GitHub Issues: https://github.com/anthropics/quadrant-vms/issues
- Documentation: https://quadrant-vms.docs

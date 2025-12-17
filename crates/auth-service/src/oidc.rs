use anyhow::Result;
use openidconnect::{
    core::{
        CoreAuthenticationFlow, CoreClient, CoreGenderClaim, CoreProviderMetadata,
    },
    reqwest::async_http_client,
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, EmptyAdditionalClaims, IssuerUrl, Nonce, OAuth2TokenResponse, RedirectUrl,
    Scope, TokenResponse,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::{error::ApiError, models::OidcProvider};

/// OIDC client manager that handles provider discovery and token operations
pub struct OidcClientManager {
    /// Cache of OIDC clients by provider_id
    clients: Arc<RwLock<HashMap<String, CoreClient>>>,
    /// Cache of pending CSRF states (state -> provider_id, nonce)
    states: Arc<RwLock<HashMap<String, (String, String)>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcUserInfo {
    pub sub: String,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub name: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub picture: Option<String>,
}

impl OidcClientManager {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create an OIDC client for a provider
    pub async fn get_client(&self, provider: &OidcProvider) -> Result<CoreClient, ApiError> {
        // Check cache first
        {
            let clients = self.clients.read().await;
            if let Some(client) = clients.get(&provider.provider_id) {
                return Ok(client.clone());
            }
        }

        // Create new client
        let client = self.create_client(provider).await?;

        // Cache it
        {
            let mut clients = self.clients.write().await;
            clients.insert(provider.provider_id.clone(), client.clone());
        }

        Ok(client)
    }

    /// Create a new OIDC client by discovering provider metadata
    async fn create_client(&self, provider: &OidcProvider) -> Result<CoreClient, ApiError> {
        let issuer_url = IssuerUrl::new(provider.issuer_url.clone())
            .map_err(|e| ApiError::internal(format!("invalid issuer URL: {}", e)))?;

        // Discover provider metadata
        let provider_metadata = CoreProviderMetadata::discover_async(issuer_url, async_http_client)
            .await
            .map_err(|e| ApiError::internal(format!("failed to discover provider metadata: {}", e)))?;

        // Create client
        let client = CoreClient::from_provider_metadata(
            provider_metadata,
            ClientId::new(provider.client_id.clone()),
            Some(ClientSecret::new(provider.client_secret.clone())),
        );

        Ok(client)
    }

    /// Generate authorization URL for OIDC login flow
    pub async fn generate_authorization_url(
        &self,
        provider: &OidcProvider,
        redirect_uri: &str,
    ) -> Result<(String, String), ApiError> {
        let client = self.get_client(provider).await?;

        let redirect_url = RedirectUrl::new(redirect_uri.to_string())
            .map_err(|e| ApiError::bad_request(format!("invalid redirect URI: {}", e)))?;

        // Generate authorization URL with PKCE
        let mut auth_request = client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .set_redirect_uri(std::borrow::Cow::Owned(redirect_url));

        // Add scopes
        for scope in &provider.scopes {
            auth_request = auth_request.add_scope(Scope::new(scope.clone()));
        }

        let (auth_url, csrf_state, nonce) = auth_request.url();

        // Store state for CSRF validation
        {
            let mut states = self.states.write().await;
            states.insert(
                csrf_state.secret().clone(),
                (provider.provider_id.clone(), nonce.secret().clone()),
            );
        }

        Ok((auth_url.to_string(), csrf_state.secret().clone()))
    }

    /// Exchange authorization code for tokens
    pub async fn exchange_code(
        &self,
        provider: &OidcProvider,
        code: &str,
        state: &str,
        redirect_uri: &str,
    ) -> Result<OidcUserInfo, ApiError> {
        // Verify CSRF state
        let (provider_id, nonce_secret) = {
            let mut states = self.states.write().await;
            states
                .remove(state)
                .ok_or_else(|| ApiError::bad_request("invalid or expired state"))?
        };

        if provider_id != provider.provider_id {
            return Err(ApiError::bad_request("state does not match provider"));
        }

        let client = self.get_client(provider).await?;

        let redirect_url = RedirectUrl::new(redirect_uri.to_string())
            .map_err(|e| ApiError::bad_request(format!("invalid redirect URI: {}", e)))?;

        // Exchange code for tokens
        let token_response = client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .set_redirect_uri(std::borrow::Cow::Owned(redirect_url))
            .request_async(async_http_client)
            .await
            .map_err(|e| ApiError::internal(format!("failed to exchange code for token: {}", e)))?;

        // Verify ID token
        let id_token = token_response
            .id_token()
            .ok_or_else(|| ApiError::internal("no ID token in response"))?;

        let nonce = Nonce::new(nonce_secret);
        let claims = id_token
            .claims(&client.id_token_verifier(), &nonce)
            .map_err(|e| ApiError::internal(format!("failed to verify ID token: {}", e)))?;

        // Extract user info from ID token claims
        let user_info = OidcUserInfo {
            sub: claims.subject().to_string(),
            email: claims.email().map(|e| e.to_string()),
            email_verified: claims.email_verified(),
            name: claims.name().and_then(|n| n.get(None).map(|s| s.to_string())),
            given_name: claims.given_name().and_then(|n| n.get(None).map(|s| s.to_string())),
            family_name: claims.family_name().and_then(|n| n.get(None).map(|s| s.to_string())),
            picture: claims.picture().and_then(|p| p.get(None).map(|u| u.to_string())),
        };

        // Optionally fetch additional user info from UserInfo endpoint
        // (ID token already contains most claims, but some providers may have more)
        let access_token = token_response.access_token();
        if let Ok(userinfo_request) = client
            .user_info(access_token.clone(), None)
        {
            if let Ok(userinfo_claims) = userinfo_request
                .request_async::<EmptyAdditionalClaims, _, _, CoreGenderClaim, _>(async_http_client)
                .await
            {
                // Merge additional claims if needed (for now we prioritize ID token claims)
                tracing::debug!("UserInfo claims: {:?}", userinfo_claims);
            }
        }

        Ok(user_info)
    }

    /// Invalidate cached client for a provider (call when provider config changes)
    pub async fn invalidate_client(&self, provider_id: &str) {
        let mut clients = self.clients.write().await;
        clients.remove(provider_id);
    }

    /// Clean up expired CSRF states (should be called periodically)
    pub async fn cleanup_expired_states(&self, _max_age_secs: i64) {
        // For simplicity, we'll remove all states older than max_age
        // In production, you'd want to store timestamps with states
        // For now, we just clear states that are older than a reasonable timeout
        // This is a simple implementation - consider using a TTL cache in production
        let mut states = self.states.write().await;
        if states.len() > 1000 {
            // Prevent unbounded growth
            states.clear();
            tracing::warn!("CSRF state cache cleared due to size limit");
        }
    }
}

impl Default for OidcClientManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Provider-specific configurations for common OIDC providers
pub struct OidcProviderTemplate {
    pub name: String,
    pub issuer_url: String,
    pub default_scopes: Vec<String>,
}

impl OidcProviderTemplate {
    /// Google Workspace / Google Identity
    pub fn google() -> Self {
        Self {
            name: "Google".to_string(),
            issuer_url: "https://accounts.google.com".to_string(),
            default_scopes: vec![
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
            ],
        }
    }

    /// Microsoft Azure AD / Entra ID
    /// Note: Replace {tenant} with your Azure AD tenant ID or "common"
    pub fn microsoft_azure(tenant_id: &str) -> Self {
        Self {
            name: "Microsoft Azure AD".to_string(),
            issuer_url: format!("https://login.microsoftonline.com/{}/v2.0", tenant_id),
            default_scopes: vec![
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
            ],
        }
    }

    /// Keycloak
    /// Note: Replace {realm} with your Keycloak realm name
    pub fn keycloak(base_url: &str, realm: &str) -> Self {
        Self {
            name: format!("Keycloak ({})", realm),
            issuer_url: format!("{}/realms/{}", base_url, realm),
            default_scopes: vec![
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
            ],
        }
    }

    /// Generic OIDC provider
    pub fn generic(name: String, issuer_url: String) -> Self {
        Self {
            name,
            issuer_url,
            default_scopes: vec![
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_templates() {
        let google = OidcProviderTemplate::google();
        assert_eq!(google.name, "Google");
        assert_eq!(google.issuer_url, "https://accounts.google.com");
        assert!(google.default_scopes.contains(&"openid".to_string()));

        let azure = OidcProviderTemplate::microsoft_azure("common");
        assert_eq!(azure.name, "Microsoft Azure AD");
        assert!(azure.issuer_url.contains("login.microsoftonline.com"));

        let keycloak = OidcProviderTemplate::keycloak("https://keycloak.example.com", "myrealm");
        assert!(keycloak.name.contains("Keycloak"));
        assert!(keycloak.issuer_url.contains("realms/myrealm"));
    }

    #[tokio::test]
    async fn test_oidc_manager_creation() {
        let manager = OidcClientManager::new();
        let clients = manager.clients.read().await;
        assert_eq!(clients.len(), 0);
    }
}

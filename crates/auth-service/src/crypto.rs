use anyhow::{Context, Result};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::Utc;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand::Rng;

use crate::models::JwtClaims;

/// Hash a password using Argon2
pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .context("failed to hash password")?
        .to_string();
    Ok(password_hash)
}

/// Verify a password against its hash
pub fn verify_password(password: &str, password_hash: &str) -> Result<bool> {
    let parsed_hash = PasswordHash::new(password_hash)
        .context("failed to parse password hash")?;
    let argon2 = Argon2::default();
    Ok(argon2
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

/// Generate a JWT token
pub fn generate_jwt(
    user_id: &str,
    tenant_id: &str,
    username: &str,
    is_system_admin: bool,
    roles: Vec<String>,
    permissions: Vec<String>,
    jwt_secret: &str,
    expiration_secs: i64,
) -> Result<String> {
    let now = Utc::now().timestamp();
    let claims = JwtClaims {
        sub: user_id.to_string(),
        tenant_id: tenant_id.to_string(),
        username: username.to_string(),
        is_system_admin,
        roles,
        permissions,
        exp: now + expiration_secs,
        iat: now,
    };

    let token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .context("failed to encode JWT")?;

    Ok(token)
}

/// Verify and decode a JWT token
pub fn verify_jwt(token: &str, jwt_secret: &str) -> Result<JwtClaims> {
    let token_data = decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .context("failed to decode JWT")?;

    Ok(token_data.claims)
}

/// Generate a random API token (cryptographically secure)
pub fn generate_api_token() -> String {
    let mut rng = rand::thread_rng();
    let token_bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    format!("qvms_{}", hex::encode(token_bytes))
}

/// Hash an API token for storage
pub fn hash_api_token(token: &str) -> Result<String> {
    hash_password(token)
}

/// Verify an API token against its hash
pub fn verify_api_token(token: &str, token_hash: &str) -> Result<bool> {
    verify_password(token, token_hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hashing() {
        let password = "test_password_123";
        let hash = hash_password(password).unwrap();
        assert!(verify_password(password, &hash).unwrap());
        assert!(!verify_password("wrong_password", &hash).unwrap());
    }

    #[test]
    fn test_jwt_generation_and_verification() {
        let secret = "test_secret";
        let token = generate_jwt(
            "user_123",
            "tenant_123",
            "testuser",
            false,
            vec!["operator".to_string()],
            vec!["stream:read".to_string(), "stream:create".to_string()],
            secret,
            3600,
        )
        .unwrap();

        let claims = verify_jwt(&token, secret).unwrap();
        assert_eq!(claims.sub, "user_123");
        assert_eq!(claims.tenant_id, "tenant_123");
        assert_eq!(claims.username, "testuser");
        assert!(!claims.is_system_admin);
        assert_eq!(claims.roles, vec!["operator"]);
        assert_eq!(claims.permissions.len(), 2);
    }

    #[test]
    fn test_api_token_generation() {
        let token = generate_api_token();
        assert!(token.starts_with("qvms_"));
        assert!(token.len() > 10);

        let hash = hash_api_token(&token).unwrap();
        assert!(verify_api_token(&token, &hash).unwrap());
        assert!(!verify_api_token("wrong_token", &hash).unwrap());
    }
}

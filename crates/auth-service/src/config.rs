use anyhow::{Context, Result};
use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub jwt_secret: String,
    pub jwt_expiration_secs: i64,
    pub bcrypt_cost: u32,
}

impl AuthConfig {
    pub fn from_env() -> Result<Self> {
        let bind_addr = std::env::var("AUTH_SERVICE_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:8083".to_string())
            .parse()
            .context("invalid AUTH_SERVICE_ADDR")?;

        let database_url = std::env::var("DATABASE_URL")
            .context("DATABASE_URL environment variable required")?;

        let jwt_secret = std::env::var("JWT_SECRET")
            .unwrap_or_else(|_| {
                tracing::warn!("JWT_SECRET not set, using default (INSECURE for production!)");
                "default-jwt-secret-CHANGE-IN-PRODUCTION".to_string()
            });

        let jwt_expiration_secs = std::env::var("JWT_EXPIRATION_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3600); // Default: 1 hour

        let bcrypt_cost = std::env::var("BCRYPT_COST")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10); // Default: 10

        Ok(Self {
            bind_addr,
            database_url,
            jwt_secret,
            jwt_expiration_secs,
            bcrypt_cost,
        })
    }
}

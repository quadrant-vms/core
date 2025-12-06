use anyhow::{Context, Result};
use reqwest::Url;
use std::env;

#[derive(Debug, Clone)]
pub struct AiServiceConfig {
    /// Address to bind the HTTP server to
    pub bind_addr: String,

    /// Coordinator endpoint URL (optional)
    pub coordinator_url: Option<Url>,

    /// Node ID for this AI service instance
    pub node_id: String,
}

impl AiServiceConfig {
    pub fn from_env() -> Result<Self> {
        let bind_addr =
            env::var("AI_SERVICE_ADDR").unwrap_or_else(|_| "0.0.0.0:8084".to_string());

        let coordinator_url = env::var("COORDINATOR_URL")
            .ok()
            .map(|s| Url::parse(&s).context("Invalid COORDINATOR_URL"))
            .transpose()?;

        let node_id = env::var("NODE_ID").unwrap_or_else(|_| {
            format!(
                "ai-service-{}",
                hostname::get()
                    .ok()
                    .and_then(|h| h.into_string().ok())
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
            )
        });

        Ok(Self {
            bind_addr,
            coordinator_url,
            node_id,
        })
    }
}

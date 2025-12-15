use anyhow::Result;
use std::env;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    pub bind_addr: String,
    pub frontend_dir: PathBuf,
    pub device_manager_url: String,
    pub admin_gateway_url: String,
    pub recorder_node_url: String,
    pub ai_service_url: String,
    pub alert_service_url: String,
    pub auth_service_url: String,
    pub playback_service_url: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            bind_addr: env::var("OPERATOR_UI_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:8090".to_string()),
            frontend_dir: env::var("FRONTEND_DIR")
                .unwrap_or_else(|_| "./crates/operator-ui/frontend/dist".to_string())
                .into(),
            device_manager_url: env::var("DEVICE_MANAGER_URL")
                .unwrap_or_else(|_| "http://localhost:8087".to_string()),
            admin_gateway_url: env::var("ADMIN_GATEWAY_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            recorder_node_url: env::var("RECORDER_NODE_URL")
                .unwrap_or_else(|_| "http://localhost:8085".to_string()),
            ai_service_url: env::var("AI_SERVICE_URL")
                .unwrap_or_else(|_| "http://localhost:8088".to_string()),
            alert_service_url: env::var("ALERT_SERVICE_URL")
                .unwrap_or_else(|_| "http://localhost:8089".to_string()),
            auth_service_url: env::var("AUTH_SERVICE_URL")
                .unwrap_or_else(|_| "http://localhost:8081".to_string()),
            playback_service_url: env::var("PLAYBACK_SERVICE_URL")
                .unwrap_or_else(|_| "http://localhost:8084".to_string()),
        })
    }
}

use anyhow::{Context, Result};
use reqwest::Url;
use std::{env, net::SocketAddr};

#[derive(Clone)]
pub struct GatewayConfig {
    pub bind_addr: SocketAddr,
    pub coordinator_base_url: Url,
    pub node_id: String,
    pub worker_base_url: Url,
}

impl GatewayConfig {
    pub fn from_env() -> Result<Self> {
        let bind = env::var("ADMIN_GATEWAY_ADDR")
            .unwrap_or_else(|_| "0.0.0.0:8081".to_string());
        let bind_addr: SocketAddr = bind.parse().context("invalid ADMIN_GATEWAY_ADDR")?;

        let coord = env::var("COORDINATOR_ENDPOINT")
            .unwrap_or_else(|_| "http://127.0.0.1:8082".to_string());
        let coordinator_base_url = Url::parse(&coord).context("invalid COORDINATOR_ENDPOINT")?;

        let worker = env::var("STREAM_WORKER_ENDPOINT")
            .unwrap_or_else(|_| "http://127.0.0.1:8080/".to_string());
        let worker_base_url =
            Url::parse(&worker).context("invalid STREAM_WORKER_ENDPOINT")?;

        let node_id = env::var("NODE_ID").unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());

        Ok(Self {
            bind_addr,
            coordinator_base_url,
            node_id,
            worker_base_url,
        })
    }
}

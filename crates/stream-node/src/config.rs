use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let bind_addr = env::var("STREAM_NODE_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

        Ok(Config {
            bind_addr,
        })
    }
}

use anyhow::Result;
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::Config;
use crate::incident::IncidentStore;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub http_client: Client,
    pub incident_store: Arc<RwLock<IncidentStore>>,
}

impl AppState {
    pub async fn new(config: Config) -> Result<Self> {
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let incident_store = Arc::new(RwLock::new(IncidentStore::new()));

        Ok(Self {
            config,
            http_client,
            incident_store,
        })
    }
}

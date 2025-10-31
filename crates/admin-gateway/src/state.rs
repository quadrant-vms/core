use crate::{config::GatewayConfig, coordinator::CoordinatorClient};
use common::streams::StreamInfo;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    inner: Arc<AppStateInner>,
}

struct AppStateInner {
    config: GatewayConfig,
    coordinator: Arc<dyn CoordinatorClient>,
    streams: RwLock<HashMap<String, StreamInfo>>,
}

impl AppState {
    pub fn new(config: GatewayConfig, coordinator: Arc<dyn CoordinatorClient>) -> Self {
        let inner = AppStateInner {
            config,
            coordinator,
            streams: RwLock::new(HashMap::new()),
        };
        Self {
            inner: Arc::new(inner),
        }
    }

    pub fn node_id(&self) -> &str {
        &self.inner.config.node_id
    }

    pub fn coordinator(&self) -> Arc<dyn CoordinatorClient> {
        self.inner.coordinator.clone()
    }

    pub fn streams(&self) -> &RwLock<HashMap<String, StreamInfo>> {
        &self.inner.streams
    }
}

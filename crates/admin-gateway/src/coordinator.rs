use anyhow::{Context, Result};
use async_trait::async_trait;
use common::leases::{
    LeaseAcquireRequest, LeaseAcquireResponse, LeaseReleaseRequest, LeaseReleaseResponse,
};
use reqwest::Url;
use std::time::Duration;
use tracing::instrument;

#[async_trait]
pub trait CoordinatorClient: Send + Sync {
    async fn acquire(&self, request: &LeaseAcquireRequest) -> Result<LeaseAcquireResponse>;
    async fn release(&self, request: &LeaseReleaseRequest) -> Result<LeaseReleaseResponse>;
}

pub struct HttpCoordinatorClient {
    base: Url,
    client: reqwest::Client,
}

impl HttpCoordinatorClient {
    pub fn new(base: Url) -> Result<Self> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(10))
            .build()?;
        Ok(Self { base, client })
    }

    fn endpoint(&self, path: &str) -> Result<Url> {
        self.base.join(path).context("invalid coordinator endpoint")
    }
}

#[async_trait]
impl CoordinatorClient for HttpCoordinatorClient {
    #[instrument(skip_all, fields(resource = %request.resource_id, holder = %request.holder_id))]
    async fn acquire(&self, request: &LeaseAcquireRequest) -> Result<LeaseAcquireResponse> {
        let url = self.endpoint("v1/leases/acquire")?;
        let resp = self
            .client
            .post(url)
            .json(request)
            .send()
            .await
            .context("coordinator acquire request failed")?;
        let resp = resp
            .error_for_status()
            .context("coordinator acquire returned error status")?;
        Ok(resp.json().await.context("failed to parse acquire response")?)
    }

    #[instrument(skip_all, fields(lease = %request.lease_id))]
    async fn release(&self, request: &LeaseReleaseRequest) -> Result<LeaseReleaseResponse> {
        let url = self.endpoint("v1/leases/release")?;
        let resp = self
            .client
            .post(url)
            .json(request)
            .send()
            .await
            .context("coordinator release request failed")?;
        let resp = resp
            .error_for_status()
            .context("coordinator release returned error status")?;
        Ok(resp.json().await.context("failed to parse release response")?)
    }
}

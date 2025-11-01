use anyhow::{Context, Result};
use async_trait::async_trait;
use common::streams::StreamConfig;
use reqwest::Url;
use std::time::Duration;
use tracing::instrument;

#[async_trait]
pub trait WorkerClient: Send + Sync {
  async fn start_stream(&self, config: &StreamConfig) -> Result<()>;
  async fn stop_stream(&self, stream_id: &str) -> Result<()>;
}

pub struct HttpWorkerClient {
  base: Url,
  client: reqwest::Client,
}

impl HttpWorkerClient {
  pub fn new(base: Url) -> Result<Self> {
    let client = reqwest::Client::builder()
      .connect_timeout(Duration::from_secs(3))
      .timeout(Duration::from_secs(10))
      .build()?;
    Ok(Self { base, client })
  }

  fn endpoint(&self, path: &str) -> Result<Url> {
    self.base.join(path).context("invalid worker endpoint")
  }
}

#[async_trait]
impl WorkerClient for HttpWorkerClient {
  #[instrument(skip_all, fields(stream = %config.id))]
  async fn start_stream(&self, config: &StreamConfig) -> Result<()> {
    let mut url = self.endpoint("start")?;
    {
      let mut pairs = url.query_pairs_mut();
      pairs.append_pair("id", &config.id);
      pairs.append_pair("uri", &config.uri);
      if let Some(codec) = &config.codec {
        pairs.append_pair("codec", codec);
      }
      if let Some(container) = &config.container {
        pairs.append_pair("container", container);
      }
    }

    let resp = self
      .client
      .get(url)
      .send()
      .await
      .context("worker start request failed")?;
    resp
      .error_for_status()
      .context("worker start returned error status")?;
    Ok(())
  }

  #[instrument(skip_all, fields(stream = stream_id))]
  async fn stop_stream(&self, stream_id: &str) -> Result<()> {
    let mut url = self.endpoint("stop")?;
    {
      let mut pairs = url.query_pairs_mut();
      pairs.append_pair("id", stream_id);
    }
    let resp = self
      .client
      .get(url)
      .send()
      .await
      .context("worker stop request failed")?;
    resp
      .error_for_status()
      .context("worker stop returned error status")?;
    Ok(())
  }
}

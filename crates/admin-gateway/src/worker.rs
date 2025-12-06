use anyhow::{Context, Result};
use async_trait::async_trait;
use common::{
  recordings::{RecordingStartRequest, RecordingStartResponse, RecordingStopRequest, RecordingStopResponse},
  streams::StreamConfig,
};
use reqwest::Url;
use std::time::Duration;
use tracing::instrument;

#[async_trait]
pub trait WorkerClient: Send + Sync {
  async fn start_stream(&self, config: &StreamConfig) -> Result<()>;
  async fn stop_stream(&self, stream_id: &str) -> Result<()>;
}

#[async_trait]
pub trait RecorderClient: Send + Sync {
  async fn start_recording(&self, request: &RecordingStartRequest) -> Result<RecordingStartResponse>;
  async fn stop_recording(&self, request: &RecordingStopRequest) -> Result<RecordingStopResponse>;
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

pub struct HttpRecorderClient {
  base: Url,
  client: reqwest::Client,
}

impl HttpRecorderClient {
  pub fn new(base: Url) -> Result<Self> {
    let client = reqwest::Client::builder()
      .connect_timeout(Duration::from_secs(3))
      .timeout(Duration::from_secs(10))
      .build()?;
    Ok(Self { base, client })
  }

  fn endpoint(&self, path: &str) -> Result<Url> {
    self.base.join(path).context("invalid recorder endpoint")
  }
}

#[async_trait]
impl RecorderClient for HttpRecorderClient {
  #[instrument(skip_all, fields(recording_id = %request.config.id))]
  async fn start_recording(&self, request: &RecordingStartRequest) -> Result<RecordingStartResponse> {
    let url = self.endpoint("start")?;
    let resp = self
      .client
      .post(url)
      .json(request)
      .send()
      .await
      .context("recorder start request failed")?;

    let response = resp
      .error_for_status()
      .context("recorder start returned error status")?
      .json::<RecordingStartResponse>()
      .await
      .context("failed to parse recorder start response")?;

    Ok(response)
  }

  #[instrument(skip_all, fields(recording_id = %request.id))]
  async fn stop_recording(&self, request: &RecordingStopRequest) -> Result<RecordingStopResponse> {
    let url = self.endpoint("stop")?;
    let resp = self
      .client
      .post(url)
      .json(request)
      .send()
      .await
      .context("recorder stop request failed")?;

    let response = resp
      .error_for_status()
      .context("recorder stop returned error status")?
      .json::<RecordingStopResponse>()
      .await
      .context("failed to parse recorder stop response")?;

    Ok(response)
  }
}

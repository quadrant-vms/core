use anyhow::Result;
use aws_config::{meta::region::RegionProviderChain, BehaviorVersion, Region};
use aws_credential_types::Credentials;
use aws_sdk_s3::{config::Builder as S3ConfigBuilder, primitives::ByteStream, Client};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
  path::{Path, PathBuf},
  time::Duration,
};
use tokio::sync::mpsc;
use tracing::{info, warn};

#[derive(Clone)]
pub struct S3Config {
  pub endpoint: String,
  pub access_key: String,
  pub secret_key: String,
  pub region: String,
  pub bucket: String,
  pub prefix: String, // e.g. camera id
}

impl Default for S3Config {
  fn default() -> Self {
    Self {
      endpoint: std::env::var("S3_ENDPOINT").unwrap_or_else(|_| "http://localhost:9000".into()),
      access_key: std::env::var("S3_ACCESS_KEY").unwrap_or_else(|_| "minio".into()),
      secret_key: std::env::var("S3_SECRET_KEY").unwrap_or_else(|_| "minio123".into()),
      region: std::env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".into()),
      bucket: std::env::var("S3_BUCKET").unwrap_or_else(|_| "vms".into()),
      prefix: "".into(),
    }
  }
}

async fn s3_client(cfg: &S3Config) -> Client {
  let region = Region::new(cfg.region.clone());
  let region_provider = RegionProviderChain::first_try(region.clone()).or_default_provider();
  let base = aws_config::defaults(BehaviorVersion::v2025_08_07())
    .region(region_provider)
    .load()
    .await;

  let conf = S3ConfigBuilder::from(&base)
    .region(region)
    .endpoint_url(cfg.endpoint.clone())
    .force_path_style(true)
    .credentials_provider(Credentials::new(
      cfg.access_key.clone(),
      cfg.secret_key.clone(),
      None,
      None,
      "static",
    ))
    .build();

  Client::from_conf(conf)
}

pub async fn ensure_bucket(client: &Client, name: &str) {
  let _ = client.create_bucket().bucket(name).send().await;
}

/// Watch a directory and upload new files to S3 (MinIO).
/// Files filtered by extension: m3u8, ts, m4s, mp4.
pub async fn watch_and_upload(dir: PathBuf, s3cfg: S3Config, prefix: String) -> Result<()> {
  let client = s3_client(&s3cfg).await;
  ensure_bucket(&client, &s3cfg.bucket).await;

  let (tx, mut rx) = mpsc::unbounded_channel::<PathBuf>();

  // notify
  let mut watcher: RecommendedWatcher =
    notify::recommended_watcher(move |res: notify::Result<notify::Event>| match res {
      Ok(event) => {
        if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
          for p in event.paths {
            if p.is_file() {
              let _ = tx.send(p);
            }
          }
        }
      }
      Err(e) => eprintln!("watch error: {e}"),
    })?;
  watcher.watch(Path::new(&dir), RecursiveMode::NonRecursive)?;

  info!(?dir, "S3 uploader watching");

  while let Some(path) = rx.recv().await {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
      continue;
    };
    if !matches!(ext, "m3u8" | "ts" | "m4s" | "mp4") {
      continue;
    }

    tokio::time::sleep(Duration::from_millis(100)).await;

    let Some(filename) = path.file_name() else {
      warn!("invalid path, no filename: {:?}", path);
      continue;
    };
    let key = format!("{}/{}", prefix, filename.to_string_lossy());
    match tokio::fs::read(&path).await {
      Ok(bytes) => {
        let body = ByteStream::from(bytes);
        match client
          .put_object()
          .bucket(&s3cfg.bucket)
          .key(&key)
          .body(body)
          .send()
          .await
        {
          Ok(_) => info!(%key, "uploaded"),
          Err(e) => warn!(%key, "upload failed: {e}"),
        }
      }
      Err(e) => warn!("read failed for {:?}: {e}", path),
    }
  }

  Ok(())
}

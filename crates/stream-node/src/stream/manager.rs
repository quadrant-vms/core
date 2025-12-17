use super::{build_pipeline_args, hls_root, Codec, Container};
use crate::compat;
use crate::metrics::STREAMS_RUNNING;
use crate::storage::{self, S3Config as UploaderConfig};
use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use std::{
  collections::HashMap,
  fs,
  path::PathBuf,
  process::{Child, Command, Stdio},
  time::{Duration, Instant},
};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

// Maximum concurrent streams to prevent OOM
const MAX_CONCURRENT_STREAMS: usize = 1000;

#[derive(Clone, Debug)]
pub struct StreamSpec {
  pub id: String,
  pub uri: String,
  pub codec: Codec,
  pub container: Container,
}

#[derive(Clone, Debug)]
pub struct StreamStatus {
  pub id: String,
  pub uri: String,
  pub codec: String,
  pub container: String,
  pub running: bool,
  pub playlist: PathBuf,
  pub output_dir: PathBuf,
}

static REGISTRY: Lazy<Mutex<HashMap<String, (Child, StreamStatus, StreamSpec)>>> =
  Lazy::new(|| Mutex::new(HashMap::new()));

fn readiness_timeout() -> Duration {
  std::env::var("HLS_READY_TIMEOUT_SECS")
    .ok()
    .and_then(|v| v.parse::<u64>().ok())
    .map(Duration::from_secs)
    .unwrap_or_else(|| Duration::from_secs(20))
}

pub async fn start_stream(spec_req: &StreamSpec) -> Result<()> {
  {
    let reg = REGISTRY.lock().await;
    if reg.contains_key(&spec_req.id) {
      return Err(anyhow!("stream '{}' already running", spec_req.id));
    }
    // Check concurrent stream limit
    if reg.len() >= MAX_CONCURRENT_STREAMS {
      return Err(anyhow!(
        "Maximum concurrent streams ({}) exceeded. Cannot start new stream.",
        MAX_CONCURRENT_STREAMS
      ));
    }
  }

  let pr = compat::probe::probe(&spec_req.uri)
    .await
    .unwrap_or_default();

  let profiles = compat::load_profiles_from_dir(&compat::profiles_dir());
  let profile = profiles
    .iter()
    .find(|p| match (&p.vendor, &pr.vendor_hint) {
      (Some(v), Some(h)) => v.eq_ignore_ascii_case(h),
      _ => false,
    })
    .cloned()
    .unwrap_or_default();

  let out_dir = hls_root().join(&spec_req.id);
  fs::create_dir_all(&out_dir)?;
  let playlist = out_dir.join("index.m3u8");
  let segment = out_dir.join("segment_%05d.ts");

  let mut last_err: Option<anyhow::Error> = None;
  for name in if profile.presets.is_empty() {
    vec!["h264_ts_default".into()]
  } else {
    profile.presets.clone()
  } {
    let Some(preset) = compat::preset::get_preset(&name) else {
      warn!(preset=%name, "unknown preset, skip");
      continue;
    };

    let adapter = compat::adapter::find_adapter(pr.vendor_hint.as_deref());
    let mut tuned = adapter.adjust(preset.clone(), &pr);
    if tuned.codec != spec_req.codec {
      tuned.codec = spec_req.codec.clone();
    }
    if tuned.container != spec_req.container {
      tuned.container = spec_req.container.clone();
    }

    let codec = tuned.codec.clone();
    let container = tuned.container.clone();
    let latency = tuned.latency_ms;
    let parse_opts = tuned.parse_opts.clone();

    let args = build_pipeline_args(
      &codec,
      &container,
      &spec_req.uri,
      latency,
      &parse_opts,
      playlist
        .to_str()
        .ok_or_else(|| anyhow!("bad playlist path"))?,
      segment
        .to_str()
        .ok_or_else(|| anyhow!("bad segment path"))?,
    );

    info!(id=%spec_req.id, preset=%tuned.name, args=?args, "trying FFmpeg pipeline");

    match Command::new("ffmpeg")
      .args(&args)
      .stdout(Stdio::inherit())
      .stderr(Stdio::inherit())
      .spawn()
    {
      Ok(mut child) => {
        let ok = wait_for_hls_ready(&out_dir, readiness_timeout()).await;
        if ok {
          let status = StreamStatus {
            id: spec_req.id.clone(),
            uri: spec_req.uri.clone(),
            codec: match codec {
              Codec::H264 => "h264".into(),
              Codec::H265 => "h265".into(),
            },
            container: match container {
              Container::Ts => "ts".into(),
              Container::Fmp4 => "fmp4".into(),
            },
            running: true,
            playlist: playlist.clone(),
            output_dir: out_dir.clone(),
          };
          {
            let mut reg = REGISTRY.lock().await;
            reg.insert(
              spec_req.id.clone(),
              (
                child,
                status,
                StreamSpec {
                  id: spec_req.id.clone(),
                  uri: spec_req.uri.clone(),
                  codec,
                  container,
                },
              ),
            );
          }
          STREAMS_RUNNING.inc();

          {
            let dir_for_upload = out_dir.clone();
            let id_for_upload = spec_req.id.clone();
            tokio::spawn(async move {
              let mut cfg = UploaderConfig::default();
              cfg.prefix = id_for_upload.clone();
              if let Err(e) = storage::watch_and_upload(dir_for_upload, cfg, id_for_upload).await {
                error!("uploader error: {e}");
              }
            });
          }

          info!(id=%spec_req.id, preset=%tuned.name, "pipeline ready");
          return Ok(());
        } else {
          let _ = child.kill();
          let _ = child.wait();
          last_err = Some(anyhow!("preset '{}' produced no HLS in time", tuned.name));
          continue;
        }
      }
      Err(e) => {
        warn!(preset=%tuned.name, "spawn failed: {}", e);
        last_err = Some(anyhow!(e));
        continue;
      }
    }
  }

  Err(last_err.unwrap_or_else(|| anyhow!("no working preset found")))
}

async fn wait_for_hls_ready(dir: &PathBuf, timeout: Duration) -> bool {
  use std::fs;

  let deadline = Instant::now() + timeout;

  while Instant::now() < deadline {
    let m3u8 = dir.join("index.m3u8");
    if m3u8.exists() {
      if let Ok(rd) = fs::read_dir(dir) {
        let mut has_segment = false;
        for ent in rd.flatten() {
          let p = ent.path();
          if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
            let ext = ext.to_ascii_lowercase(); // ← owned String
            if ext == "ts" || ext == "m4s" {
              has_segment = true;
              break;
            }
          }
        }
        if has_segment {
          return true;
        }
      }
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
  }
  false
}

pub async fn stop_stream(id: &str) -> Result<()> {
  let mut reg = REGISTRY.lock().await;
  if let Some((mut child, _status, _spec)) = reg.remove(id) {
    let _ = child.kill();
    STREAMS_RUNNING.dec();
    Ok(())
  } else {
    Err(anyhow!("stream '{}' not found", id))
  }
}

pub async fn list_streams() -> Vec<StreamStatus> {
  let mut reg = REGISTRY.lock().await;
  let mut to_remove = vec![];
  for (id, (child, status, _spec)) in reg.iter_mut() {
    if let Ok(Some(_exit)) = child.try_wait() {
      status.running = false;
      to_remove.push(id.clone());
    } else {
      status.running = true;
    }
  }
  for id in to_remove {
    reg.remove(&id);
    STREAMS_RUNNING.dec();
  }

  reg.values().map(|(_c, s, _)| s.clone()).collect()
}

use super::{build_pipeline_args, hls_root, Codec, Container};
use crate::compat;
use crate::metrics::{FFMPEG_CRASHES_TOTAL, FFMPEG_RESTARTS_TOTAL, STREAMS_RUNNING};
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
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

// Maximum concurrent streams to prevent OOM
const MAX_CONCURRENT_STREAMS: usize = 1000;

// FFmpeg restart policy configuration
const MAX_RESTART_ATTEMPTS: u32 = 5;
const INITIAL_RESTART_DELAY_SECS: u64 = 2;
const MAX_RESTART_DELAY_SECS: u64 = 60;

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

struct StreamEntry {
  child: Child,
  status: StreamStatus,
  spec: StreamSpec,
  upload_handle: Option<JoinHandle<()>>,
  restart_count: u32,
  monitor_handle: Option<JoinHandle<()>>,
}

static REGISTRY: Lazy<Mutex<HashMap<String, StreamEntry>>> =
  Lazy::new(|| Mutex::new(HashMap::new()));

fn readiness_timeout() -> Duration {
  std::env::var("HLS_READY_TIMEOUT_SECS")
    .ok()
    .and_then(|v| v.parse::<u64>().ok())
    .map(Duration::from_secs)
    .unwrap_or_else(|| Duration::from_secs(20))
}

/// Calculate exponential backoff delay for restart attempts
fn calculate_restart_delay(attempt: u32) -> Duration {
  let delay_secs = INITIAL_RESTART_DELAY_SECS * 2u64.pow(attempt);
  Duration::from_secs(delay_secs.min(MAX_RESTART_DELAY_SECS))
}

/// Spawn a monitor task to detect FFmpeg crashes and restart with exponential backoff
fn spawn_monitor_task(stream_id: String) -> JoinHandle<()> {
  tokio::spawn(async move {
    loop {
      tokio::time::sleep(Duration::from_secs(5)).await;

      let should_restart = {
        let mut reg = REGISTRY.lock().await;
        if let Some(entry) = reg.get_mut(&stream_id) {
          // Check if child process has exited
          match entry.child.try_wait() {
            Ok(Some(exit_status)) => {
              error!(
                id = %stream_id,
                exit_code = ?exit_status.code(),
                restart_count = entry.restart_count,
                "FFmpeg pipeline crashed"
              );
              FFMPEG_CRASHES_TOTAL.inc();

              // Check if we should restart
              if entry.restart_count < MAX_RESTART_ATTEMPTS {
                entry.restart_count += 1;
                true
              } else {
                warn!(
                  id = %stream_id,
                  max_attempts = MAX_RESTART_ATTEMPTS,
                  "Maximum restart attempts reached, giving up"
                );
                entry.status.running = false;
                false
              }
            }
            Ok(None) => {
              // Process still running
              false
            }
            Err(e) => {
              warn!(id = %stream_id, error = %e, "Failed to check process status");
              false
            }
          }
        } else {
          // Stream entry removed, exit monitor
          return;
        }
      };

      if should_restart {
        let restart_count = {
          let reg = REGISTRY.lock().await;
          reg.get(&stream_id).map(|e| e.restart_count).unwrap_or(0)
        };

        let delay = calculate_restart_delay(restart_count - 1);
        info!(
          id = %stream_id,
          attempt = restart_count,
          delay_secs = delay.as_secs(),
          "Scheduling FFmpeg pipeline restart"
        );

        FFMPEG_RESTARTS_TOTAL.inc();
        tokio::time::sleep(delay).await;

        // Attempt restart
        let spec = {
          let reg = REGISTRY.lock().await;
          reg.get(&stream_id).map(|e| e.spec.clone())
        };

        if let Some(spec) = spec {
          info!(id = %stream_id, "Attempting to restart FFmpeg pipeline");
          if let Err(e) = restart_stream_internal(&spec).await {
            error!(id = %stream_id, error = %e, "Failed to restart FFmpeg pipeline");
          }
        } else {
          return;
        }
      }
    }
  })
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
          // Spawn upload task
          let dir_for_upload = out_dir.clone();
          let id_for_upload = spec_req.id.clone();
          let upload_handle = tokio::spawn(async move {
            let mut cfg = UploaderConfig::default();
            cfg.prefix = id_for_upload.clone();
            if let Err(e) = storage::watch_and_upload(dir_for_upload, cfg, id_for_upload).await {
              error!("uploader error: {e}");
            }
          });

          // Spawn monitor task for automatic restart
          let monitor_handle = spawn_monitor_task(spec_req.id.clone());

          {
            let mut reg = REGISTRY.lock().await;
            reg.insert(
              spec_req.id.clone(),
              StreamEntry {
                child,
                status,
                spec: StreamSpec {
                  id: spec_req.id.clone(),
                  uri: spec_req.uri.clone(),
                  codec,
                  container,
                },
                upload_handle: Some(upload_handle),
                restart_count: 0,
                monitor_handle: Some(monitor_handle),
              },
            );
          }
          STREAMS_RUNNING.inc();

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

/// Internal function to restart a stream (called by monitor task)
async fn restart_stream_internal(spec: &StreamSpec) -> Result<()> {
  // First stop the existing stream (clean up resources)
  let _ = stop_stream(&spec.id).await;

  // Wait a bit before restarting
  tokio::time::sleep(Duration::from_millis(500)).await;

  // Start the stream again with the same spec
  start_stream(spec).await
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
  if let Some(mut entry) = reg.remove(id) {
    // Kill FFmpeg process
    let _ = entry.child.kill();

    // Cancel upload task if it exists
    if let Some(handle) = entry.upload_handle {
      handle.abort();
      info!(id=%id, "upload task cancelled");
    }

    // Cancel monitor task if it exists
    if let Some(handle) = entry.monitor_handle {
      handle.abort();
      info!(id=%id, "monitor task cancelled");
    }

    STREAMS_RUNNING.dec();
    Ok(())
  } else {
    Err(anyhow!("stream '{}' not found", id))
  }
}

pub async fn list_streams() -> Vec<StreamStatus> {
  let mut reg = REGISTRY.lock().await;
  let mut to_remove = vec![];
  for (id, entry) in reg.iter_mut() {
    if let Ok(Some(_exit)) = entry.child.try_wait() {
      entry.status.running = false;
      to_remove.push(id.clone());
    } else {
      entry.status.running = true;
    }
  }
  for id in to_remove {
    if let Some(entry) = reg.remove(&id) {
      // Cancel upload task on stream exit
      if let Some(handle) = entry.upload_handle {
        handle.abort();
      }
      // Cancel monitor task on stream exit
      if let Some(handle) = entry.monitor_handle {
        handle.abort();
      }
      STREAMS_RUNNING.dec();
    }
  }

  reg.values().map(|entry| entry.status.clone()).collect()
}

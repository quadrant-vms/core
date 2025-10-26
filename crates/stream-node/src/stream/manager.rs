use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use std::{collections::HashMap, fs, path::PathBuf, process::{Child, Command, Stdio}, time::Duration};
use tokio::sync::Mutex;
use tracing::{info, warn, error};
use crate::metrics::{STREAMS_RUNNING, RESTARTS_TOTAL};
use crate::storage::{self, S3Config as UploaderConfig};
use super::{Codec, Container, hls_root, build_pipeline_args};

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

#[derive(Clone, Debug)]
pub struct RestartPolicy {
    pub max_retries: u32,
    pub backoff_start_ms: u64,
    pub backoff_max_ms: u64,
}
impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            max_retries: env_u32("RESTART_MAX_RETRIES", 5),
            backoff_start_ms: env_u64("RESTART_BACKOFF_MS_START", 500),
            backoff_max_ms: env_u64("RESTART_BACKOFF_MS_MAX", 10_000),
        }
    }
}
fn env_u32(key: &str, def: u32) -> u32 { std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(def) }
fn env_u64(key: &str, def: u64) -> u64 { std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(def) }

static REGISTRY: Lazy<Mutex<HashMap<String, (Child, StreamStatus, StreamSpec)>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub async fn start_stream(spec: &StreamSpec) -> Result<()> {
    {
        let reg = REGISTRY.lock().await;
        if reg.contains_key(&spec.id) {
            return Err(anyhow!("stream '{}' already running", spec.id));
        }
    }

    let out_dir = hls_root().join(&spec.id);
    fs::create_dir_all(&out_dir)?;
    let playlist = out_dir.join("index.m3u8");
    let segment  = out_dir.join("segment_%05d.ts");

    let args = build_pipeline_args(
        &spec.codec,
        &spec.container,
        &spec.uri,
        playlist.to_str().ok_or_else(|| anyhow!("bad playlist path"))?,
        segment.to_str().ok_or_else(|| anyhow!("bad segment path"))?,
    );

    info!(id=%spec.id, uri=%spec.uri, args=?args, "starting gst-launch-1.0");
    let child = Command::new("gst-launch-1.0")
        .args(&args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| anyhow!("spawn gst-launch-1.0 failed: {}", e))?;

    let status = StreamStatus {
        id: spec.id.clone(),
        uri: spec.uri.clone(),
        codec: match spec.codec { Codec::H264 => "h264".into(), Codec::H265 => "h265".into() },
        container: match spec.container { Container::Ts => "ts".into(), Container::Fmp4 => "fmp4".into() },
        running: true,
        playlist: playlist.clone(),
        output_dir: out_dir.clone(),
    };

    {
        let mut reg = REGISTRY.lock().await;
        reg.insert(spec.id.clone(), (child, status, spec.clone()));
        STREAMS_RUNNING.inc();
    }

    {
        let dir_for_upload = out_dir.clone();
        let id_for_upload = spec.id.clone();
        tokio::spawn(async move {
            let mut cfg = UploaderConfig::default();
            cfg.prefix = id_for_upload.clone();
            if let Err(e) = storage::watch_and_upload(dir_for_upload, cfg, id_for_upload).await {
                error!("uploader error: {e}");
            }
        });
    }

    // Supervisor
    let policy = RestartPolicy::default();
    let id_for_task = spec.id.clone();
    tokio::spawn(async move {
        supervise_loop(id_for_task, policy).await;
    });

    Ok(())
}

async fn supervise_loop(id: String, policy: RestartPolicy) {
    let mut attempts: u32 = 0;

    loop {
        // wait current child
        let child_exit = {
            let mut reg = REGISTRY.lock().await;
            if let Some((child, _status, _spec)) = reg.get_mut(&id) {
                match child.try_wait() {
                    Ok(Some(exit)) => Some(exit),
                    Ok(None) => None, // still running
                    Err(e) => { error!(%id, "try_wait error: {e}"); None }
                }
            } else { return; } // stream removed (stop called)
        };

        if child_exit.is_none() {
            // still running; sleep a bit
            tokio::time::sleep(Duration::from_millis(500)).await;
            continue;
        }

        // process exited
        attempts += 1;
        if attempts > policy.max_retries {
            warn!(%id, "max retries reached, giving up");
            let mut reg = REGISTRY.lock().await;
            reg.remove(&id);
            STREAMS_RUNNING.dec();
            return;
        }

        // backoff
        let backoff = (policy.backoff_start_ms << (attempts.saturating_sub(1)))
            .min(policy.backoff_max_ms);
        warn!(%id, attempts, backoff, "pipeline exited, restarting...");
        tokio::time::sleep(Duration::from_millis(backoff)).await;

        // restart with same spec
        let (new_child, new_status, spec_clone) = {
            let reg_snapshot = {
                let reg = REGISTRY.lock().await;
                reg.get(&id).map(|(_, _st, sp)| sp.clone())
            };

            // If registry no longer has it (stop called), exit
            let spec = match reg_snapshot {
                Some(s) => s,
                None => return,
            };

            // rebuild args (paths unchanged)
            let out_dir = hls_root().join(&spec.id);
            let playlist = out_dir.join("index.m3u8");
            let segment  = out_dir.join("segment_%05d.ts");

            let args = build_pipeline_args(
                &spec.codec, &spec.container, &spec.uri,
                playlist.to_str().unwrap(), segment.to_str().unwrap(),
            );

            match Command::new("gst-launch-1.0")
                .args(&args)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()
            {
                Ok(child) => {
                    let st = StreamStatus {
                        id: spec.id.clone(),
                        uri: spec.uri.clone(),
                        codec: match spec.codec { Codec::H264 => "h264".into(), Codec::H265 => "h265".into() },
                        container: match spec.container { Container::Ts => "ts".into(), Container::Fmp4 => "fmp4".into() },
                        running: true,
                        playlist: playlist.clone(),
                        output_dir: out_dir.clone(),
                    };
                    RESTARTS_TOTAL.inc();
                    (child, st, spec)
                }
                Err(e) => {
                    error!(%id, "restart spawn failed: {e}");
                    // try again next loop
                    continue;
                }
            }
        };

        let mut reg = REGISTRY.lock().await;
        reg.insert(id.clone(), (new_child, new_status, spec_clone));
    }
}

pub async fn stop_stream(id: &str) -> Result<()> {
    let mut reg = REGISTRY.lock().await;
    if let Some((mut child, _status, _spec)) = reg.remove(id) {
        child.kill().ok();
        STREAMS_RUNNING.dec();
        Ok(())
    } else {
        Err(anyhow!("stream '{}' not found", id))
    }
}

pub async fn list_streams() -> Vec<StreamStatus> {
    let mut reg = REGISTRY.lock().await;

    // refresh running flags, prune exited ones (supervisor will restart)
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
    }

    reg.values().map(|(_c, s, _)| s.clone()).collect()
}
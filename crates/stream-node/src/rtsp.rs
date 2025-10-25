use std::process::{Command, Stdio};
use std::fs;
use tracing::{info, error};
use anyhow::{Result, anyhow};

#[derive(Clone, Debug)]
pub enum Codec {
    H264,
    H265,
}

#[derive(Clone, Debug)]
pub struct StreamSpec {
    pub id: String,
    pub uri: String,
    pub codec: Codec, // 新增：讓我們能選 H264 / H265
}

pub fn start_stream(spec: &StreamSpec) -> Result<()> {
    let output_dir = format!("/data/hls/{}", spec.id);
    fs::create_dir_all(&output_dir)?;

    let playlist = format!("{}/index.m3u8", output_dir);
    let segment  = format!("{}/segment_%05d.ts", output_dir);

    // Build args as owned Strings to avoid borrow-of-temporary issues
    let mut args: Vec<String> = Vec::new();

    // source
    args.push("rtspsrc".into());
    args.push(format!("location={}", spec.uri));
    args.push("latency=0".into());
    args.push("!".into());

    // depay + parse by codec
    match spec.codec {
        Codec::H264 => {
            args.push("rtph264depay".into());
            args.push("!".into());
            args.push("h264parse".into());
            args.push("!".into());
        }
        Codec::H265 => {
            args.push("rtph265depay".into());
            args.push("!".into());
            args.push("h265parse".into());
            args.push("!".into());
        }
    }

    // mux to MPEG-TS (simplest for MVP)
    args.push("mpegtsmux".into());
    args.push("!".into());

    // HLS sink (hlssink2 is recommended)
    args.push("hlssink2".into());
    args.push(format!("max-files={}", 5));
    args.push(format!("target-duration={}", 2));
    args.push(format!("playlist-location={}", playlist));
    args.push(format!("location={}", segment));
    args.push("use-mpegts=true".into()); // 我們前面已經用 mpegtsmux

    info!(?spec, "Launching GStreamer: gst-launch-1.0 {}", args.join(" "));
    let mut child = Command::new("gst-launch-1.0")
        .args(&args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| anyhow!("Failed to spawn gst-launch-1.0: {}", e))?;

    // 非阻塞監看（MVP）
    std::thread::spawn(move || {
        match child.wait() {
            Ok(status) => info!(?status, "stream finished"),
            Err(e) => error!("stream error: {}", e),
        }
    });

    Ok(())
}

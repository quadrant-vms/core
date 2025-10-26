use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum Codec { H264, H265 }

#[derive(Clone, Debug)]
pub enum Container { Ts, Fmp4 }

pub fn hls_root() -> PathBuf {
    std::env::var("HLS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/data/hls"))
}

pub fn build_pipeline_args(
    codec: &Codec,
    container: &Container,
    uri: &str,
    playlist: &str,
    segment: &str,
) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();

    args.push("rtspsrc".into());
    args.push(format!("location={}", uri));
    args.push("latency=0".into());
    args.push("!".into());

    match codec {
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

    // Container branch
    match container {
        Container::Ts => {
            args.push("mpegtsmux".into());
            args.push("!".into());
            args.push("hlssink2".into());
            args.push("use-mpegts=true".into());
            args.push(format!("max-files={}", 5));
            args.push(format!("target-duration={}", 2));
            args.push(format!("playlist-location={}", playlist));
            args.push(format!("location={}", segment));
        }
        Container::Fmp4 => {
            args.push("mp4mux".into());
            args.push("fragment-duration=2000000".into()); // 2s in ns
            args.push("streamable=true".into());
            args.push("!".into());
            args.push("hlssink2".into());
            args.push("use-mpegts=false".into());
            args.push(format!("max-files={}", 5));
            args.push(format!("target-duration={}", 2));
            args.push(format!("playlist-location={}", playlist));
            args.push(format!("location={}", segment.replace(".ts", ".m4s")));
        }
    }

    args
}
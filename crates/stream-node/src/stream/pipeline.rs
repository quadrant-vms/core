use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum Codec { H264, H265 }

#[derive(Clone, Debug)]
pub enum Container { Ts, Fmp4 }

pub fn hls_root() -> PathBuf {
    if let Ok(v) = std::env::var("HLS_ROOT") { return PathBuf::from(v); }
    if std::path::Path::new("/.dockerenv").exists() || std::env::var("CONTAINERIZED").is_ok() {
        PathBuf::from("/data/hls")
    } else {
        PathBuf::from("./data/hls")
    }
}

pub fn build_pipeline_args(
    codec: &Codec,
    container: &Container,
    uri: &str,
    latency_ms: u32,
    parse_opts: &[String],      // e.g. ["config-interval=-1"]
    playlist: &str,
    segment: &str,
) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();

    // Source
    args.push("rtspsrc".into());
    args.push(format!("location={}", uri));
    args.push(format!("latency={}", latency_ms));
    args.push("!".into());

    match codec {
        Codec::H264 => {
            args.push("rtph264depay".into());
            args.push("!".into());
            args.push("h264parse".into());
            for opt in parse_opts { args.push(opt.clone()); }
            args.push("!".into());
        }
        Codec::H265 => {
            args.push("rtph265depay".into());
            args.push("!".into());
            args.push("h265parse".into());
            for opt in parse_opts { args.push(opt.clone()); }
            args.push("!".into());
        }
    }

    match container {
        Container::Ts => {
            args.push("mpegtsmux".into());
            args.push("!".into());
            args.push("hlssink".into());
            args.push(format!("max-files={}", 5));
            args.push(format!("target-duration={}", 2));
            args.push(format!("playlist-location={}", playlist));
            args.push(format!("location={}", segment));
        }
        Container::Fmp4 => {
            args.push("mp4mux".into());
            args.push("fragment-duration=2000000".into()); // 2s (ns)
            args.push("streamable=true".into());
            args.push("!".into());
            args.push("hlssink2".into());
            args.push(format!("max-files={}", 5));
            args.push(format!("target-duration={}", 2));
            args.push(format!("playlist-location={}", playlist));
            let loc = if segment.ends_with(".ts") { segment.replace(".ts", ".m4s") } else { format!("{segment}.m4s") };
            args.push(format!("location={}", loc));
        }
    }

    args
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn build_h264_ts_args_contains_expected_elements() {
        let args = build_pipeline_args(
            &Codec::H264, &Container::Ts, "rtsp://x", 0,
            &vec!["config-interval=-1".into()],
            "/p.m3u8", "/seg_%05d.ts",
        );
        let joined = args.join(" ");
        assert!(joined.contains("rtph264depay"));
        assert!(joined.contains("h264parse"));
        assert!(joined.contains("mpegtsmux"));
        assert!(joined.contains("hlssink"));
        assert!(joined.contains("playlist-location=/p.m3u8"));
        assert!(joined.contains("location=/seg_%05d.ts"));
    }
}
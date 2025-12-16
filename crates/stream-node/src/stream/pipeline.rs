use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Codec {
  H264,
  H265,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Container {
  Ts,
  Fmp4,
}

pub fn hls_root() -> PathBuf {
  if let Ok(v) = std::env::var("HLS_ROOT") {
    return PathBuf::from(v);
  }
  if std::path::Path::new("/.dockerenv").exists() || std::env::var("CONTAINERIZED").is_ok() {
    PathBuf::from("/data/hls")
  } else {
    PathBuf::from("./data/hls")
  }
}

/// Build FFmpeg command arguments for HLS transcoding
///
/// Creates FFmpeg arguments to convert RTSP stream to HLS format:
/// - Uses TCP transport for RTSP (more reliable than UDP)
/// - Copies video codec (no re-encoding)
/// - Generates HLS playlist with 2-second segments
/// - Keeps last 5 segments in playlist
pub fn build_pipeline_args(
  _codec: &Codec, // Not used in FFmpeg (codec is copied as-is)
  container: &Container,
  uri: &str,
  _latency_ms: u32, // Not used in FFmpeg (GStreamer legacy parameter)
  _parse_opts: &[String], // Not used in FFmpeg (GStreamer legacy parameter)
  playlist: &str,
  segment: &str,
) -> Vec<String> {
  let mut args: Vec<String> = Vec::new();

  // Input options
  args.push("-rtsp_transport".into());
  args.push("tcp".into());
  args.push("-i".into());
  args.push(uri.to_string());

  // Codec selection (copy to avoid re-encoding)
  args.push("-c:v".into());
  args.push("copy".into());
  args.push("-c:a".into());
  args.push("copy".into());

  // HLS output format
  args.push("-f".into());
  args.push("hls".into());

  // HLS segment duration (2 seconds)
  args.push("-hls_time".into());
  args.push("2".into());

  // Keep last 5 segments
  args.push("-hls_list_size".into());
  args.push("5".into());

  // Segment filename pattern
  args.push("-hls_segment_filename".into());
  let segment_path = match container {
    Container::Ts => segment.to_string(),
    Container::Fmp4 => {
      if segment.ends_with(".ts") {
        segment.replace(".ts", ".m4s")
      } else {
        format!("{}.m4s", segment)
      }
    }
  };
  args.push(segment_path);

  // HLS flags for better compatibility
  args.push("-hls_flags".into());
  match container {
    Container::Ts => {
      // Standard TS segments
      args.push("delete_segments".into());
    }
    Container::Fmp4 => {
      // Fragmented MP4 segments (fMP4)
      args.push("delete_segments+independent_segments".into());
      args.push("-hls_segment_type".into());
      args.push("fmp4".into());
    }
  }

  // Playlist location (output file)
  args.push(playlist.to_string());

  args
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn build_h264_ts_args_contains_expected_elements() {
    let args = build_pipeline_args(
      &Codec::H264,
      &Container::Ts,
      "rtsp://x",
      0,
      &vec!["config-interval=-1".into()],
      "/p.m3u8",
      "/seg_%05d.ts",
    );
    let joined = args.join(" ");
    // FFmpeg arguments
    assert!(joined.contains("-rtsp_transport"));
    assert!(joined.contains("tcp"));
    assert!(joined.contains("-i"));
    assert!(joined.contains("rtsp://x"));
    assert!(joined.contains("-c:v"));
    assert!(joined.contains("copy"));
    assert!(joined.contains("-f"));
    assert!(joined.contains("hls"));
    assert!(joined.contains("-hls_segment_filename"));
    assert!(joined.contains("/seg_%05d.ts"));
    assert!(joined.contains("/p.m3u8"));
  }

  #[test]
  fn build_fmp4_args_uses_m4s_extension() {
    let args = build_pipeline_args(
      &Codec::H264,
      &Container::Fmp4,
      "rtsp://test",
      0,
      &[],
      "/playlist.m3u8",
      "/seg_%05d.ts",
    );
    let joined = args.join(" ");
    // Should convert .ts to .m4s for fMP4
    assert!(joined.contains("/seg_%05d.m4s"));
    assert!(joined.contains("-hls_segment_type"));
    assert!(joined.contains("fmp4"));
  }
}

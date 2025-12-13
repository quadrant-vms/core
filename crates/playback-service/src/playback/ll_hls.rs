use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tokio::fs;
use tracing::{debug, info, warn};

/// LL-HLS configuration
#[derive(Debug, Clone)]
pub struct LlHlsConfig {
    /// Target partial segment duration (seconds)
    pub part_target_duration: f64,
    /// Number of parts per segment
    pub parts_per_segment: usize,
    /// Enable HTTP/2 server push for preload hints
    pub enable_server_push: bool,
    /// Enable blocking playlist reload
    pub enable_blocking_reload: bool,
    /// Maximum blocking duration (seconds)
    pub max_blocking_duration: f64,
}

impl Default for LlHlsConfig {
    fn default() -> Self {
        Self {
            part_target_duration: 0.33, // 330ms parts for ~1s latency
            parts_per_segment: 6,        // 6 parts = ~2s segments
            enable_server_push: false,   // Requires HTTP/2
            enable_blocking_reload: true,
            max_blocking_duration: 3.0,  // Wait up to 3s for new parts
        }
    }
}

/// LL-HLS playlist generator
pub struct LlHlsPlaylistGenerator {
    config: LlHlsConfig,
    hls_root: PathBuf,
}

impl LlHlsPlaylistGenerator {
    pub fn new(config: LlHlsConfig, hls_root: PathBuf) -> Self {
        Self { config, hls_root }
    }

    /// Generate LL-HLS master playlist
    pub async fn generate_master_playlist(
        &self,
        stream_id: &str,
        variants: Vec<HlsVariant>,
    ) -> Result<String> {
        let mut playlist = String::new();

        playlist.push_str("#EXTM3U\n");
        playlist.push_str("#EXT-X-VERSION:9\n"); // LL-HLS requires version 9+

        for variant in variants {
            playlist.push_str(&format!(
                "#EXT-X-STREAM-INF:BANDWIDTH={},RESOLUTION={}\n",
                variant.bandwidth, variant.resolution
            ));
            playlist.push_str(&format!("{}/playlist.m3u8\n", variant.name));
        }

        Ok(playlist)
    }

    /// Generate LL-HLS media playlist
    pub async fn generate_media_playlist(
        &self,
        stream_path: &Path,
        sequence_number: u64,
        blocking_params: Option<BlockingParams>,
    ) -> Result<PlaylistResponse> {
        debug!("generating LL-HLS media playlist for {:?}", stream_path);

        // Scan for segments and partial segments
        let segments = self.scan_segments(stream_path).await?;

        if segments.is_empty() {
            return Err(anyhow!("No segments found"));
        }

        // If blocking mode and requested sequence not ready, wait
        if let Some(params) = blocking_params {
            if params.msn > segments.len() as u64 {
                // Wait for new segment
                let wait_result = self.wait_for_segment(
                    stream_path,
                    params.msn,
                    Duration::from_secs_f64(self.config.max_blocking_duration)
                ).await;

                if wait_result.is_ok() {
                    // Re-scan after waiting
                    let new_segments = self.scan_segments(stream_path).await?;
                    return self.build_playlist(&new_segments, sequence_number);
                }
            }
        }

        self.build_playlist(&segments, sequence_number)
    }

    /// Build the actual playlist content
    fn build_playlist(&self, segments: &[Segment], sequence_number: u64) -> Result<PlaylistResponse> {
        let mut playlist = String::new();

        // Header
        playlist.push_str("#EXTM3U\n");
        playlist.push_str("#EXT-X-VERSION:9\n");
        playlist.push_str(&format!("#EXT-X-TARGETDURATION:{}\n",
            (self.config.part_target_duration * self.config.parts_per_segment as f64).ceil() as u64));
        playlist.push_str(&format!("#EXT-X-PART-INF:PART-TARGET={:.3}\n", self.config.part_target_duration));
        playlist.push_str(&format!("#EXT-X-MEDIA-SEQUENCE:{}\n", sequence_number));
        playlist.push_str("#EXT-X-SERVER-CONTROL:CAN-BLOCK-RELOAD=YES");
        playlist.push_str(&format!(",PART-HOLD-BACK={:.3}", self.config.part_target_duration * 2.0));
        playlist.push_str("\n");

        // Segments
        for (idx, segment) in segments.iter().enumerate() {
            // Add partial segments (for incomplete segments)
            if !segment.parts.is_empty() {
                for part in &segment.parts {
                    playlist.push_str(&format!(
                        "#EXT-X-PART:DURATION={:.3},URI=\"{}\"\n",
                        part.duration, part.uri
                    ));
                }
            }

            // Add full segment
            if segment.complete {
                playlist.push_str(&format!("#EXTINF:{:.3},\n", segment.duration));
                playlist.push_str(&format!("{}\n", segment.uri));
            }

            // Add preload hint for next part (last segment only)
            if idx == segments.len() - 1 && !segment.complete {
                if let Some(next_part_num) = segment.next_part_number() {
                    let next_part_uri = format!("fileSequence{}.{}.m4s", segment.sequence, next_part_num);
                    playlist.push_str(&format!(
                        "#EXT-X-PRELOAD-HINT:TYPE=PART,URI=\"{}\"\n",
                        next_part_uri
                    ));
                }
            }
        }

        Ok(PlaylistResponse {
            content: playlist,
            modified: true,
        })
    }

    /// Scan directory for segments and partial segments
    async fn scan_segments(&self, stream_path: &Path) -> Result<Vec<Segment>> {
        let mut segments: Vec<Segment> = Vec::new();
        let mut entries = fs::read_dir(stream_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            // Parse segment files (e.g., fileSequence0.m4s, fileSequence0.0.m4s)
            if filename.starts_with("fileSequence") && filename.ends_with(".m4s") {
                if let Some(segment) = self.parse_segment_file(&path).await? {
                    // Insert or update segment
                    if let Some(existing) = segments.iter_mut().find(|s| s.sequence == segment.sequence) {
                        // Update with parts
                        if !segment.parts.is_empty() {
                            existing.parts.extend(segment.parts);
                        }
                        existing.complete = segment.complete || existing.complete;
                    } else {
                        segments.push(segment);
                    }
                }
            }
        }

        // Sort by sequence number
        segments.sort_by_key(|s| s.sequence);

        Ok(segments)
    }

    /// Parse segment filename and extract metadata
    async fn parse_segment_file(&self, path: &Path) -> Result<Option<Segment>> {
        let filename = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| anyhow!("Invalid filename"))?;

        // Extract sequence number and part number
        // Format: fileSequence{seq}.m4s or fileSequence{seq}.{part}.m4s
        let name = filename.trim_start_matches("fileSequence").trim_end_matches(".m4s");
        let parts: Vec<&str> = name.split('.').collect();

        match parts.len() {
            1 => {
                // Full segment
                let sequence: u64 = parts[0].parse()?;
                let metadata = fs::metadata(path).await?;
                let duration = self.estimate_duration(metadata.len());

                Ok(Some(Segment {
                    sequence,
                    uri: filename.to_string(),
                    duration,
                    complete: true,
                    parts: Vec::new(),
                }))
            }
            2 => {
                // Partial segment
                let sequence: u64 = parts[0].parse()?;
                let part_num: u64 = parts[1].parse()?;
                let metadata = fs::metadata(path).await?;
                let duration = self.estimate_duration(metadata.len());

                let part = Part {
                    number: part_num,
                    uri: filename.to_string(),
                    duration,
                    independent: true, // Assume IDR frames for now
                };

                Ok(Some(Segment {
                    sequence,
                    uri: format!("fileSequence{}.m4s", sequence),
                    duration: 0.0,
                    complete: false,
                    parts: vec![part],
                }))
            }
            _ => Ok(None),
        }
    }

    /// Estimate duration based on file size (rough approximation)
    fn estimate_duration(&self, file_size: u64) -> f64 {
        // Rough estimate: assume 1MB/sec bitrate
        let bitrate = 1_000_000.0; // 1 Mbps
        (file_size as f64 * 8.0) / bitrate
    }

    /// Wait for a specific segment to become available
    async fn wait_for_segment(
        &self,
        stream_path: &Path,
        target_sequence: u64,
        timeout: Duration,
    ) -> Result<()> {
        let start = SystemTime::now();

        loop {
            let segments = self.scan_segments(stream_path).await?;
            if segments.len() as u64 >= target_sequence {
                return Ok(());
            }

            if start.elapsed()? > timeout {
                return Err(anyhow!("Timeout waiting for segment {}", target_sequence));
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

/// HLS variant stream
#[derive(Debug, Clone)]
pub struct HlsVariant {
    pub name: String,
    pub bandwidth: u32,
    pub resolution: String,
}

/// Segment representation
#[derive(Debug, Clone)]
struct Segment {
    sequence: u64,
    uri: String,
    duration: f64,
    complete: bool,
    parts: Vec<Part>,
}

impl Segment {
    fn next_part_number(&self) -> Option<u64> {
        if self.complete {
            None
        } else {
            Some(self.parts.len() as u64)
        }
    }
}

/// Partial segment representation
#[derive(Debug, Clone)]
struct Part {
    number: u64,
    uri: String,
    duration: f64,
    independent: bool, // Contains IDR frame
}

/// Blocking playlist request parameters
#[derive(Debug, Clone)]
pub struct BlockingParams {
    /// Media Sequence Number - segment number to wait for
    pub msn: u64,
    /// Part number within the segment (optional)
    pub part: Option<u64>,
}

/// Playlist generation response
pub struct PlaylistResponse {
    pub content: String,
    pub modified: bool,
}

use anyhow::{Context, Result};
use common::playback::{DvrSegment, DvrWindowInfo};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// DVR buffer manager for tracking HLS segment timeline
#[derive(Clone)]
pub struct DvrBufferManager {
    inner: Arc<RwLock<DvrBufferInner>>,
}

struct DvrBufferInner {
    /// Stream identifier
    stream_id: String,
    /// HLS directory path
    hls_path: PathBuf,
    /// Ordered segment buffer (oldest to newest)
    segments: VecDeque<DvrSegment>,
    /// Maximum buffer duration in seconds
    max_buffer_secs: f64,
    /// Last scan timestamp
    last_scan: Option<SystemTime>,
}

impl DvrBufferManager {
    /// Create a new DVR buffer manager
    pub fn new(stream_id: String, hls_path: PathBuf, max_buffer_secs: f64) -> Self {
        Self {
            inner: Arc::new(RwLock::new(DvrBufferInner {
                stream_id,
                hls_path,
                segments: VecDeque::new(),
                max_buffer_secs,
                last_scan: None,
            })),
        }
    }

    /// Scan HLS directory and update segment timeline
    pub async fn scan_segments(&self) -> Result<()> {
        let mut inner = self.inner.write().await;

        debug!(
            stream_id = %inner.stream_id,
            path = %inner.hls_path.display(),
            "Scanning HLS segments for DVR buffer"
        );

        // Read directory entries
        let mut entries = fs::read_dir(&inner.hls_path)
            .await
            .context("Failed to read HLS directory")?;

        let mut new_segments = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            // Only process .ts and .m4s files (HLS segments)
            if let Some(ext) = path.extension() {
                if ext == "ts" || ext == "m4s" {
                    if let Some(segment) = Self::parse_segment_file(&path).await? {
                        new_segments.push(segment);
                    }
                }
            }
        }

        // Sort by sequence number
        new_segments.sort_by_key(|s| s.sequence);

        // Calculate timestamps based on segment duration
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut current_timestamp = now;

        // Process segments in reverse to assign timestamps from live edge
        for segment in new_segments.iter_mut().rev() {
            segment.end_timestamp = current_timestamp;
            segment.start_timestamp = current_timestamp - segment.duration as u64;
            current_timestamp = segment.start_timestamp;
        }

        // Replace old segments with new scan
        inner.segments.clear();
        for segment in new_segments {
            inner.segments.push_back(segment);
        }

        // Trim segments exceeding buffer duration
        Self::trim_buffer(&mut inner);

        inner.last_scan = Some(SystemTime::now());

        info!(
            stream_id = %inner.stream_id,
            segment_count = inner.segments.len(),
            "DVR buffer scan completed"
        );

        Ok(())
    }

    /// Parse segment file and extract metadata
    async fn parse_segment_file(path: &Path) -> Result<Option<DvrSegment>> {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .context("Invalid filename")?;

        // Parse filename format: segment{seq}.{ext} or {stream_id}_{seq}.{ext}
        // Try to extract sequence number
        let sequence = if let Some(caps) = filename.strip_prefix("segment").and_then(|s| s.split('.').next()) {
            match caps.parse::<u64>() {
                Ok(seq) => seq,
                Err(_) => return Ok(None),
            }
        } else if let Some(parts) = filename.split('_').last().and_then(|s| s.split('.').next()) {
            match parts.parse::<u64>() {
                Ok(seq) => seq,
                Err(_) => return Ok(None),
            }
        } else {
            return Ok(None);
        };

        // Get file metadata for timestamp estimation
        let _metadata = fs::metadata(path).await.ok();

        // Default segment duration (can be refined by parsing m3u8)
        let duration = 6.0; // Typical HLS segment duration

        Ok(Some(DvrSegment {
            filename: filename.to_string(),
            sequence,
            duration,
            start_timestamp: 0, // Will be calculated after sorting
            end_timestamp: 0,   // Will be calculated after sorting
            file_path: path.to_string_lossy().to_string(),
        }))
    }

    /// Trim buffer to maintain max duration
    fn trim_buffer(inner: &mut DvrBufferInner) {
        if inner.segments.is_empty() {
            return;
        }

        // SAFETY: We just checked that segments is not empty above
        let latest = inner.segments.back().expect("BUG: segments should not be empty").end_timestamp;
        let earliest_allowed = latest.saturating_sub(inner.max_buffer_secs as u64);

        while let Some(segment) = inner.segments.front() {
            if segment.end_timestamp < earliest_allowed {
                debug!(
                    stream_id = %inner.stream_id,
                    sequence = segment.sequence,
                    "Trimming old segment from DVR buffer"
                );
                inner.segments.pop_front();
            } else {
                break;
            }
        }
    }

    /// Get DVR window information
    pub async fn get_window(&self, current_position: Option<u64>) -> Result<DvrWindowInfo> {
        let inner = self.inner.read().await;

        if inner.segments.is_empty() {
            return Ok(DvrWindowInfo {
                stream_id: inner.stream_id.clone(),
                earliest_available: 0,
                latest_available: 0,
                buffer_seconds: 0.0,
                current_position,
                live_offset_secs: None,
            });
        }

        // SAFETY: We just checked that segments is not empty above
        let earliest = inner.segments.front().expect("BUG: segments should not be empty").start_timestamp;
        let latest = inner.segments.back().expect("BUG: segments should not be empty").end_timestamp;
        let buffer_seconds = (latest - earliest) as f64;

        let live_offset_secs = current_position.map(|pos| {
            if pos <= latest {
                (latest - pos) as f64
            } else {
                0.0
            }
        });

        Ok(DvrWindowInfo {
            stream_id: inner.stream_id.clone(),
            earliest_available: earliest,
            latest_available: latest,
            buffer_seconds,
            current_position,
            live_offset_secs,
        })
    }

    /// Find segment containing the target timestamp
    pub async fn find_segment_at_timestamp(&self, timestamp: u64) -> Result<Option<DvrSegment>> {
        let inner = self.inner.read().await;

        for segment in inner.segments.iter() {
            if timestamp >= segment.start_timestamp && timestamp <= segment.end_timestamp {
                return Ok(Some(segment.clone()));
            }
        }

        Ok(None)
    }

    /// Get all segments in the buffer
    pub async fn get_segments(&self) -> Result<Vec<DvrSegment>> {
        let inner = self.inner.read().await;
        Ok(inner.segments.iter().cloned().collect())
    }

    /// Get the live edge timestamp
    pub async fn get_live_edge(&self) -> Result<Option<u64>> {
        let inner = self.inner.read().await;
        Ok(inner.segments.back().map(|s| s.end_timestamp))
    }

    /// Update buffer size limit
    pub async fn set_buffer_limit(&self, max_buffer_secs: f64) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.max_buffer_secs = max_buffer_secs;
        Self::trim_buffer(&mut inner);
        Ok(())
    }

    /// Clear the buffer
    pub async fn clear(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.segments.clear();
        info!(stream_id = %inner.stream_id, "DVR buffer cleared");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_dvr_buffer_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = DvrBufferManager::new(
            "test_stream".to_string(),
            temp_dir.path().to_path_buf(),
            300.0,
        );

        let window = manager.get_window(None).await.unwrap();
        assert_eq!(window.stream_id, "test_stream");
        assert_eq!(window.buffer_seconds, 0.0);
    }

    #[tokio::test]
    async fn test_buffer_limit() {
        let temp_dir = TempDir::new().unwrap();
        let manager = DvrBufferManager::new(
            "test_stream".to_string(),
            temp_dir.path().to_path_buf(),
            300.0,
        );

        manager.set_buffer_limit(600.0).await.unwrap();
        // Buffer limit updated successfully
    }

    #[tokio::test]
    async fn test_clear_buffer() {
        let temp_dir = TempDir::new().unwrap();
        let manager = DvrBufferManager::new(
            "test_stream".to_string(),
            temp_dir.path().to_path_buf(),
            300.0,
        );

        manager.clear().await.unwrap();
        let window = manager.get_window(None).await.unwrap();
        assert_eq!(window.buffer_seconds, 0.0);
    }
}

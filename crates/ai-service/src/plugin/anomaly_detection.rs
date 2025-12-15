/// Anomaly detection plugin for temporal and spatial anomaly detection
///
/// This plugin provides two types of anomaly detection:
/// 1. Temporal: Detects unusual patterns in time-series metrics (object counts, activity times)
/// 2. Spatial: Detects unusual objects or behaviors in specific zones
use super::AiPlugin;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use common::ai_tasks::{AiResult, BoundingBox, Detection, VideoFrame};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use tokio::sync::RwLock;

const DEFAULT_HISTORY_SIZE: usize = 100;
const DEFAULT_SENSITIVITY: f32 = 2.0; // Standard deviations for anomaly threshold
const DEFAULT_MIN_SAMPLES: usize = 10; // Minimum samples before anomaly detection

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Zone {
    /// Zone identifier
    pub id: String,

    /// Zone name
    pub name: String,

    /// Bounding box defining the zone
    pub bbox: BoundingBox,

    /// Allowed object classes in this zone (empty = all allowed)
    #[serde(default)]
    pub allowed_classes: Vec<String>,

    /// Restricted object classes in this zone (takes precedence over allowed)
    #[serde(default)]
    pub restricted_classes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyDetectorConfig {
    /// Enable temporal anomaly detection
    #[serde(default = "default_true")]
    pub enable_temporal: bool,

    /// Enable spatial anomaly detection
    #[serde(default = "default_true")]
    pub enable_spatial: bool,

    /// Sensitivity for anomaly detection (standard deviations)
    #[serde(default = "default_sensitivity")]
    pub sensitivity: f32,

    /// History size for temporal analysis
    #[serde(default = "default_history_size")]
    pub history_size: usize,

    /// Minimum samples before anomaly detection starts
    #[serde(default = "default_min_samples")]
    pub min_samples: usize,

    /// Object classes to track (empty = track all)
    #[serde(default)]
    pub tracked_classes: Vec<String>,

    /// Restricted zones for spatial anomaly detection
    #[serde(default)]
    pub zones: Vec<Zone>,

    /// Confidence threshold for detections to analyze
    #[serde(default = "default_confidence")]
    pub confidence_threshold: f32,
}

fn default_true() -> bool {
    true
}

fn default_sensitivity() -> f32 {
    DEFAULT_SENSITIVITY
}

fn default_history_size() -> usize {
    DEFAULT_HISTORY_SIZE
}

fn default_min_samples() -> usize {
    DEFAULT_MIN_SAMPLES
}

fn default_confidence() -> f32 {
    0.5
}

impl Default for AnomalyDetectorConfig {
    fn default() -> Self {
        Self {
            enable_temporal: true,
            enable_spatial: true,
            sensitivity: DEFAULT_SENSITIVITY,
            history_size: DEFAULT_HISTORY_SIZE,
            min_samples: DEFAULT_MIN_SAMPLES,
            tracked_classes: Vec::new(),
            zones: Vec::new(),
            confidence_threshold: 0.5,
        }
    }
}

#[derive(Debug, Clone)]
struct TemporalMetrics {
    /// Historical object counts per class
    class_counts: HashMap<String, VecDeque<u32>>,

    /// Historical total object counts
    total_counts: VecDeque<u32>,

    /// Timestamps of historical samples
    timestamps: VecDeque<u64>,
}

impl TemporalMetrics {
    fn new(history_size: usize) -> Self {
        Self {
            class_counts: HashMap::new(),
            total_counts: VecDeque::with_capacity(history_size),
            timestamps: VecDeque::with_capacity(history_size),
        }
    }

    fn add_sample(&mut self, timestamp: u64, class_counts: &HashMap<String, u32>, max_size: usize) {
        // Add timestamp
        self.timestamps.push_back(timestamp);
        if self.timestamps.len() > max_size {
            self.timestamps.pop_front();
        }

        // Add total count
        let total: u32 = class_counts.values().sum();
        self.total_counts.push_back(total);
        if self.total_counts.len() > max_size {
            self.total_counts.pop_front();
        }

        // Add per-class counts
        for (class, count) in class_counts {
            let history = self.class_counts.entry(class.clone()).or_insert_with(|| VecDeque::with_capacity(max_size));
            history.push_back(*count);
            if history.len() > max_size {
                history.pop_front();
            }
        }

        // Ensure all tracked classes have entries
        for history in self.class_counts.values_mut() {
            while history.len() < self.total_counts.len() {
                history.push_front(0);
            }
        }
    }

    fn calculate_stats(data: &VecDeque<u32>) -> (f32, f32) {
        if data.is_empty() {
            return (0.0, 0.0);
        }

        let sum: u32 = data.iter().sum();
        let mean = sum as f32 / data.len() as f32;

        let variance = data.iter()
            .map(|&x| {
                let diff = x as f32 - mean;
                diff * diff
            })
            .sum::<f32>() / data.len() as f32;

        let std_dev = variance.sqrt();
        (mean, std_dev)
    }

    fn is_anomaly(&self, current_value: u32, data: &VecDeque<u32>, sensitivity: f32, min_samples: usize) -> bool {
        if data.len() < min_samples {
            return false;
        }

        let (mean, std_dev) = Self::calculate_stats(data);

        // If std_dev is very small, use absolute threshold
        if std_dev < 0.1 {
            return (current_value as f32 - mean).abs() > sensitivity;
        }

        let z_score = ((current_value as f32 - mean) / std_dev).abs();
        z_score > sensitivity
    }
}

/// Anomaly detection plugin
pub struct AnomalyDetectorPlugin {
    config: AnomalyDetectorConfig,
    temporal_metrics: RwLock<TemporalMetrics>,
}

impl AnomalyDetectorPlugin {
    pub fn new() -> Self {
        Self {
            config: AnomalyDetectorConfig::default(),
            temporal_metrics: RwLock::new(TemporalMetrics::new(DEFAULT_HISTORY_SIZE)),
        }
    }

    /// Check if an object is in a zone
    fn is_in_zone(bbox: &BoundingBox, zone: &Zone) -> bool {
        let obj_center_x = bbox.x + bbox.width / 2;
        let obj_center_y = bbox.y + bbox.height / 2;

        obj_center_x >= zone.bbox.x
            && obj_center_x <= zone.bbox.x + zone.bbox.width
            && obj_center_y >= zone.bbox.y
            && obj_center_y <= zone.bbox.y + zone.bbox.height
    }

    /// Detect spatial anomalies (objects in restricted zones)
    fn detect_spatial_anomalies(&self, frame: &VideoFrame, detections: &[Detection]) -> Vec<Detection> {
        if !self.config.enable_spatial || self.config.zones.is_empty() {
            return Vec::new();
        }

        let mut anomalies = Vec::new();

        for detection in detections {
            if detection.confidence < self.config.confidence_threshold {
                continue;
            }

            for zone in &self.config.zones {
                if !Self::is_in_zone(&detection.bbox, zone) {
                    continue;
                }

                // Check if object is restricted in this zone
                let is_restricted = if !zone.restricted_classes.is_empty() {
                    zone.restricted_classes.contains(&detection.class)
                } else if !zone.allowed_classes.is_empty() {
                    !zone.allowed_classes.contains(&detection.class)
                } else {
                    false
                };

                if is_restricted {
                    anomalies.push(Detection {
                        class: format!("spatial_anomaly_{}", detection.class),
                        confidence: detection.confidence,
                        bbox: detection.bbox.clone(),
                        metadata: Some(serde_json::json!({
                            "anomaly_type": "spatial",
                            "zone_id": zone.id,
                            "zone_name": zone.name,
                            "original_class": detection.class,
                            "reason": "restricted_object_in_zone",
                            "frame_sequence": frame.sequence,
                        })),
                    });
                }
            }
        }

        anomalies
    }

    /// Detect temporal anomalies (unusual counts or patterns)
    async fn detect_temporal_anomalies(&self, frame: &VideoFrame, detections: &[Detection]) -> Result<Vec<Detection>> {
        if !self.config.enable_temporal {
            return Ok(Vec::new());
        }

        // Count detections by class
        let mut class_counts: HashMap<String, u32> = HashMap::new();
        for detection in detections {
            if detection.confidence >= self.config.confidence_threshold {
                // Filter by tracked classes if specified
                if !self.config.tracked_classes.is_empty()
                    && !self.config.tracked_classes.contains(&detection.class) {
                    continue;
                }

                *class_counts.entry(detection.class.clone()).or_insert(0) += 1;
            }
        }

        let mut metrics = self.temporal_metrics.write().await;

        // Calculate current total
        let current_total: u32 = class_counts.values().sum();

        // Check for anomalies before updating history
        let mut anomalies = Vec::new();

        // Check total count anomaly
        if metrics.is_anomaly(
            current_total,
            &metrics.total_counts,
            self.config.sensitivity,
            self.config.min_samples
        ) {
            anomalies.push(Detection {
                class: "temporal_anomaly_total_count".to_string(),
                confidence: 0.9,
                bbox: BoundingBox { x: 0, y: 0, width: frame.width, height: frame.height },
                metadata: Some(serde_json::json!({
                    "anomaly_type": "temporal",
                    "metric": "total_object_count",
                    "current_value": current_total,
                    "historical_mean": TemporalMetrics::calculate_stats(&metrics.total_counts).0,
                    "historical_std_dev": TemporalMetrics::calculate_stats(&metrics.total_counts).1,
                    "frame_sequence": frame.sequence,
                    "timestamp": frame.timestamp,
                })),
            });
        }

        // Check per-class anomalies
        for (class, &current_count) in &class_counts {
            if let Some(history) = metrics.class_counts.get(class) {
                if metrics.is_anomaly(
                    current_count,
                    history,
                    self.config.sensitivity,
                    self.config.min_samples
                ) {
                    let (mean, std_dev) = TemporalMetrics::calculate_stats(history);
                    anomalies.push(Detection {
                        class: format!("temporal_anomaly_{}", class),
                        confidence: 0.85,
                        bbox: BoundingBox { x: 0, y: 0, width: frame.width, height: frame.height },
                        metadata: Some(serde_json::json!({
                            "anomaly_type": "temporal",
                            "metric": "class_count",
                            "object_class": class,
                            "current_value": current_count,
                            "historical_mean": mean,
                            "historical_std_dev": std_dev,
                            "frame_sequence": frame.sequence,
                            "timestamp": frame.timestamp,
                        })),
                    });
                }
            }
        }

        // Update history
        metrics.add_sample(frame.timestamp, &class_counts, self.config.history_size);

        Ok(anomalies)
    }
}

impl Default for AnomalyDetectorPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AiPlugin for AnomalyDetectorPlugin {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn id(&self) -> &'static str {
        "anomaly_detector"
    }

    fn name(&self) -> &'static str {
        "Anomaly Detector"
    }

    fn description(&self) -> &'static str {
        "Detects temporal and spatial anomalies in video analytics"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn config_schema(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "enable_temporal": {
                    "type": "boolean",
                    "default": true,
                    "description": "Enable temporal anomaly detection"
                },
                "enable_spatial": {
                    "type": "boolean",
                    "default": true,
                    "description": "Enable spatial anomaly detection"
                },
                "sensitivity": {
                    "type": "number",
                    "minimum": 0.5,
                    "maximum": 5.0,
                    "default": 2.0,
                    "description": "Sensitivity for anomaly detection (standard deviations)"
                },
                "history_size": {
                    "type": "integer",
                    "minimum": 10,
                    "maximum": 1000,
                    "default": 100,
                    "description": "Number of historical samples to maintain"
                },
                "min_samples": {
                    "type": "integer",
                    "minimum": 5,
                    "maximum": 100,
                    "default": 10,
                    "description": "Minimum samples before anomaly detection starts"
                },
                "tracked_classes": {
                    "type": "array",
                    "items": {"type": "string"},
                    "default": [],
                    "description": "Object classes to track (empty = all)"
                },
                "zones": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": {"type": "string"},
                            "name": {"type": "string"},
                            "bbox": {
                                "type": "object",
                                "properties": {
                                    "x": {"type": "integer"},
                                    "y": {"type": "integer"},
                                    "width": {"type": "integer"},
                                    "height": {"type": "integer"}
                                }
                            },
                            "allowed_classes": {
                                "type": "array",
                                "items": {"type": "string"}
                            },
                            "restricted_classes": {
                                "type": "array",
                                "items": {"type": "string"}
                            }
                        }
                    },
                    "default": [],
                    "description": "Restricted zones for spatial anomaly detection"
                },
                "confidence_threshold": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "default": 0.5,
                    "description": "Minimum confidence for detections to analyze"
                }
            }
        }))
    }

    fn supported_formats(&self) -> Vec<String> {
        vec!["jpeg".to_string(), "png".to_string(), "raw".to_string()]
    }

    fn requires_gpu(&self) -> bool {
        false
    }

    async fn init(&mut self, config: serde_json::Value) -> Result<()> {
        if !config.is_null() {
            self.config = serde_json::from_value(config)
                .map_err(|e| anyhow!("Failed to parse anomaly detector config: {}", e))?;
        }

        // Reset temporal metrics with new history size
        *self.temporal_metrics.write().await = TemporalMetrics::new(self.config.history_size);

        tracing::info!(
            enable_temporal = self.config.enable_temporal,
            enable_spatial = self.config.enable_spatial,
            sensitivity = self.config.sensitivity,
            zones = self.config.zones.len(),
            "Initialized AnomalyDetectorPlugin"
        );

        Ok(())
    }

    async fn process_frame(&self, frame: &VideoFrame) -> Result<AiResult> {
        let start = std::time::Instant::now();

        // This plugin expects detections from other plugins in the metadata
        // In a real implementation, this would be integrated with a pipeline
        // For now, we'll parse detections from frame metadata or generate mock data
        let detections: Vec<Detection> = if let Ok(data) = serde_json::from_str::<serde_json::Value>(&frame.data) {
            if let Some(dets) = data.get("detections") {
                serde_json::from_value(dets.clone()).unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // Detect spatial anomalies
        let spatial_anomalies = self.detect_spatial_anomalies(frame, &detections);

        // Detect temporal anomalies
        let temporal_anomalies = self.detect_temporal_anomalies(frame, &detections).await?;

        // Combine all anomalies
        let mut all_anomalies = Vec::new();
        all_anomalies.extend(spatial_anomalies);
        all_anomalies.extend(temporal_anomalies);

        let processing_time_ms = start.elapsed().as_millis() as u64;
        let has_anomalies = !all_anomalies.is_empty();

        Ok(AiResult {
            task_id: frame.source_id.clone(),
            timestamp: frame.timestamp,
            plugin_type: self.id().to_string(),
            detections: all_anomalies,
            confidence: if has_anomalies { Some(0.9) } else { Some(0.0) },
            processing_time_ms: Some(processing_time_ms),
            metadata: Some(serde_json::json!({
                "frame_width": frame.width,
                "frame_height": frame.height,
                "frame_sequence": frame.sequence,
                "temporal_enabled": self.config.enable_temporal,
                "spatial_enabled": self.config.enable_spatial,
            })),
        })
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(true)
    }

    async fn shutdown(&mut self) -> Result<()> {
        tracing::info!("Shutting down AnomalyDetectorPlugin");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_anomaly_detector_init() {
        let mut plugin = AnomalyDetectorPlugin::new();
        let config = serde_json::json!({
            "enable_temporal": true,
            "enable_spatial": true,
            "sensitivity": 2.5,
            "history_size": 50,
            "tracked_classes": ["person", "vehicle"],
            "zones": [
                {
                    "id": "zone1",
                    "name": "Restricted Area",
                    "bbox": {"x": 100, "y": 100, "width": 200, "height": 200},
                    "restricted_classes": ["person"]
                }
            ]
        });

        plugin.init(config).await.unwrap();
        assert_eq!(plugin.config.sensitivity, 2.5);
        assert_eq!(plugin.config.history_size, 50);
        assert_eq!(plugin.config.zones.len(), 1);
    }

    #[tokio::test]
    async fn test_spatial_anomaly_detection() {
        let mut plugin = AnomalyDetectorPlugin::new();
        let config = serde_json::json!({
            "enable_spatial": true,
            "zones": [
                {
                    "id": "zone1",
                    "name": "No Entry Zone",
                    "bbox": {"x": 100, "y": 100, "width": 200, "height": 200},
                    "restricted_classes": ["person"]
                }
            ]
        });

        plugin.init(config).await.unwrap();

        let frame = VideoFrame {
            source_id: "test-stream".to_string(),
            timestamp: 1234567890,
            sequence: 1,
            width: 1920,
            height: 1080,
            format: "jpeg".to_string(),
            data: serde_json::json!({
                "detections": [
                    {
                        "class": "person",
                        "confidence": 0.9,
                        "bbox": {"x": 150, "y": 150, "width": 50, "height": 100},
                        "metadata": null
                    }
                ]
            }).to_string(),
        };

        let result = plugin.process_frame(&frame).await.unwrap();
        assert!(!result.detections.is_empty());
        assert_eq!(result.detections[0].class, "spatial_anomaly_person");
    }

    #[tokio::test]
    async fn test_temporal_anomaly_detection() {
        let mut plugin = AnomalyDetectorPlugin::new();
        let config = serde_json::json!({
            "enable_temporal": true,
            "sensitivity": 2.0,
            "min_samples": 5
        });

        plugin.init(config).await.unwrap();

        // Feed normal frames
        for i in 0..10 {
            let frame = VideoFrame {
                source_id: "test-stream".to_string(),
                timestamp: 1234567890 + i * 1000,
                sequence: i,
                width: 1920,
                height: 1080,
                format: "jpeg".to_string(),
                data: serde_json::json!({
                    "detections": [
                        {
                            "class": "person",
                            "confidence": 0.9,
                            "bbox": {"x": 100, "y": 100, "width": 50, "height": 100},
                            "metadata": null
                        }
                    ]
                }).to_string(),
            };

            plugin.process_frame(&frame).await.unwrap();
        }

        // Feed anomalous frame (10 persons instead of 1)
        let anomalous_frame = VideoFrame {
            source_id: "test-stream".to_string(),
            timestamp: 1234567890 + 11000,
            sequence: 11,
            width: 1920,
            height: 1080,
            format: "jpeg".to_string(),
            data: serde_json::json!({
                "detections": (0..10).map(|i| serde_json::json!({
                    "class": "person",
                    "confidence": 0.9,
                    "bbox": {"x": 100 + i * 50, "y": 100, "width": 50, "height": 100},
                    "metadata": null
                })).collect::<Vec<_>>()
            }).to_string(),
        };

        let result = plugin.process_frame(&anomalous_frame).await.unwrap();

        // Should detect temporal anomaly
        let has_temporal_anomaly = result.detections.iter()
            .any(|d| d.class.starts_with("temporal_anomaly"));
        assert!(has_temporal_anomaly, "Should detect temporal anomaly");
    }

    #[test]
    fn test_is_in_zone() {
        let zone = Zone {
            id: "zone1".to_string(),
            name: "Test Zone".to_string(),
            bbox: BoundingBox { x: 100, y: 100, width: 200, height: 200 },
            allowed_classes: Vec::new(),
            restricted_classes: Vec::new(),
        };

        // Object center inside zone
        let bbox_inside = BoundingBox { x: 150, y: 150, width: 50, height: 50 };
        assert!(AnomalyDetectorPlugin::is_in_zone(&bbox_inside, &zone));

        // Object center outside zone
        let bbox_outside = BoundingBox { x: 50, y: 50, width: 30, height: 30 };
        assert!(!AnomalyDetectorPlugin::is_in_zone(&bbox_outside, &zone));
    }

    #[test]
    fn test_temporal_metrics_stats() {
        let mut data = VecDeque::new();
        data.extend([10, 12, 11, 13, 10, 12, 11].iter());

        let (mean, std_dev) = TemporalMetrics::calculate_stats(&data);

        // Mean should be approximately 11.29
        assert!((mean - 11.29).abs() < 0.1);

        // Std dev should be approximately 1.06
        assert!((std_dev - 1.06).abs() < 0.1);
    }
}

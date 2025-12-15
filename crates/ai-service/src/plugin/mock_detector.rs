/// Mock object detection plugin for testing and demonstration purposes
use super::AiPlugin;
use anyhow::Result;
use async_trait::async_trait;
use common::ai_tasks::{AiResult, BoundingBox, Detection, VideoFrame};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockDetectorConfig {
    /// Confidence threshold for detections (0.0 to 1.0)
    #[serde(default = "default_confidence")]
    pub confidence_threshold: f32,

    /// Classes to detect
    #[serde(default)]
    pub classes: Vec<String>,

    /// Simulate processing delay in milliseconds
    #[serde(default)]
    pub simulated_delay_ms: u64,
}

fn default_confidence() -> f32 {
    0.5
}

impl Default for MockDetectorConfig {
    fn default() -> Self {
        Self {
            confidence_threshold: 0.5,
            classes: vec!["person".to_string(), "car".to_string(), "dog".to_string()],
            simulated_delay_ms: 0,
        }
    }
}

/// Mock object detection plugin
pub struct MockDetectorPlugin {
    config: MockDetectorConfig,
}

impl MockDetectorPlugin {
    pub fn new() -> Self {
        Self {
            config: MockDetectorConfig::default(),
        }
    }
}

impl Default for MockDetectorPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AiPlugin for MockDetectorPlugin {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn id(&self) -> &'static str {
        "mock_object_detector"
    }

    fn name(&self) -> &'static str {
        "Mock Object Detector"
    }

    fn description(&self) -> &'static str {
        "A mock object detection plugin for testing and demonstration"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn config_schema(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "confidence_threshold": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "default": 0.5,
                    "description": "Minimum confidence threshold for detections"
                },
                "classes": {
                    "type": "array",
                    "items": {"type": "string"},
                    "default": ["person", "car", "dog"],
                    "description": "List of object classes to detect"
                },
                "simulated_delay_ms": {
                    "type": "integer",
                    "minimum": 0,
                    "default": 0,
                    "description": "Simulated processing delay in milliseconds"
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
            self.config = serde_json::from_value(config)?;
        }
        tracing::info!(
            "Initialized MockDetectorPlugin with confidence threshold: {}",
            self.config.confidence_threshold
        );
        Ok(())
    }

    async fn process_frame(&self, frame: &VideoFrame) -> Result<AiResult> {
        let start = std::time::Instant::now();

        // Simulate processing delay
        if self.config.simulated_delay_ms > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(
                self.config.simulated_delay_ms,
            ))
            .await;
        }

        // Generate mock detections based on frame properties
        let mut detections = Vec::new();

        // Use frame sequence and timestamp to generate deterministic but varied results
        let num_detections = (frame.sequence % 3) + 1; // 1-3 detections per frame

        for i in 0..num_detections {
            let class_idx = ((frame.sequence + i) % self.config.classes.len() as u64) as usize;
            let class = self.config.classes[class_idx].clone();

            // Generate pseudo-random but deterministic bounding box
            let seed = frame.sequence.wrapping_mul(7).wrapping_add(i.wrapping_mul(13));
            let x = (seed % (frame.width / 2) as u64) as u32;
            let y = ((seed / 2) % (frame.height / 2) as u64) as u32;
            let width = ((seed % 200) + 50) as u32;
            let height = ((seed % 200) + 50) as u32;

            // Generate pseudo-random confidence above threshold
            let confidence = self.config.confidence_threshold
                + ((seed % 50) as f32 / 100.0)
                    .min(1.0 - self.config.confidence_threshold);

            detections.push(Detection {
                class,
                confidence,
                bbox: BoundingBox {
                    x,
                    y,
                    width,
                    height,
                },
                metadata: Some(serde_json::json!({
                    "mock": true,
                    "detection_index": i
                })),
            });
        }

        let processing_time_ms = start.elapsed().as_millis() as u64;

        Ok(AiResult {
            task_id: frame.source_id.clone(),
            timestamp: frame.timestamp,
            plugin_type: self.id().to_string(),
            detections,
            confidence: Some(0.85), // Overall confidence
            processing_time_ms: Some(processing_time_ms),
            metadata: Some(serde_json::json!({
                "frame_width": frame.width,
                "frame_height": frame.height,
                "frame_sequence": frame.sequence,
                "mock_mode": true
            })),
        })
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(true)
    }

    async fn shutdown(&mut self) -> Result<()> {
        tracing::info!("Shutting down MockDetectorPlugin");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_detector_init() {
        let mut plugin = MockDetectorPlugin::new();
        let config = serde_json::json!({
            "confidence_threshold": 0.7,
            "classes": ["person", "vehicle"],
            "simulated_delay_ms": 10
        });

        plugin.init(config).await.unwrap();
        assert_eq!(plugin.config.confidence_threshold, 0.7);
        assert_eq!(plugin.config.classes.len(), 2);
    }

    #[tokio::test]
    async fn test_mock_detector_process_frame() {
        let mut plugin = MockDetectorPlugin::new();
        plugin.init(serde_json::Value::Null).await.unwrap();

        let frame = VideoFrame {
            source_id: "test-stream".to_string(),
            timestamp: 1234567890,
            sequence: 42,
            width: 1920,
            height: 1080,
            format: "jpeg".to_string(),
            data: "base64encodeddata".to_string(),
        };

        let result = plugin.process_frame(&frame).await.unwrap();
        assert_eq!(result.plugin_type, "mock_object_detector");
        assert!(!result.detections.is_empty());
        assert!(result.processing_time_ms.is_some());
    }

    #[tokio::test]
    async fn test_mock_detector_deterministic() {
        let mut plugin = MockDetectorPlugin::new();
        plugin.init(serde_json::Value::Null).await.unwrap();

        let frame = VideoFrame {
            source_id: "test-stream".to_string(),
            timestamp: 1234567890,
            sequence: 10,
            width: 1920,
            height: 1080,
            format: "jpeg".to_string(),
            data: "base64encodeddata".to_string(),
        };

        let result1 = plugin.process_frame(&frame).await.unwrap();
        let result2 = plugin.process_frame(&frame).await.unwrap();

        // Same frame should produce same detections
        assert_eq!(result1.detections.len(), result2.detections.len());
        for (d1, d2) in result1.detections.iter().zip(result2.detections.iter()) {
            assert_eq!(d1.class, d2.class);
            assert_eq!(d1.confidence, d2.confidence);
            assert_eq!(d1.bbox.x, d2.bbox.x);
        }
    }
}

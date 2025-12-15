/// Pose estimation plugin using ONNX Runtime
///
/// This plugin detects human poses and keypoints (e.g., shoulders, elbows, wrists, hips, knees, ankles).
/// Compatible with MoveNet, MediaPipe Pose, or similar ONNX models.
use super::AiPlugin;
use anyhow::{Context, Result};
use async_trait::async_trait;
use common::ai_tasks::{AiResult, BoundingBox, Detection, VideoFrame};
use base64::Engine;
use image::DynamicImage;
use ndarray::{Array, IxDyn};
use ort::{
    execution_providers::{CUDAExecutionProvider, CPUExecutionProvider},
    session::{builder::GraphOptimizationLevel, Session},
    value::Value,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// Standard COCO pose keypoint names (17 keypoints)
pub const COCO_KEYPOINTS: [&str; 17] = [
    "nose",
    "left_eye",
    "right_eye",
    "left_ear",
    "right_ear",
    "left_shoulder",
    "right_shoulder",
    "left_elbow",
    "right_elbow",
    "left_wrist",
    "right_wrist",
    "left_hip",
    "right_hip",
    "left_knee",
    "right_knee",
    "left_ankle",
    "right_ankle",
];

/// Pose keypoint with 2D coordinates and confidence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keypoint {
    /// Keypoint name (e.g., "nose", "left_shoulder")
    pub name: String,

    /// X coordinate in image space (pixels)
    pub x: f32,

    /// Y coordinate in image space (pixels)
    pub y: f32,

    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,

    /// Keypoint index in the model output
    pub index: usize,
}

/// Detected pose with all keypoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pose {
    /// All keypoints for this pose
    pub keypoints: Vec<Keypoint>,

    /// Overall pose confidence (average of all keypoint confidences)
    pub confidence: f32,

    /// Bounding box encompassing the entire pose
    pub bbox: BoundingBox,

    /// Person ID (if tracking is enabled)
    pub person_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseEstimationConfig {
    /// Path to the ONNX model file
    pub model_path: String,

    /// Confidence threshold for keypoints (0.0 to 1.0)
    #[serde(default = "default_keypoint_confidence")]
    pub keypoint_confidence_threshold: f32,

    /// Minimum confidence for pose detection (0.0 to 1.0)
    #[serde(default = "default_pose_confidence")]
    pub pose_confidence_threshold: f32,

    /// Model input size (width and height)
    #[serde(default = "default_input_size")]
    pub input_size: u32,

    /// Maximum number of poses to detect per frame
    #[serde(default = "default_max_poses")]
    pub max_poses: usize,

    /// Keypoint names (default: COCO 17 keypoints)
    #[serde(default = "default_keypoint_names")]
    pub keypoint_names: Vec<String>,

    /// Execution provider preference (CPU, CUDA)
    #[serde(default = "default_execution_provider")]
    pub execution_provider: String,

    /// GPU device ID (0, 1, 2, etc.)
    #[serde(default)]
    pub device_id: i32,

    /// Number of intra-operation threads
    #[serde(default = "default_intra_threads")]
    pub intra_threads: usize,

    /// Number of inter-operation threads
    #[serde(default = "default_inter_threads")]
    pub inter_threads: usize,
}

fn default_keypoint_confidence() -> f32 {
    0.3
}

fn default_pose_confidence() -> f32 {
    0.5
}

fn default_input_size() -> u32 {
    256
}

fn default_max_poses() -> usize {
    10
}

fn default_keypoint_names() -> Vec<String> {
    COCO_KEYPOINTS.iter().map(|s| s.to_string()).collect()
}

fn default_execution_provider() -> String {
    "CUDA".to_string()
}

fn default_intra_threads() -> usize {
    4
}

fn default_inter_threads() -> usize {
    1
}

impl Default for PoseEstimationConfig {
    fn default() -> Self {
        Self {
            model_path: "models/movenet.onnx".to_string(),
            keypoint_confidence_threshold: 0.3,
            pose_confidence_threshold: 0.5,
            input_size: 256,
            max_poses: 10,
            keypoint_names: default_keypoint_names(),
            execution_provider: default_execution_provider(),
            device_id: 0,
            intra_threads: 4,
            inter_threads: 1,
        }
    }
}

/// Pose estimation plugin
pub struct PoseEstimationPlugin {
    config: PoseEstimationConfig,
    session: Option<Arc<Mutex<Session>>>,
    execution_provider_used: Arc<Mutex<String>>,
}

impl PoseEstimationPlugin {
    pub fn new() -> Self {
        Self {
            config: PoseEstimationConfig::default(),
            session: None,
            execution_provider_used: Arc::new(Mutex::new("CPU".to_string())),
        }
    }

    /// Preprocess image to model input format
    fn preprocess_image(&self, img: &DynamicImage) -> Result<Array<f32, IxDyn>> {
        let size = self.config.input_size;
        let resized = img.resize_exact(size, size, image::imageops::FilterType::Triangle);
        let rgb_img = resized.to_rgb8();

        // Convert to NCHW format and normalize to [0, 1]
        let mut input = Array::zeros(IxDyn(&[1, 3, size as usize, size as usize]));

        for (x, y, pixel) in rgb_img.enumerate_pixels() {
            let r = pixel[0] as f32 / 255.0;
            let g = pixel[1] as f32 / 255.0;
            let b = pixel[2] as f32 / 255.0;

            input[[0, 0, y as usize, x as usize]] = r;
            input[[0, 1, y as usize, x as usize]] = g;
            input[[0, 2, y as usize, x as usize]] = b;
        }

        Ok(input)
    }

    /// Calculate bounding box from keypoints
    fn calculate_bbox(&self, keypoints: &[Keypoint], _scale_x: f32, _scale_y: f32) -> BoundingBox {
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for kp in keypoints {
            if kp.confidence >= self.config.keypoint_confidence_threshold {
                min_x = min_x.min(kp.x);
                min_y = min_y.min(kp.y);
                max_x = max_x.max(kp.x);
                max_y = max_y.max(kp.y);
            }
        }

        // Add padding (10% on each side)
        let padding_x = (max_x - min_x) * 0.1;
        let padding_y = (max_y - min_y) * 0.1;

        min_x = (min_x - padding_x).max(0.0);
        min_y = (min_y - padding_y).max(0.0);
        max_x = max_x + padding_x;
        max_y = max_y + padding_y;

        BoundingBox {
            x: min_x as u32,
            y: min_y as u32,
            width: ((max_x - min_x) as u32).max(1),
            height: ((max_y - min_y) as u32).max(1),
        }
    }

    /// Post-process model output to extract poses
    ///
    /// Expected output format: [batch, num_keypoints, 3] where 3 = (y, x, confidence)
    /// or [batch, num_poses, num_keypoints, 3]
    fn postprocess_output(
        &self,
        output: Array<f32, IxDyn>,
        original_width: u32,
        original_height: u32,
    ) -> Result<Vec<Pose>> {
        let scale_x = original_width as f32;
        let scale_y = original_height as f32;

        let mut poses = Vec::new();
        let shape = output.shape();

        // Handle different output shapes
        match shape.len() {
            // Single pose: [1, num_keypoints, 3]
            3 if shape[0] == 1 => {
                let num_keypoints = shape[1];
                let mut keypoints = Vec::new();

                for kp_idx in 0..num_keypoints {
                    let y = output[[0, kp_idx, 0]]; // Normalized [0, 1]
                    let x = output[[0, kp_idx, 1]]; // Normalized [0, 1]
                    let confidence = output[[0, kp_idx, 2]];

                    let name = if kp_idx < self.config.keypoint_names.len() {
                        self.config.keypoint_names[kp_idx].clone()
                    } else {
                        format!("keypoint_{}", kp_idx)
                    };

                    keypoints.push(Keypoint {
                        name,
                        x: x * scale_x,
                        y: y * scale_y,
                        confidence,
                        index: kp_idx,
                    });
                }

                // Calculate average confidence for visible keypoints
                let visible_keypoints: Vec<&Keypoint> = keypoints
                    .iter()
                    .filter(|kp| kp.confidence >= self.config.keypoint_confidence_threshold)
                    .collect();

                if !visible_keypoints.is_empty() {
                    let avg_confidence = visible_keypoints
                        .iter()
                        .map(|kp| kp.confidence)
                        .sum::<f32>()
                        / visible_keypoints.len() as f32;

                    if avg_confidence >= self.config.pose_confidence_threshold {
                        let bbox = self.calculate_bbox(&keypoints, scale_x, scale_y);
                        poses.push(Pose {
                            keypoints,
                            confidence: avg_confidence,
                            bbox,
                            person_id: None,
                        });
                    }
                }
            }
            // Multiple poses: [1, num_poses, num_keypoints, 3]
            4 if shape[0] == 1 => {
                let num_poses = shape[1];
                let num_keypoints = shape[2];

                for pose_idx in 0..num_poses.min(self.config.max_poses) {
                    let mut keypoints = Vec::new();

                    for kp_idx in 0..num_keypoints {
                        let y = output[[0, pose_idx, kp_idx, 0]];
                        let x = output[[0, pose_idx, kp_idx, 1]];
                        let confidence = output[[0, pose_idx, kp_idx, 2]];

                        let name = if kp_idx < self.config.keypoint_names.len() {
                            self.config.keypoint_names[kp_idx].clone()
                        } else {
                            format!("keypoint_{}", kp_idx)
                        };

                        keypoints.push(Keypoint {
                            name,
                            x: x * scale_x,
                            y: y * scale_y,
                            confidence,
                            index: kp_idx,
                        });
                    }

                    let visible_keypoints: Vec<&Keypoint> = keypoints
                        .iter()
                        .filter(|kp| kp.confidence >= self.config.keypoint_confidence_threshold)
                        .collect();

                    if !visible_keypoints.is_empty() {
                        let avg_confidence = visible_keypoints
                            .iter()
                            .map(|kp| kp.confidence)
                            .sum::<f32>()
                            / visible_keypoints.len() as f32;

                        if avg_confidence >= self.config.pose_confidence_threshold {
                            let bbox = self.calculate_bbox(&keypoints, scale_x, scale_y);
                            poses.push(Pose {
                                keypoints,
                                confidence: avg_confidence,
                                bbox,
                                person_id: Some(pose_idx as u32),
                            });
                        }
                    }
                }
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported output shape: {:?}. Expected [1, num_keypoints, 3] or [1, num_poses, num_keypoints, 3]",
                    shape
                ));
            }
        }

        Ok(poses)
    }

    /// Convert poses to detections for the AI result
    fn poses_to_detections(&self, poses: Vec<Pose>) -> Vec<Detection> {
        poses
            .into_iter()
            .map(|pose| {
                let class = if let Some(id) = pose.person_id {
                    format!("person_{}", id)
                } else {
                    "person".to_string()
                };

                Detection {
                    class,
                    confidence: pose.confidence,
                    bbox: pose.bbox.clone(),
                    metadata: Some(serde_json::json!({
                        "pose": {
                            "keypoints": pose.keypoints,
                            "num_visible_keypoints": pose.keypoints.iter()
                                .filter(|kp| kp.confidence >= self.config.keypoint_confidence_threshold)
                                .count(),
                            "person_id": pose.person_id,
                        }
                    })),
                }
            })
            .collect()
    }
}

impl Default for PoseEstimationPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AiPlugin for PoseEstimationPlugin {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn id(&self) -> &'static str {
        "pose_estimation"
    }

    fn name(&self) -> &'static str {
        "Pose Estimation"
    }

    fn description(&self) -> &'static str {
        "Human pose estimation with keypoint detection (COCO 17 keypoints)"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn config_schema(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "model_path": {
                    "type": "string",
                    "default": "models/movenet.onnx",
                    "description": "Path to the pose estimation ONNX model file"
                },
                "keypoint_confidence_threshold": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "default": 0.3,
                    "description": "Minimum confidence threshold for individual keypoints"
                },
                "pose_confidence_threshold": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "default": 0.5,
                    "description": "Minimum average confidence for pose detection"
                },
                "max_poses": {
                    "type": "integer",
                    "minimum": 1,
                    "default": 10,
                    "description": "Maximum number of poses to detect per frame"
                },
                "input_size": {
                    "type": "integer",
                    "default": 256,
                    "description": "Model input size (width and height)"
                },
                "keypoint_names": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "List of keypoint names (default: COCO 17 keypoints)"
                },
                "execution_provider": {
                    "type": "string",
                    "enum": ["CPU", "CUDA"],
                    "default": "CUDA",
                    "description": "Execution provider (CPU, CUDA)"
                },
                "device_id": {
                    "type": "integer",
                    "minimum": 0,
                    "default": 0,
                    "description": "GPU device ID (0, 1, 2, etc.)"
                },
                "intra_threads": {
                    "type": "integer",
                    "minimum": 1,
                    "default": 4,
                    "description": "Number of intra-operation threads"
                },
                "inter_threads": {
                    "type": "integer",
                    "minimum": 1,
                    "default": 1,
                    "description": "Number of inter-operation threads"
                }
            },
            "required": ["model_path"]
        }))
    }

    fn supported_formats(&self) -> Vec<String> {
        vec!["jpeg".to_string(), "png".to_string()]
    }

    fn requires_gpu(&self) -> bool {
        false // Can run on CPU, GPU support is optional
    }

    async fn init(&mut self, config: serde_json::Value) -> Result<()> {
        if !config.is_null() {
            self.config = serde_json::from_value(config)?;
        }

        // Read configuration from environment variables if set
        if let Ok(model_path) = std::env::var("POSE_MODEL_PATH") {
            self.config.model_path = model_path;
        }
        if let Ok(provider) = std::env::var("POSE_EXECUTION_PROVIDER") {
            self.config.execution_provider = provider;
        }
        if let Ok(device_id) = std::env::var("POSE_DEVICE_ID") {
            if let Ok(id) = device_id.parse::<i32>() {
                self.config.device_id = id;
            }
        }

        // Configure execution providers with fallback
        let provider_preference = self.config.execution_provider.to_uppercase();
        let (session, actual_provider) = match provider_preference.as_str() {
            "CUDA" => {
                tracing::info!(
                    "Attempting to use CUDA execution provider for pose estimation (device: {})",
                    self.config.device_id
                );
                let result = Session::builder()?
                    .with_optimization_level(GraphOptimizationLevel::Level3)?
                    .with_intra_threads(self.config.intra_threads)?
                    .with_inter_threads(self.config.inter_threads)?
                    .with_execution_providers([
                        CUDAExecutionProvider::default()
                            .with_device_id(self.config.device_id)
                            .build(),
                        CPUExecutionProvider::default().build(),
                    ])?
                    .commit_from_file(&self.config.model_path);

                match result {
                    Ok(session) => {
                        tracing::info!("Successfully configured CUDA execution provider for pose estimation");
                        (session, "CUDA".to_string())
                    }
                    Err(e) => {
                        tracing::warn!("Failed with CUDA, using CPU for pose estimation: {}", e);
                        let cpu_session = Session::builder()?
                            .with_optimization_level(GraphOptimizationLevel::Level3)?
                            .with_intra_threads(self.config.intra_threads)?
                            .with_inter_threads(self.config.inter_threads)?
                            .commit_from_file(&self.config.model_path)?;
                        (cpu_session, "CPU".to_string())
                    }
                }
            }
            _ => {
                tracing::info!("Using CPU execution provider for pose estimation");
                let session = Session::builder()?
                    .with_optimization_level(GraphOptimizationLevel::Level3)?
                    .with_intra_threads(self.config.intra_threads)?
                    .with_inter_threads(self.config.inter_threads)?
                    .commit_from_file(&self.config.model_path)?;
                (session, "CPU".to_string())
            }
        };

        self.session = Some(Arc::new(Mutex::new(session)));
        *self.execution_provider_used.lock().unwrap() = actual_provider.clone();

        tracing::info!(
            "Initialized pose estimation plugin - model: {}, provider: {}, device: {}, input_size: {}",
            self.config.model_path,
            actual_provider,
            self.config.device_id,
            self.config.input_size
        );

        Ok(())
    }

    async fn process_frame(&self, frame: &VideoFrame) -> Result<AiResult> {
        let start = std::time::Instant::now();

        let session_lock = self
            .session
            .as_ref()
            .context("Model not initialized - call init() first")?;

        // Decode base64 image
        let image_data = base64::prelude::BASE64_STANDARD
            .decode(&frame.data)
            .context("Failed to decode base64 image")?;

        let img = image::load_from_memory(&image_data).context("Failed to load image")?;

        let original_width = img.width();
        let original_height = img.height();

        // Preprocess image
        let input_array = self.preprocess_image(&img)?;

        // Convert ndarray to ort Value
        let input_tensor = Value::from_array(input_array)?;

        // Run inference
        let inference_start = std::time::Instant::now();
        let mut session = session_lock
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock session: {}", e))?;
        let outputs = session.run(ort::inputs![input_tensor])?;
        let inference_time = inference_start.elapsed();

        // Get output tensor - try common output names
        let output_value = outputs
            .get("output")
            .or_else(|| outputs.get("output0"))
            .context("No output tensor found")?;

        let (shape, data) = output_value.try_extract_tensor::<f32>()?;

        // Convert shape from i64 to usize
        let shape_usize: Vec<usize> = shape.as_ref().iter().map(|&x| x as usize).collect();

        // Convert to ndarray
        let output = Array::from_shape_vec(IxDyn(&shape_usize), data.to_vec())?;

        // Post-process results
        let poses = self.postprocess_output(output, original_width, original_height)?;

        // Convert to detections
        let detections = self.poses_to_detections(poses.clone());

        let processing_time_ms = start.elapsed().as_millis() as u64;

        // Calculate average confidence
        let avg_confidence = if !detections.is_empty() {
            detections.iter().map(|d| d.confidence).sum::<f32>() / detections.len() as f32
        } else {
            0.0
        };

        let execution_provider = self.execution_provider_used.lock().unwrap().clone();

        // Track metrics
        telemetry::metrics::AI_SERVICE_GPU_INFERENCE
            .with_label_values(&[self.id(), &execution_provider])
            .inc();

        telemetry::metrics::AI_SERVICE_INFERENCE_TIME
            .with_label_values(&[self.id(), &execution_provider])
            .observe(inference_time.as_secs_f64());

        Ok(AiResult {
            task_id: frame.source_id.clone(),
            timestamp: frame.timestamp,
            plugin_type: self.id().to_string(),
            detections,
            confidence: Some(avg_confidence),
            processing_time_ms: Some(processing_time_ms),
            metadata: Some(serde_json::json!({
                "frame_width": original_width,
                "frame_height": original_height,
                "frame_sequence": frame.sequence,
                "model_path": self.config.model_path,
                "input_size": self.config.input_size,
                "execution_provider": execution_provider,
                "device_id": self.config.device_id,
                "inference_time_ms": inference_time.as_millis() as u64,
                "num_poses_detected": poses.len(),
                "num_keypoints_per_pose": self.config.keypoint_names.len(),
            })),
        })
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(self.session.is_some())
    }

    async fn shutdown(&mut self) -> Result<()> {
        tracing::info!("Shutting down pose estimation plugin");
        self.session = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = PoseEstimationConfig::default();
        assert_eq!(config.keypoint_confidence_threshold, 0.3);
        assert_eq!(config.pose_confidence_threshold, 0.5);
        assert_eq!(config.max_poses, 10);
        assert_eq!(config.input_size, 256);
        assert_eq!(config.keypoint_names.len(), 17);
    }

    #[test]
    fn test_keypoint_names() {
        let config = PoseEstimationConfig::default();
        assert_eq!(config.keypoint_names[0], "nose");
        assert_eq!(config.keypoint_names[5], "left_shoulder");
        assert_eq!(config.keypoint_names[16], "right_ankle");
    }

    #[test]
    fn test_calculate_bbox() {
        let plugin = PoseEstimationPlugin::new();

        let keypoints = vec![
            Keypoint {
                name: "nose".to_string(),
                x: 100.0,
                y: 50.0,
                confidence: 0.9,
                index: 0,
            },
            Keypoint {
                name: "left_shoulder".to_string(),
                x: 80.0,
                y: 100.0,
                confidence: 0.8,
                index: 5,
            },
            Keypoint {
                name: "right_shoulder".to_string(),
                x: 120.0,
                y: 100.0,
                confidence: 0.85,
                index: 6,
            },
        ];

        let bbox = plugin.calculate_bbox(&keypoints, 1.0, 1.0);

        // Should encompass all keypoints with padding
        assert!(bbox.x <= 80);
        assert!(bbox.y <= 50);
        assert!(bbox.width > 40);
        assert!(bbox.height > 50);
    }

    #[test]
    fn test_pose_serialization() {
        let pose = Pose {
            keypoints: vec![
                Keypoint {
                    name: "nose".to_string(),
                    x: 100.0,
                    y: 50.0,
                    confidence: 0.9,
                    index: 0,
                },
            ],
            confidence: 0.9,
            bbox: BoundingBox {
                x: 90,
                y: 40,
                width: 20,
                height: 20,
            },
            person_id: Some(0),
        };

        let json = serde_json::to_string(&pose).unwrap();
        let deserialized: Pose = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.keypoints.len(), 1);
        assert_eq!(deserialized.confidence, 0.9);
    }
}

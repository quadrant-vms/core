/// Action Recognition plugin using temporal feature analysis
///
/// This plugin analyzes sequences of video frames to detect human actions.
/// Supported actions: walking, running, sitting, standing, waving, jumping, etc.
use super::AiPlugin;
use anyhow::{Context, Result};
use async_trait::async_trait;
use base64::Engine;
use common::ai_tasks::{AiResult, BoundingBox, Detection, VideoFrame};
use image::DynamicImage;
use ndarray::{Array, Array4, IxDyn};
use ort::{
    execution_providers::{CPUExecutionProvider, CUDAExecutionProvider, TensorRTExecutionProvider},
    session::{builder::GraphOptimizationLevel, Session},
    value::Value,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRecognitionConfig {
    /// Path to the ONNX model file
    pub model_path: String,

    /// Confidence threshold for action detections (0.0 to 1.0)
    #[serde(default = "default_confidence")]
    pub confidence_threshold: f32,

    /// Number of frames to buffer for temporal analysis
    #[serde(default = "default_temporal_window")]
    pub temporal_window: usize,

    /// Model input size (width and height)
    #[serde(default = "default_input_size")]
    pub input_size: u32,

    /// Action class names
    #[serde(default = "default_action_classes")]
    pub action_classes: Vec<String>,

    /// Execution provider preference (CPU, CUDA, TensorRT)
    #[serde(default = "default_execution_provider")]
    pub execution_provider: String,

    /// GPU device ID (0, 1, 2, etc.)
    #[serde(default = "default_device_id")]
    pub device_id: i32,

    /// Number of intra-operation threads
    #[serde(default = "default_intra_threads")]
    pub intra_threads: usize,

    /// Number of inter-operation threads
    #[serde(default = "default_inter_threads")]
    pub inter_threads: usize,

    /// GPU memory limit in bytes (0 = unlimited)
    #[serde(default = "default_gpu_mem_limit")]
    pub gpu_mem_limit: usize,
}

fn default_confidence() -> f32 {
    0.6
}

fn default_temporal_window() -> usize {
    16 // Standard temporal window for action recognition
}

fn default_input_size() -> u32 {
    224
}

fn default_action_classes() -> Vec<String> {
    vec![
        "walking",
        "running",
        "sitting",
        "standing",
        "waving",
        "jumping",
        "clapping",
        "pointing",
        "talking",
        "phone_call",
        "drinking",
        "eating",
        "reading",
        "writing",
        "pushing",
        "pulling",
        "carrying",
        "throwing",
        "catching",
        "falling",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn default_execution_provider() -> String {
    "CUDA".to_string()
}

fn default_device_id() -> i32 {
    0
}

fn default_intra_threads() -> usize {
    4
}

fn default_inter_threads() -> usize {
    4
}

fn default_gpu_mem_limit() -> usize {
    0 // Unlimited
}

impl Default for ActionRecognitionConfig {
    fn default() -> Self {
        Self {
            model_path: "models/action_recognition.onnx".to_string(),
            confidence_threshold: default_confidence(),
            temporal_window: default_temporal_window(),
            input_size: default_input_size(),
            action_classes: default_action_classes(),
            execution_provider: default_execution_provider(),
            device_id: default_device_id(),
            intra_threads: default_intra_threads(),
            inter_threads: default_inter_threads(),
            gpu_mem_limit: default_gpu_mem_limit(),
        }
    }
}

/// Temporal frame buffer for action recognition
struct FrameBuffer {
    frames: VecDeque<DynamicImage>,
    capacity: usize,
}

impl FrameBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            frames: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    fn push(&mut self, frame: DynamicImage) {
        if self.frames.len() >= self.capacity {
            self.frames.pop_front();
        }
        self.frames.push_back(frame);
    }

    fn is_ready(&self) -> bool {
        self.frames.len() >= self.capacity
    }

    fn get_frames(&self) -> &VecDeque<DynamicImage> {
        &self.frames
    }
}

pub struct ActionRecognitionPlugin {
    config: ActionRecognitionConfig,
    session: Option<Arc<Mutex<Session>>>,
    frame_buffer: Arc<Mutex<FrameBuffer>>,
    initialized: bool,
}

impl ActionRecognitionPlugin {
    pub fn new() -> Self {
        Self {
            config: ActionRecognitionConfig::default(),
            session: None,
            frame_buffer: Arc::new(Mutex::new(FrameBuffer::new(default_temporal_window()))),
            initialized: false,
        }
    }

    /// Preprocess a single frame for the model
    fn preprocess_frame(&self, img: &DynamicImage) -> Result<Array4<f32>> {
        let input_size = self.config.input_size;
        let resized = img.resize_exact(
            input_size,
            input_size,
            image::imageops::FilterType::Triangle,
        );
        let rgb_img = resized.to_rgb8();

        let mut array = Array4::<f32>::zeros((1, 3, input_size as usize, input_size as usize));

        // Normalize to [0, 1] and apply ImageNet normalization
        let mean = [0.485, 0.456, 0.406];
        let std = [0.229, 0.224, 0.225];

        for (x, y, pixel) in rgb_img.enumerate_pixels() {
            let r = (pixel[0] as f32 / 255.0 - mean[0]) / std[0];
            let g = (pixel[1] as f32 / 255.0 - mean[1]) / std[1];
            let b = (pixel[2] as f32 / 255.0 - mean[2]) / std[2];

            array[[0, 0, y as usize, x as usize]] = r;
            array[[0, 1, y as usize, x as usize]] = g;
            array[[0, 2, y as usize, x as usize]] = b;
        }

        Ok(array)
    }

    /// Preprocess a sequence of frames for temporal analysis
    fn preprocess_sequence(&self, frames: &VecDeque<DynamicImage>) -> Result<Array<f32, IxDyn>> {
        let input_size = self.config.input_size as usize;
        let num_frames = frames.len();

        // Shape: [batch=1, channels=3, temporal=num_frames, height=input_size, width=input_size]
        let mut array =
            Array::zeros(IxDyn(&[1, 3, num_frames, input_size, input_size]));

        let mean = [0.485, 0.456, 0.406];
        let std = [0.229, 0.224, 0.225];

        for (t, frame) in frames.iter().enumerate() {
            let resized = frame.resize_exact(
                self.config.input_size,
                self.config.input_size,
                image::imageops::FilterType::Triangle,
            );
            let rgb_img = resized.to_rgb8();

            for (x, y, pixel) in rgb_img.enumerate_pixels() {
                let r = (pixel[0] as f32 / 255.0 - mean[0]) / std[0];
                let g = (pixel[1] as f32 / 255.0 - mean[1]) / std[1];
                let b = (pixel[2] as f32 / 255.0 - mean[2]) / std[2];

                array[[0, 0, t, y as usize, x as usize]] = r;
                array[[0, 1, t, y as usize, x as usize]] = g;
                array[[0, 2, t, y as usize, x as usize]] = b;
            }
        }

        Ok(array)
    }

    /// Run inference on the temporal sequence
    fn run_inference(&self, input: Array<f32, IxDyn>) -> Result<Vec<Detection>> {
        let session_arc = self
            .session
            .as_ref()
            .context("Model session not initialized")?;

        let mut session = session_arc.lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock session: {}", e))?;

        // Create input tensor value
        let input_value = Value::from_array(input)?;

        // Run inference
        let outputs = session
            .run(ort::inputs![input_value])
            .context("Failed to run inference")?;

        // Get output tensor
        let output_tensor = outputs[0].try_extract_tensor::<f32>()?;
        let output = output_tensor.1;

        // Apply softmax to get probabilities
        let logits = output.to_vec();
        let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_logits: Vec<f32> = logits.iter().map(|&x| (x - max_logit).exp()).collect();
        let sum_exp: f32 = exp_logits.iter().sum();
        let probabilities: Vec<f32> = exp_logits.iter().map(|&x| x / sum_exp).collect();

        // Filter detections by confidence threshold
        let mut detections = Vec::new();
        for (class_idx, &prob) in probabilities.iter().enumerate() {
            if prob >= self.config.confidence_threshold
                && class_idx < self.config.action_classes.len()
            {
                let class_name = self.config.action_classes[class_idx].clone();

                // Action recognition doesn't have bounding boxes,
                // so we use full frame as bbox
                let bbox = BoundingBox {
                    x: 0,
                    y: 0,
                    width: self.config.input_size,
                    height: self.config.input_size,
                };

                let metadata = serde_json::json!({
                    "action_class": class_name,
                    "probability": prob,
                    "all_probabilities": probabilities
                        .iter()
                        .enumerate()
                        .map(|(i, &p)| (self.config.action_classes.get(i).map(|s: &String| s.as_str()).unwrap_or("unknown"), p))
                        .collect::<Vec<_>>(),
                });

                detections.push(Detection {
                    class: class_name,
                    confidence: prob,
                    bbox,
                    metadata: Some(metadata),
                });
            }
        }

        // Sort by confidence (descending)
        detections.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

        Ok(detections)
    }
}

impl Default for ActionRecognitionPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AiPlugin for ActionRecognitionPlugin {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn id(&self) -> &'static str {
        "action_recognition"
    }

    fn name(&self) -> &'static str {
        "Action Recognition"
    }

    fn description(&self) -> &'static str {
        "Temporal action recognition plugin that detects human actions from video sequences (walking, running, sitting, etc.)"
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
                    "description": "Path to the ONNX model file"
                },
                "confidence_threshold": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "default": 0.6,
                    "description": "Confidence threshold for action detections"
                },
                "temporal_window": {
                    "type": "integer",
                    "minimum": 8,
                    "maximum": 64,
                    "default": 16,
                    "description": "Number of frames to buffer for temporal analysis"
                },
                "input_size": {
                    "type": "integer",
                    "default": 224,
                    "description": "Model input size (width and height)"
                },
                "execution_provider": {
                    "type": "string",
                    "enum": ["CPU", "CUDA", "TensorRT"],
                    "default": "CUDA",
                    "description": "Execution provider (CPU, CUDA, or TensorRT)"
                },
                "device_id": {
                    "type": "integer",
                    "default": 0,
                    "description": "GPU device ID"
                }
            },
            "required": ["model_path"]
        }))
    }

    fn supported_formats(&self) -> Vec<String> {
        vec!["jpeg".to_string(), "png".to_string(), "rgb24".to_string()]
    }

    fn requires_gpu(&self) -> bool {
        self.config.execution_provider == "CUDA" || self.config.execution_provider == "TensorRT"
    }

    async fn init(&mut self, config: serde_json::Value) -> Result<()> {
        // Parse configuration
        self.config = serde_json::from_value(config).context("Invalid configuration")?;

        // Update frame buffer capacity
        let temporal_window = self.config.temporal_window;
        self.frame_buffer = Arc::new(Mutex::new(FrameBuffer::new(temporal_window)));

        tracing::info!(
            "Initializing ActionRecognition plugin with model: {}",
            self.config.model_path
        );

        // Initialize ONNX Runtime session
        let mut session_builder = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(self.config.intra_threads)?
            .with_inter_threads(self.config.inter_threads)?;

        // Configure execution provider
        match self.config.execution_provider.as_str() {
            "CUDA" => {
                tracing::info!("Using CUDA execution provider (GPU: {})", self.config.device_id);
                session_builder = session_builder.with_execution_providers([
                    CUDAExecutionProvider::default()
                        .with_device_id(self.config.device_id)
                        .with_memory_limit(self.config.gpu_mem_limit)
                        .build(),
                    CPUExecutionProvider::default().build()
                ])?;
            }
            "TensorRT" => {
                tracing::info!(
                    "Using TensorRT execution provider (GPU: {})",
                    self.config.device_id
                );
                session_builder = session_builder.with_execution_providers([
                    TensorRTExecutionProvider::default()
                        .with_device_id(self.config.device_id)
                        .build(),
                    CPUExecutionProvider::default().build()
                ])?;
            }
            "CPU" => {
                tracing::info!("Using CPU execution provider");
                session_builder =
                    session_builder.with_execution_providers([CPUExecutionProvider::default().build()])?;
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported execution provider: {}",
                    self.config.execution_provider
                ));
            }
        }

        // Load the model
        let session = session_builder
            .commit_from_file(&self.config.model_path)
            .context("Failed to load ONNX model")?;

        self.session = Some(Arc::new(Mutex::new(session)));
        self.initialized = true;

        tracing::info!("ActionRecognition plugin initialized successfully");
        Ok(())
    }

    async fn process_frame(&self, frame: &VideoFrame) -> Result<AiResult> {
        if !self.initialized {
            return Err(anyhow::anyhow!("Plugin not initialized"));
        }

        let start_time = std::time::Instant::now();

        // Decode frame
        let img_data = base64::engine::general_purpose::STANDARD
            .decode(&frame.data)
            .context("Failed to decode base64 frame data")?;
        let img = image::load_from_memory(&img_data).context("Failed to load image")?;

        // Add frame to buffer
        {
            let mut buffer = self.frame_buffer.lock()
                .map_err(|e| anyhow::anyhow!("Failed to lock frame buffer: {}", e))?;
            buffer.push(img);
        }

        // Check if we have enough frames for inference
        let buffer = self.frame_buffer.lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock frame buffer: {}", e))?;

        let detections = if buffer.is_ready() {
            // Preprocess sequence
            let input = self.preprocess_sequence(buffer.get_frames())?;

            // Run inference
            self.run_inference(input)?
        } else {
            // Not enough frames yet, return empty result
            Vec::new()
        };

        let processing_time = start_time.elapsed().as_millis() as u64;

        Ok(AiResult {
            task_id: frame.source_id.clone(),
            timestamp: frame.timestamp,
            plugin_type: self.id().to_string(),
            detections,
            confidence: None,
            processing_time_ms: Some(processing_time),
            metadata: Some(serde_json::json!({
                "temporal_window": self.config.temporal_window,
                "buffer_ready": buffer.is_ready(),
                "buffer_size": buffer.frames.len(),
            })),
        })
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(self.initialized && self.session.is_some())
    }

    async fn shutdown(&mut self) -> Result<()> {
        tracing::info!("Shutting down ActionRecognition plugin");
        self.session = None;
        self.initialized = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_action_recognition_plugin_metadata() {
        let plugin = ActionRecognitionPlugin::new();
        assert_eq!(plugin.id(), "action_recognition");
        assert_eq!(plugin.name(), "Action Recognition");
        assert!(!plugin.id().is_empty());
        assert!(!plugin.description().is_empty());
    }

    #[tokio::test]
    async fn test_config_schema() {
        let plugin = ActionRecognitionPlugin::new();
        let schema = plugin.config_schema();
        assert!(schema.is_some());

        let schema_obj = schema.unwrap();
        assert!(schema_obj.get("type").is_some());
        assert!(schema_obj.get("properties").is_some());
    }

    #[test]
    fn test_frame_buffer() {
        let mut buffer = FrameBuffer::new(3);
        assert!(!buffer.is_ready());

        // Create dummy images
        let img1 = DynamicImage::new_rgb8(224, 224);
        let img2 = DynamicImage::new_rgb8(224, 224);
        let img3 = DynamicImage::new_rgb8(224, 224);

        buffer.push(img1);
        assert!(!buffer.is_ready());

        buffer.push(img2);
        assert!(!buffer.is_ready());

        buffer.push(img3);
        assert!(buffer.is_ready());

        // Test overflow
        let img4 = DynamicImage::new_rgb8(224, 224);
        buffer.push(img4);
        assert_eq!(buffer.frames.len(), 3);
        assert!(buffer.is_ready());
    }
}

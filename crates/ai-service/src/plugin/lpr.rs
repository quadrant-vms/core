/// License Plate Recognition (LPR) plugin using ONNX Runtime
///
/// This plugin performs two-stage license plate recognition:
/// 1. Detection stage: Locates license plates in the image using YOLOv8
/// 2. OCR stage: Reads the text from detected plates using CRNN/LSTM model
use super::AiPlugin;
use anyhow::{Context, Result};
use async_trait::async_trait;
use common::ai_tasks::{AiResult, BoundingBox, Detection, VideoFrame};
use base64::Engine;
use image::DynamicImage;
use ndarray::{Array, IxDyn};
use ort::{
    execution_providers::{CUDAExecutionProvider, TensorRTExecutionProvider, CPUExecutionProvider},
    session::{builder::GraphOptimizationLevel, Session},
    value::Value,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LprConfig {
    /// Path to the license plate detection ONNX model file
    pub detection_model_path: String,

    /// Path to the OCR ONNX model file (optional - if not provided, only detection is performed)
    pub ocr_model_path: Option<String>,

    /// Confidence threshold for plate detections (0.0 to 1.0)
    #[serde(default = "default_confidence")]
    pub confidence_threshold: f32,

    /// IoU (Intersection over Union) threshold for NMS
    #[serde(default = "default_iou_threshold")]
    pub iou_threshold: f32,

    /// Maximum number of plates to detect per frame
    #[serde(default = "default_max_detections")]
    pub max_detections: usize,

    /// Detection model input size (width and height)
    #[serde(default = "default_detection_input_size")]
    pub detection_input_size: u32,

    /// OCR model input width
    #[serde(default = "default_ocr_input_width")]
    pub ocr_input_width: u32,

    /// OCR model input height
    #[serde(default = "default_ocr_input_height")]
    pub ocr_input_height: u32,

    /// Character vocabulary for OCR (default: digits + uppercase letters)
    #[serde(default = "default_char_vocab")]
    pub char_vocab: String,

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
}

fn default_confidence() -> f32 {
    0.6
}

fn default_iou_threshold() -> f32 {
    0.4
}

fn default_max_detections() -> usize {
    10
}

fn default_detection_input_size() -> u32 {
    640
}

fn default_ocr_input_width() -> u32 {
    200
}

fn default_ocr_input_height() -> u32 {
    64
}

fn default_char_vocab() -> String {
    // Common characters found on license plates (digits + uppercase letters)
    // CTC blank character is at index 0, so vocab starts at index 1
    "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ-".to_string()
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
    1
}

impl Default for LprConfig {
    fn default() -> Self {
        Self {
            detection_model_path: "models/lpr_detector.onnx".to_string(),
            ocr_model_path: Some("models/lpr_ocr.onnx".to_string()),
            confidence_threshold: default_confidence(),
            iou_threshold: default_iou_threshold(),
            max_detections: default_max_detections(),
            detection_input_size: default_detection_input_size(),
            ocr_input_width: default_ocr_input_width(),
            ocr_input_height: default_ocr_input_height(),
            char_vocab: default_char_vocab(),
            execution_provider: default_execution_provider(),
            device_id: default_device_id(),
            intra_threads: default_intra_threads(),
            inter_threads: default_inter_threads(),
        }
    }
}

/// License Plate Recognition plugin
pub struct LprPlugin {
    config: LprConfig,
    detection_session: Option<Arc<Mutex<Session>>>,
    ocr_session: Option<Arc<Mutex<Session>>>,
    execution_provider_used: Arc<Mutex<String>>,
}

impl LprPlugin {
    pub fn new() -> Self {
        Self {
            config: LprConfig::default(),
            detection_session: None,
            ocr_session: None,
            execution_provider_used: Arc::new(Mutex::new("CPU".to_string())),
        }
    }

    /// Preprocess image for detection model
    fn preprocess_for_detection(&self, img: &DynamicImage) -> Result<Array<f32, IxDyn>> {
        let size = self.config.detection_input_size;
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

    /// Preprocess cropped plate image for OCR model
    fn preprocess_for_ocr(&self, img: &DynamicImage) -> Result<Array<f32, IxDyn>> {
        let width = self.config.ocr_input_width;
        let height = self.config.ocr_input_height;
        let resized = img.resize_exact(width, height, image::imageops::FilterType::Triangle);
        let gray_img = resized.to_luma8();

        // Convert to NCHW format and normalize to [0, 1]
        let mut input = Array::zeros(IxDyn(&[1, 1, height as usize, width as usize]));

        for (x, y, pixel) in gray_img.enumerate_pixels() {
            let gray = pixel[0] as f32 / 255.0;
            input[[0, 0, y as usize, x as usize]] = gray;
        }

        Ok(input)
    }

    /// Apply Non-Maximum Suppression (NMS)
    fn nms(&self, boxes: Vec<(BoundingBox, f32)>) -> Vec<(BoundingBox, f32)> {
        if boxes.is_empty() {
            return vec![];
        }

        let mut sorted_boxes = boxes.clone();
        sorted_boxes.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut keep = Vec::new();

        while !sorted_boxes.is_empty() {
            let current = sorted_boxes.remove(0);
            keep.push(current.clone());

            sorted_boxes.retain(|box_item| {
                let iou = self.calculate_iou(&current.0, &box_item.0);
                iou < self.config.iou_threshold
            });
        }

        keep
    }

    /// Calculate Intersection over Union (IoU)
    fn calculate_iou(&self, box1: &BoundingBox, box2: &BoundingBox) -> f32 {
        let x1 = box1.x.max(box2.x);
        let y1 = box1.y.max(box2.y);
        let x2 = (box1.x + box1.width).min(box2.x + box2.width);
        let y2 = (box1.y + box1.height).min(box2.y + box2.height);

        let intersection = if x2 > x1 && y2 > y1 {
            ((x2 - x1) * (y2 - y1)) as f32
        } else {
            0.0
        };

        let area1 = (box1.width * box1.height) as f32;
        let area2 = (box2.width * box2.height) as f32;
        let union = area1 + area2 - intersection;

        if union > 0.0 {
            intersection / union
        } else {
            0.0
        }
    }

    /// Post-process detection output (YOLOv8 format)
    fn postprocess_detection(
        &self,
        output: Array<f32, IxDyn>,
        original_width: u32,
        original_height: u32,
    ) -> Result<Vec<(BoundingBox, f32)>> {
        let scale_x = original_width as f32 / self.config.detection_input_size as f32;
        let scale_y = original_height as f32 / self.config.detection_input_size as f32;

        let mut boxes = Vec::new();

        // YOLOv8 output format: [batch, 5, num_predictions] (4 box coords + 1 confidence)
        let num_predictions = output.shape()[2];

        for i in 0..num_predictions {
            // Get confidence score (index 4)
            let confidence = output[[0, 4, i]];

            // Filter by confidence threshold
            if confidence < self.config.confidence_threshold {
                continue;
            }

            // Extract bounding box (cx, cy, w, h)
            let cx = output[[0, 0, i]];
            let cy = output[[0, 1, i]];
            let w = output[[0, 2, i]];
            let h = output[[0, 3, i]];

            // Convert to (x, y, w, h) and scale to original image
            let x = ((cx - w / 2.0) * scale_x).max(0.0) as u32;
            let y = ((cy - h / 2.0) * scale_y).max(0.0) as u32;
            let width = (w * scale_x).min(original_width as f32) as u32;
            let height = (h * scale_y).min(original_height as f32) as u32;

            boxes.push((
                BoundingBox {
                    x,
                    y,
                    width,
                    height,
                },
                confidence,
            ));
        }

        // Apply NMS
        let filtered_boxes = self.nms(boxes);

        Ok(filtered_boxes.into_iter().take(self.config.max_detections).collect())
    }

    /// Perform OCR on a cropped plate image using CTC decoding
    fn recognize_plate(&self, plate_img: &DynamicImage) -> Result<String> {
        if self.ocr_session.is_none() {
            return Ok("N/A".to_string());
        }

        let session_lock = self
            .ocr_session
            .as_ref()
            .context("OCR model not initialized")?;

        // Preprocess plate image
        let input_array = self.preprocess_for_ocr(plate_img)?;

        // Convert to ort Value
        let input_tensor = Value::from_array(input_array)?;

        // Run OCR inference
        let mut session = session_lock
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock OCR session: {}", e))?;
        let outputs = session.run(ort::inputs![input_tensor])?;

        // Get output tensor (softmax probabilities over vocabulary)
        // Expected shape: [batch, sequence_length, vocab_size]
        // Note: Different OCR models may use different output names (output, output0, etc.)
        let output_value = outputs
            .get("output")
            .or_else(|| outputs.get("output0"))
            .or_else(|| outputs.get("logits"))
            .context("No OCR output tensor found (tried: output, output0, logits)")?;
        let (shape, data) = output_value.try_extract_tensor::<f32>()?;

        let shape_usize: Vec<usize> = shape.as_ref().iter().map(|&x| x as usize).collect();
        let output = Array::from_shape_vec(IxDyn(&shape_usize), data.to_vec())?;

        // Decode using CTC greedy decoding
        let text = self.ctc_decode(&output)?;

        Ok(text)
    }

    /// CTC greedy decoding
    fn ctc_decode(&self, output: &Array<f32, IxDyn>) -> Result<String> {
        let sequence_length = output.shape()[1];
        let vocab_size = output.shape()[2];

        let mut result = String::new();
        let mut prev_char_idx = 0; // CTC blank is index 0

        for t in 0..sequence_length {
            // Find character with highest probability at this timestep
            let mut max_prob = output[[0, t, 0]];
            let mut max_idx = 0;

            for c in 1..vocab_size {
                let prob = output[[0, t, c]];
                if prob > max_prob {
                    max_prob = prob;
                    max_idx = c;
                }
            }

            // CTC decoding: skip blank (index 0) and repeated characters
            if max_idx > 0 && max_idx != prev_char_idx {
                // Convert index to character (vocab is 1-indexed, with blank at 0)
                let char_idx = max_idx - 1;
                if char_idx < self.config.char_vocab.len() {
                    let ch = self.config.char_vocab.chars().nth(char_idx).context("Invalid character index")?;
                    result.push(ch);
                }
            }

            prev_char_idx = max_idx;
        }

        Ok(result)
    }

    /// Create ONNX session with execution provider fallback
    fn create_session(&self, model_path: &str) -> Result<(Session, String)> {
        let provider_preference = self.config.execution_provider.to_uppercase();

        match provider_preference.as_str() {
            "TENSORRT" => {
                tracing::info!("Attempting TensorRT for {}", model_path);
                let result = Session::builder()
                    .context("Failed to create session builder")?
                    .with_optimization_level(GraphOptimizationLevel::Level3)
                    .context("Failed to set optimization level")?
                    .with_intra_threads(self.config.intra_threads)
                    .context("Failed to set intra threads")?
                    .with_inter_threads(self.config.inter_threads)
                    .context("Failed to set inter threads")?
                    .with_execution_providers([
                        TensorRTExecutionProvider::default()
                            .with_device_id(self.config.device_id)
                            .build(),
                        CUDAExecutionProvider::default()
                            .with_device_id(self.config.device_id)
                            .build(),
                        CPUExecutionProvider::default().build(),
                    ])
                    .context("Failed to set execution providers")?
                    .commit_from_file(model_path);

                match result {
                    Ok(session) => {
                        tracing::info!("TensorRT configured for {}", model_path);
                        Ok((session, "TensorRT".to_string()))
                    }
                    Err(e) => {
                        tracing::warn!("TensorRT failed, trying CUDA: {}", e);
                        self.try_cuda(model_path)
                    }
                }
            }
            "CUDA" => self.try_cuda(model_path),
            _ => self.try_cpu(model_path),
        }
    }

    fn try_cuda(&self, model_path: &str) -> Result<(Session, String)> {
        tracing::info!("Attempting CUDA for {}", model_path);
        let result = Session::builder()
            .context("Failed to create session builder")?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .context("Failed to set optimization level")?
            .with_intra_threads(self.config.intra_threads)
            .context("Failed to set intra threads")?
            .with_inter_threads(self.config.inter_threads)
            .context("Failed to set inter threads")?
            .with_execution_providers([
                CUDAExecutionProvider::default()
                    .with_device_id(self.config.device_id)
                    .build(),
                CPUExecutionProvider::default().build(),
            ])
            .context("Failed to set execution providers")?
            .commit_from_file(model_path);

        match result {
            Ok(session) => {
                tracing::info!("CUDA configured for {}", model_path);
                Ok((session, "CUDA".to_string()))
            }
            Err(e) => {
                tracing::warn!("CUDA failed, using CPU: {}", e);
                self.try_cpu(model_path)
            }
        }
    }

    fn try_cpu(&self, model_path: &str) -> Result<(Session, String)> {
        tracing::info!("Using CPU for {}", model_path);
        let session = Session::builder()
            .context("Failed to create session builder")?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .context("Failed to set optimization level")?
            .with_intra_threads(self.config.intra_threads)
            .context("Failed to set intra threads")?
            .with_inter_threads(self.config.inter_threads)
            .context("Failed to set inter threads")?
            .commit_from_file(model_path)
            .context("Failed to load model from file")?;
        Ok((session, "CPU".to_string()))
    }
}

impl Default for LprPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AiPlugin for LprPlugin {
    fn id(&self) -> &'static str {
        "lpr"
    }

    fn name(&self) -> &'static str {
        "License Plate Recognition"
    }

    fn description(&self) -> &'static str {
        "Two-stage license plate detection and recognition using ONNX models"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn config_schema(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "detection_model_path": {
                    "type": "string",
                    "default": "models/lpr_detector.onnx",
                    "description": "Path to the license plate detection ONNX model"
                },
                "ocr_model_path": {
                    "type": "string",
                    "description": "Path to the OCR ONNX model (optional)"
                },
                "confidence_threshold": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "default": 0.6,
                    "description": "Minimum confidence threshold for plate detections"
                },
                "iou_threshold": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "default": 0.4,
                    "description": "IoU threshold for Non-Maximum Suppression"
                },
                "max_detections": {
                    "type": "integer",
                    "minimum": 1,
                    "default": 10,
                    "description": "Maximum number of plates per frame"
                },
                "detection_input_size": {
                    "type": "integer",
                    "default": 640,
                    "description": "Detection model input size"
                },
                "ocr_input_width": {
                    "type": "integer",
                    "default": 200,
                    "description": "OCR model input width"
                },
                "ocr_input_height": {
                    "type": "integer",
                    "default": 64,
                    "description": "OCR model input height"
                },
                "char_vocab": {
                    "type": "string",
                    "default": "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ-",
                    "description": "Character vocabulary for OCR"
                },
                "execution_provider": {
                    "type": "string",
                    "enum": ["CPU", "CUDA", "TensorRT"],
                    "default": "CUDA",
                    "description": "Execution provider (CPU, CUDA, TensorRT)"
                },
                "device_id": {
                    "type": "integer",
                    "minimum": 0,
                    "default": 0,
                    "description": "GPU device ID"
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
            "required": ["detection_model_path"]
        }))
    }

    fn supported_formats(&self) -> Vec<String> {
        vec!["jpeg".to_string(), "png".to_string()]
    }

    fn requires_gpu(&self) -> bool {
        false // Can run on CPU, GPU is optional
    }

    async fn init(&mut self, config: serde_json::Value) -> Result<()> {
        if !config.is_null() {
            self.config = serde_json::from_value(config)?;
        }

        // Read GPU configuration from environment variables if set
        if let Ok(provider) = std::env::var("LPR_EXECUTION_PROVIDER") {
            self.config.execution_provider = provider;
        }
        if let Ok(device_id) = std::env::var("LPR_DEVICE_ID") {
            if let Ok(id) = device_id.parse::<i32>() {
                self.config.device_id = id;
            }
        }

        // Initialize detection model
        let (detection_session, actual_provider) =
            self.create_session(&self.config.detection_model_path)?;
        self.detection_session = Some(Arc::new(Mutex::new(detection_session)));
        *self.execution_provider_used.lock().map_err(|e| anyhow::anyhow!("Failed to lock execution provider: {}", e))? = actual_provider.clone();

        tracing::info!(
            "Initialized LPR detection model - path: {}, provider: {}, device: {}",
            self.config.detection_model_path,
            actual_provider,
            self.config.device_id
        );

        // Initialize OCR model if provided
        if let Some(ref ocr_path) = self.config.ocr_model_path {
            let (ocr_session, ocr_provider) = self.create_session(ocr_path)?;
            self.ocr_session = Some(Arc::new(Mutex::new(ocr_session)));

            tracing::info!(
                "Initialized LPR OCR model - path: {}, provider: {}",
                ocr_path,
                ocr_provider
            );
        } else {
            tracing::info!("OCR model not configured - detection only mode");
        }

        Ok(())
    }

    async fn process_frame(&self, frame: &VideoFrame) -> Result<AiResult> {
        let start = std::time::Instant::now();

        let detection_session_lock = self
            .detection_session
            .as_ref()
            .context("Detection model not initialized - call init() first")?;

        // Decode base64 image
        let image_data = base64::prelude::BASE64_STANDARD
            .decode(&frame.data)
            .context("Failed to decode base64 image")?;

        let img = image::load_from_memory(&image_data).context("Failed to load image")?;

        let original_width = img.width();
        let original_height = img.height();

        // Stage 1: Detect license plates
        let input_array = self.preprocess_for_detection(&img)?;
        let input_tensor = Value::from_array(input_array)?;

        let inference_start = std::time::Instant::now();
        let mut detection_session = detection_session_lock
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock detection session: {}", e))?;
        let outputs = detection_session.run(ort::inputs![input_tensor])?;
        let detection_time = inference_start.elapsed();

        // Get detection output - try common YOLO output names
        let output_value = outputs
            .get("output0")
            .or_else(|| outputs.get("output"))
            .or_else(|| outputs.get("boxes"))
            .context("No detection output tensor found (tried: output0, output, boxes)")?;
        let (shape, data) = output_value.try_extract_tensor::<f32>()?;

        let shape_usize: Vec<usize> = shape.as_ref().iter().map(|&x| x as usize).collect();
        let output = Array::from_shape_vec(IxDyn(&shape_usize), data.to_vec())?;

        // Post-process detections
        let plate_boxes = self.postprocess_detection(output, original_width, original_height)?;

        // Stage 2: OCR on each detected plate
        let mut detections = Vec::new();
        for (bbox, confidence) in plate_boxes {
            // Crop plate region
            let plate_img = img.crop_imm(bbox.x, bbox.y, bbox.width, bbox.height);

            // Perform OCR
            let plate_text = self.recognize_plate(&plate_img).unwrap_or_else(|e| {
                tracing::warn!("OCR failed: {}", e);
                "UNKNOWN".to_string()
            });

            detections.push(Detection {
                class: "license_plate".to_string(),
                confidence,
                bbox,
                metadata: Some(serde_json::json!({
                    "plate_number": plate_text,
                })),
            });
        }

        let processing_time_ms = start.elapsed().as_millis() as u64;

        // Calculate average confidence
        let avg_confidence = if !detections.is_empty() {
            detections.iter().map(|d| d.confidence).sum::<f32>() / detections.len() as f32
        } else {
            0.0
        };

        let execution_provider = self.execution_provider_used.lock().map_err(|e| anyhow::anyhow!("Failed to lock execution provider: {}", e))?.clone();

        // Track metrics
        telemetry::metrics::AI_SERVICE_GPU_INFERENCE
            .with_label_values(&[self.id(), &execution_provider])
            .inc();

        telemetry::metrics::AI_SERVICE_INFERENCE_TIME
            .with_label_values(&[self.id(), &execution_provider])
            .observe(detection_time.as_secs_f64());

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
                "detection_model": self.config.detection_model_path,
                "ocr_model": self.config.ocr_model_path,
                "execution_provider": execution_provider,
                "device_id": self.config.device_id,
                "detection_time_ms": detection_time.as_millis() as u64
            })),
        })
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(self.detection_session.is_some())
    }

    async fn shutdown(&mut self) -> Result<()> {
        tracing::info!("Shutting down LPR plugin");
        self.detection_session = None;
        self.ocr_session = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = LprConfig::default();
        assert_eq!(config.confidence_threshold, 0.6);
        assert_eq!(config.iou_threshold, 0.4);
        assert_eq!(config.max_detections, 10);
        assert_eq!(config.detection_input_size, 640);
        assert_eq!(config.ocr_input_width, 200);
        assert_eq!(config.ocr_input_height, 64);
        assert!(config.char_vocab.contains("0123456789"));
        assert!(config.char_vocab.contains("ABCDEFGHIJKLMNOPQRSTUVWXYZ"));
    }

    #[test]
    fn test_calculate_iou() {
        let plugin = LprPlugin::new();

        let box1 = BoundingBox {
            x: 10,
            y: 10,
            width: 50,
            height: 20,
        };
        let box2 = BoundingBox {
            x: 30,
            y: 15,
            width: 50,
            height: 20,
        };

        let iou = plugin.calculate_iou(&box1, &box2);
        assert!(iou > 0.0 && iou < 1.0);

        // Identical boxes
        let iou_same = plugin.calculate_iou(&box1, &box1);
        assert!((iou_same - 1.0).abs() < 0.001);

        // Non-overlapping boxes
        let box3 = BoundingBox {
            x: 100,
            y: 100,
            width: 50,
            height: 20,
        };
        let iou_none = plugin.calculate_iou(&box1, &box3);
        assert_eq!(iou_none, 0.0);
    }

    #[test]
    fn test_nms() {
        let plugin = LprPlugin::new();

        let boxes = vec![
            (
                BoundingBox {
                    x: 10,
                    y: 10,
                    width: 100,
                    height: 30,
                },
                0.9,
            ),
            (
                BoundingBox {
                    x: 15,
                    y: 12,
                    width: 100,
                    height: 30,
                },
                0.8,
            ),
            (
                BoundingBox {
                    x: 200,
                    y: 200,
                    width: 100,
                    height: 30,
                },
                0.85,
            ),
        ];

        let filtered = plugin.nms(boxes);
        // Should keep highest confidence from overlapping + non-overlapping
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].1, 0.9);
    }

    #[test]
    fn test_ctc_decode_simple() {
        let plugin = LprPlugin::new();

        // Create a simple output: [batch=1, sequence=5, vocab=38]
        // Simulate output for "ABC" (indices 11, 12, 13 in default vocab after digits)
        let vocab_size = plugin.config.char_vocab.len() + 1; // +1 for CTC blank
        let sequence_length = 5;

        let mut output_data = vec![0.0f32; 1 * sequence_length * vocab_size];

        // Fill with small probabilities
        for i in 0..output_data.len() {
            output_data[i] = 0.01;
        }

        // Set high probabilities for our target characters
        // Format: [batch, timestep, char_index]
        // Blank at t=0
        output_data[0] = 0.9; // t=0, blank

        // 'A' at t=1 (index 10+1=11 in vocab, which is 'A')
        output_data[1 * vocab_size + 11] = 0.9;

        // 'B' at t=2 (index 11+1=12)
        output_data[2 * vocab_size + 12] = 0.9;

        // 'C' at t=3 (index 12+1=13)
        output_data[3 * vocab_size + 13] = 0.9;

        // Blank at t=4
        output_data[4 * vocab_size + 0] = 0.9;

        let output = Array::from_shape_vec(
            IxDyn(&[1, sequence_length, vocab_size]),
            output_data,
        ).expect("Failed to create test array");

        let result = plugin.ctc_decode(&output).expect("CTC decode failed");
        assert_eq!(result, "ABC");
    }
}

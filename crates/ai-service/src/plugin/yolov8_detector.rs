/// YOLOv8 object detection plugin using ONNX Runtime
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
pub struct YoloV8Config {
    /// Path to the ONNX model file
    pub model_path: String,

    /// Confidence threshold for detections (0.0 to 1.0)
    #[serde(default = "default_confidence")]
    pub confidence_threshold: f32,

    /// IoU (Intersection over Union) threshold for NMS
    #[serde(default = "default_iou_threshold")]
    pub iou_threshold: f32,

    /// Maximum number of detections per frame
    #[serde(default = "default_max_detections")]
    pub max_detections: usize,

    /// Model input size (width and height)
    #[serde(default = "default_input_size")]
    pub input_size: u32,

    /// COCO class names (default 80 classes)
    #[serde(default = "default_coco_classes")]
    pub class_names: Vec<String>,

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
    0.5
}

fn default_iou_threshold() -> f32 {
    0.45
}

fn default_max_detections() -> usize {
    100
}

fn default_input_size() -> u32 {
    640
}

fn default_coco_classes() -> Vec<String> {
    vec![
        "person", "bicycle", "car", "motorcycle", "airplane", "bus", "train", "truck", "boat",
        "traffic light", "fire hydrant", "stop sign", "parking meter", "bench", "bird", "cat",
        "dog", "horse", "sheep", "cow", "elephant", "bear", "zebra", "giraffe", "backpack",
        "umbrella", "handbag", "tie", "suitcase", "frisbee", "skis", "snowboard", "sports ball",
        "kite", "baseball bat", "baseball glove", "skateboard", "surfboard", "tennis racket",
        "bottle", "wine glass", "cup", "fork", "knife", "spoon", "bowl", "banana", "apple",
        "sandwich", "orange", "broccoli", "carrot", "hot dog", "pizza", "donut", "cake", "chair",
        "couch", "potted plant", "bed", "dining table", "toilet", "tv", "laptop", "mouse",
        "remote", "keyboard", "cell phone", "microwave", "oven", "toaster", "sink",
        "refrigerator", "book", "clock", "vase", "scissors", "teddy bear", "hair drier",
        "toothbrush",
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
    1
}

fn default_gpu_mem_limit() -> usize {
    0 // unlimited
}

impl Default for YoloV8Config {
    fn default() -> Self {
        Self {
            model_path: "models/yolov8n.onnx".to_string(),
            confidence_threshold: 0.5,
            iou_threshold: 0.45,
            max_detections: 100,
            input_size: 640,
            class_names: default_coco_classes(),
            execution_provider: default_execution_provider(),
            device_id: default_device_id(),
            intra_threads: default_intra_threads(),
            inter_threads: default_inter_threads(),
            gpu_mem_limit: default_gpu_mem_limit(),
        }
    }
}

/// YOLOv8 object detection plugin
pub struct YoloV8DetectorPlugin {
    config: YoloV8Config,
    session: Option<Arc<Mutex<Session>>>,
    execution_provider_used: Arc<Mutex<String>>,
}

impl YoloV8DetectorPlugin {
    pub fn new() -> Self {
        Self {
            config: YoloV8Config::default(),
            session: None,
            execution_provider_used: Arc::new(Mutex::new("CPU".to_string())),
        }
    }

    /// Preprocess image to YOLOv8 input format
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

    /// Apply Non-Maximum Suppression (NMS)
    fn nms(&self, boxes: Vec<(BoundingBox, f32, usize)>) -> Vec<(BoundingBox, f32, usize)> {
        if boxes.is_empty() {
            return vec![];
        }

        let mut sorted_boxes = boxes.clone();
        sorted_boxes.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

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

    /// Post-process YOLOv8 output
    fn postprocess_output(
        &self,
        output: Array<f32, IxDyn>,
        original_width: u32,
        original_height: u32,
    ) -> Result<Vec<Detection>> {
        let scale_x = original_width as f32 / self.config.input_size as f32;
        let scale_y = original_height as f32 / self.config.input_size as f32;

        let mut boxes = Vec::new();

        // YOLOv8 output format: [batch, 84, 8400] or [batch, num_classes+4, num_predictions]
        let num_predictions = output.shape()[2];
        let num_classes = output.shape()[1] - 4;

        for i in 0..num_predictions {
            let mut max_class_score = 0.0f32;
            let mut max_class_idx = 0;

            // Find the class with highest score
            for class_idx in 0..num_classes {
                let score = output[[0, 4 + class_idx, i]];
                if score > max_class_score {
                    max_class_score = score;
                    max_class_idx = class_idx;
                }
            }

            // Filter by confidence threshold
            if max_class_score < self.config.confidence_threshold {
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
                max_class_score,
                max_class_idx,
            ));
        }

        // Apply NMS
        let filtered_boxes = self.nms(boxes);

        // Convert to Detection objects
        let detections: Vec<Detection> = filtered_boxes
            .into_iter()
            .take(self.config.max_detections)
            .map(|(bbox, confidence, class_idx)| {
                let class = if class_idx < self.config.class_names.len() {
                    self.config.class_names[class_idx].clone()
                } else {
                    format!("class_{}", class_idx)
                };

                Detection {
                    class,
                    confidence,
                    bbox,
                    metadata: Some(serde_json::json!({
                        "class_id": class_idx
                    })),
                }
            })
            .collect();

        Ok(detections)
    }
}

impl Default for YoloV8DetectorPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AiPlugin for YoloV8DetectorPlugin {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn id(&self) -> &'static str {
        "yolov8_detector"
    }

    fn name(&self) -> &'static str {
        "YOLOv8 Object Detector"
    }

    fn description(&self) -> &'static str {
        "Real-time object detection using YOLOv8 ONNX model"
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
                    "default": "models/yolov8n.onnx",
                    "description": "Path to the YOLOv8 ONNX model file"
                },
                "confidence_threshold": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "default": 0.5,
                    "description": "Minimum confidence threshold for detections"
                },
                "iou_threshold": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "default": 0.45,
                    "description": "IoU threshold for Non-Maximum Suppression"
                },
                "max_detections": {
                    "type": "integer",
                    "minimum": 1,
                    "default": 100,
                    "description": "Maximum number of detections per frame"
                },
                "input_size": {
                    "type": "integer",
                    "default": 640,
                    "description": "Model input size (width and height)"
                },
                "class_names": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "List of class names (default: COCO 80 classes)"
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
                },
                "gpu_mem_limit": {
                    "type": "integer",
                    "minimum": 0,
                    "default": 0,
                    "description": "GPU memory limit in bytes (0 = unlimited)"
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

        // Read GPU configuration from environment variables if set
        if let Ok(provider) = std::env::var("YOLOV8_EXECUTION_PROVIDER") {
            self.config.execution_provider = provider;
        }
        if let Ok(device_id) = std::env::var("YOLOV8_DEVICE_ID") {
            if let Ok(id) = device_id.parse::<i32>() {
                self.config.device_id = id;
            }
        }
        if let Ok(gpu_mem) = std::env::var("YOLOV8_GPU_MEM_LIMIT") {
            if let Ok(limit) = gpu_mem.parse::<usize>() {
                self.config.gpu_mem_limit = limit;
            }
        }

        // Try to configure execution providers with fallback
        let provider_preference = self.config.execution_provider.to_uppercase();
        let (session, actual_provider) = match provider_preference.as_str() {
            "TENSORRT" => {
                tracing::info!("Attempting to use TensorRT execution provider (device: {})", self.config.device_id);
                let result = Session::builder()?
                    .with_optimization_level(GraphOptimizationLevel::Level3)?
                    .with_intra_threads(self.config.intra_threads)?
                    .with_inter_threads(self.config.inter_threads)?
                    .with_execution_providers([
                        TensorRTExecutionProvider::default()
                            .with_device_id(self.config.device_id)
                            .build(),
                        CUDAExecutionProvider::default()
                            .with_device_id(self.config.device_id)
                            .build(),
                        CPUExecutionProvider::default().build()
                    ])?
                    .commit_from_file(&self.config.model_path);

                match result {
                    Ok(session) => {
                        tracing::info!("Successfully configured TensorRT execution provider");
                        (session, "TensorRT".to_string())
                    }
                    Err(e) => {
                        tracing::warn!("Failed with TensorRT, trying CUDA: {}", e);
                        // Try CUDA
                        let cuda_result = Session::builder()?
                            .with_optimization_level(GraphOptimizationLevel::Level3)?
                            .with_intra_threads(self.config.intra_threads)?
                            .with_inter_threads(self.config.inter_threads)?
                            .with_execution_providers([
                                CUDAExecutionProvider::default()
                                    .with_device_id(self.config.device_id)
                                    .build(),
                                CPUExecutionProvider::default().build()
                            ])?
                            .commit_from_file(&self.config.model_path);

                        match cuda_result {
                            Ok(session) => {
                                tracing::info!("Fell back to CUDA execution provider");
                                (session, "CUDA".to_string())
                            }
                            Err(e) => {
                                tracing::warn!("Failed with CUDA, using CPU: {}", e);
                                let cpu_session = Session::builder()?
                                    .with_optimization_level(GraphOptimizationLevel::Level3)?
                                    .with_intra_threads(self.config.intra_threads)?
                                    .with_inter_threads(self.config.inter_threads)?
                                    .commit_from_file(&self.config.model_path)?;
                                (cpu_session, "CPU".to_string())
                            }
                        }
                    }
                }
            }
            "CUDA" => {
                tracing::info!("Attempting to use CUDA execution provider (device: {})", self.config.device_id);
                let result = Session::builder()?
                    .with_optimization_level(GraphOptimizationLevel::Level3)?
                    .with_intra_threads(self.config.intra_threads)?
                    .with_inter_threads(self.config.inter_threads)?
                    .with_execution_providers([
                        CUDAExecutionProvider::default()
                            .with_device_id(self.config.device_id)
                            .build(),
                        CPUExecutionProvider::default().build()
                    ])?
                    .commit_from_file(&self.config.model_path);

                match result {
                    Ok(session) => {
                        tracing::info!("Successfully configured CUDA execution provider");
                        (session, "CUDA".to_string())
                    }
                    Err(e) => {
                        tracing::warn!("Failed with CUDA, using CPU: {}", e);
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
                tracing::info!("Using CPU execution provider");
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
            "Initialized YOLOv8 detector - model: {}, provider: {}, device: {}, confidence: {}, input_size: {}",
            self.config.model_path,
            actual_provider,
            self.config.device_id,
            self.config.confidence_threshold,
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

        // Run inference - acquire lock for session and measure inference time
        let inference_start = std::time::Instant::now();
        let mut session = session_lock.lock().map_err(|e| anyhow::anyhow!("Failed to lock session: {}", e))?;
        let outputs = session.run(ort::inputs![input_tensor])?;
        let inference_time = inference_start.elapsed();

        // Get output tensor - output is at index 0 (use string key for named outputs)
        let output_value = outputs.get("output0").context("No output tensor found")?;
        let (shape, data) = output_value.try_extract_tensor::<f32>()?;

        // Convert shape from i64 to usize
        let shape_usize: Vec<usize> = shape.as_ref().iter().map(|&x| x as usize).collect();

        // Convert to ndarray
        let output = Array::from_shape_vec(IxDyn(&shape_usize), data.to_vec())?;

        // Post-process results
        let detections = self.postprocess_output(output, original_width, original_height)?;

        let processing_time_ms = start.elapsed().as_millis() as u64;

        // Calculate average confidence
        let avg_confidence = if !detections.is_empty() {
            detections.iter().map(|d| d.confidence).sum::<f32>() / detections.len() as f32
        } else {
            0.0
        };

        let execution_provider = self.execution_provider_used.lock().unwrap().clone();

        // Track GPU/CPU inference metrics
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
                "inference_time_ms": inference_time.as_millis() as u64
            })),
        })
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(self.session.is_some())
    }

    async fn shutdown(&mut self) -> Result<()> {
        tracing::info!("Shutting down YOLOv8 detector");
        self.session = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = YoloV8Config::default();
        assert_eq!(config.confidence_threshold, 0.5);
        assert_eq!(config.iou_threshold, 0.45);
        assert_eq!(config.max_detections, 100);
        assert_eq!(config.input_size, 640);
        assert_eq!(config.class_names.len(), 80);
    }

    #[test]
    fn test_calculate_iou() {
        let plugin = YoloV8DetectorPlugin::new();

        let box1 = BoundingBox {
            x: 10,
            y: 10,
            width: 50,
            height: 50,
        };
        let box2 = BoundingBox {
            x: 30,
            y: 30,
            width: 50,
            height: 50,
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
            height: 50,
        };
        let iou_none = plugin.calculate_iou(&box1, &box3);
        assert_eq!(iou_none, 0.0);
    }

    #[test]
    fn test_nms() {
        let plugin = YoloV8DetectorPlugin::new();

        let boxes = vec![
            (
                BoundingBox {
                    x: 10,
                    y: 10,
                    width: 50,
                    height: 50,
                },
                0.9,
                0,
            ),
            (
                BoundingBox {
                    x: 15,
                    y: 15,
                    width: 50,
                    height: 50,
                },
                0.8,
                0,
            ),
            (
                BoundingBox {
                    x: 100,
                    y: 100,
                    width: 50,
                    height: 50,
                },
                0.85,
                1,
            ),
        ];

        let filtered = plugin.nms(boxes);
        // Should keep the highest confidence box from overlapping ones + non-overlapping box
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].1, 0.9); // Highest confidence kept first
    }
}

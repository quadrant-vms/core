/// Facial Recognition plugin using ONNX Runtime
///
/// This plugin performs two-stage facial recognition:
/// 1. Detection stage: Locates faces in the image using RetinaFace/SCRFD
/// 2. Embedding stage: Extracts facial embeddings using ArcFace/FaceNet
/// 3. Matching stage: Compares embeddings against enrolled face database
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
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacialRecognitionConfig {
    /// Path to the face detection ONNX model file
    pub detection_model_path: String,

    /// Path to the face embedding ONNX model file (optional - if not provided, only detection is performed)
    pub embedding_model_path: Option<String>,

    /// Confidence threshold for face detections (0.0 to 1.0)
    #[serde(default = "default_confidence")]
    pub confidence_threshold: f32,

    /// IoU (Intersection over Union) threshold for NMS
    #[serde(default = "default_iou_threshold")]
    pub iou_threshold: f32,

    /// Maximum number of faces to detect per frame
    #[serde(default = "default_max_detections")]
    pub max_detections: usize,

    /// Detection model input size (width and height)
    #[serde(default = "default_detection_input_size")]
    pub detection_input_size: u32,

    /// Embedding model input size (width and height)
    #[serde(default = "default_embedding_input_size")]
    pub embedding_input_size: u32,

    /// Cosine similarity threshold for face matching (0.0 to 1.0, higher = stricter)
    #[serde(default = "default_similarity_threshold")]
    pub similarity_threshold: f32,

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
    50
}

fn default_detection_input_size() -> u32 {
    640
}

fn default_embedding_input_size() -> u32 {
    112
}

fn default_similarity_threshold() -> f32 {
    0.5
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

impl Default for FacialRecognitionConfig {
    fn default() -> Self {
        Self {
            detection_model_path: "models/face_detector.onnx".to_string(),
            embedding_model_path: Some("models/face_embedding.onnx".to_string()),
            confidence_threshold: default_confidence(),
            iou_threshold: default_iou_threshold(),
            max_detections: default_max_detections(),
            detection_input_size: default_detection_input_size(),
            embedding_input_size: default_embedding_input_size(),
            similarity_threshold: default_similarity_threshold(),
            execution_provider: default_execution_provider(),
            device_id: default_device_id(),
            intra_threads: default_intra_threads(),
            inter_threads: default_inter_threads(),
        }
    }
}

/// Enrolled face record in the database
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnrolledFace {
    /// Unique face ID
    pub face_id: String,

    /// Person's name or identifier
    pub name: String,

    /// Face embedding vector (512-D typical for ArcFace/FaceNet)
    pub embedding: Vec<f32>,

    /// Additional metadata (age, gender, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,

    /// Enrollment timestamp (Unix timestamp in milliseconds)
    pub enrolled_at: u64,
}

/// Face match result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceMatch {
    /// Matched face ID from database
    pub face_id: String,

    /// Person's name
    pub name: String,

    /// Cosine similarity score (0.0 to 1.0)
    pub similarity: f32,

    /// Additional metadata from enrolled face
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Facial Recognition plugin
pub struct FacialRecognitionPlugin {
    config: FacialRecognitionConfig,
    detection_session: Option<Arc<tokio::sync::Mutex<Session>>>,
    embedding_session: Option<Arc<tokio::sync::Mutex<Session>>>,
    execution_provider_used: Arc<RwLock<String>>,
    /// In-memory face database: face_id -> EnrolledFace
    face_database: Arc<RwLock<HashMap<String, EnrolledFace>>>,
}

impl FacialRecognitionPlugin {
    pub fn new() -> Self {
        Self {
            config: FacialRecognitionConfig::default(),
            detection_session: None,
            embedding_session: None,
            execution_provider_used: Arc::new(RwLock::new("CPU".to_string())),
            face_database: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Enroll a new face into the database
    pub async fn enroll_face(
        &self,
        face_id: String,
        name: String,
        face_image: &DynamicImage,
        metadata: Option<serde_json::Value>,
    ) -> Result<EnrolledFace> {
        // Extract embedding from the face image
        let embedding = self.extract_embedding(face_image).await?;

        let enrolled_face = EnrolledFace {
            face_id: face_id.clone(),
            name,
            embedding,
            metadata,
            enrolled_at: common::validation::safe_unix_timestamp(),
        };

        // Store in database
        self.face_database
            .write()
            .map_err(|e| anyhow::anyhow!("Failed to lock face database: {}", e))?
            .insert(face_id, enrolled_face.clone());

        Ok(enrolled_face)
    }

    /// Remove a face from the database
    pub fn remove_face(&self, face_id: &str) -> Result<bool> {
        let removed = self
            .face_database
            .write()
            .map_err(|e| anyhow::anyhow!("Failed to lock face database: {}", e))?
            .remove(face_id)
            .is_some();
        Ok(removed)
    }

    /// List all enrolled faces
    pub fn list_faces(&self) -> Result<Vec<EnrolledFace>> {
        Ok(self
            .face_database
            .read()
            .map_err(|e| anyhow::anyhow!("Failed to lock face database: {}", e))?
            .values()
            .cloned()
            .collect())
    }

    /// Get face database size
    pub fn database_size(&self) -> Result<usize> {
        Ok(self
            .face_database
            .read()
            .map_err(|e| anyhow::anyhow!("Failed to lock face database: {}", e))?
            .len())
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

    /// Preprocess cropped face image for embedding model
    fn preprocess_for_embedding(&self, img: &DynamicImage) -> Result<Array<f32, IxDyn>> {
        let size = self.config.embedding_input_size;
        let resized = img.resize_exact(size, size, image::imageops::FilterType::Triangle);
        let rgb_img = resized.to_rgb8();

        // Convert to NCHW format and normalize to [-1, 1] (typical for ArcFace)
        let mut input = Array::zeros(IxDyn(&[1, 3, size as usize, size as usize]));

        for (x, y, pixel) in rgb_img.enumerate_pixels() {
            let r = (pixel[0] as f32 / 127.5) - 1.0;
            let g = (pixel[1] as f32 / 127.5) - 1.0;
            let b = (pixel[2] as f32 / 127.5) - 1.0;

            input[[0, 0, y as usize, x as usize]] = r;
            input[[0, 1, y as usize, x as usize]] = g;
            input[[0, 2, y as usize, x as usize]] = b;
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

    /// Post-process detection output (YOLO/RetinaFace format)
    fn postprocess_detection(
        &self,
        output: Array<f32, IxDyn>,
        original_width: u32,
        original_height: u32,
    ) -> Result<Vec<(BoundingBox, f32)>> {
        let scale_x = original_width as f32 / self.config.detection_input_size as f32;
        let scale_y = original_height as f32 / self.config.detection_input_size as f32;

        let mut boxes = Vec::new();

        // YOLO output format: [batch, 5, num_predictions] (4 box coords + 1 confidence)
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

    /// Extract face embedding vector
    async fn extract_embedding(&self, face_img: &DynamicImage) -> Result<Vec<f32>> {
        if self.embedding_session.is_none() {
            return Err(anyhow::anyhow!("Embedding model not initialized"));
        }

        let session_lock = self
            .embedding_session
            .as_ref()
            .context("Embedding model not initialized")?;

        // Preprocess face image
        let input_array = self.preprocess_for_embedding(face_img)?;

        // Convert to ort Value
        let input_tensor = Value::from_array(input_array)?;

        // Run embedding inference
        let mut session = session_lock.lock().await;
        let outputs = session.run(ort::inputs![input_tensor])?;

        // Get output tensor (embedding vector)
        // Expected shape: [batch, embedding_dim] (e.g., [1, 512])
        let output_value = outputs
            .get("output")
            .or_else(|| outputs.get("output0"))
            .or_else(|| outputs.get("embedding"))
            .context("No embedding output tensor found")?;
        let (shape, data) = output_value.try_extract_tensor::<f32>()?;

        let shape_usize: Vec<usize> = shape.as_ref().iter().map(|&x| x as usize).collect();
        let output = Array::from_shape_vec(IxDyn(&shape_usize), data.to_vec())?;

        // Extract embedding vector and normalize (L2 normalization)
        let embedding_dim = output.shape()[1];
        let mut embedding: Vec<f32> = Vec::with_capacity(embedding_dim);

        for i in 0..embedding_dim {
            embedding.push(output[[0, i]]);
        }

        // L2 normalization
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for val in &mut embedding {
                *val /= norm;
            }
        }

        Ok(embedding)
    }

    /// Calculate cosine similarity between two embeddings
    fn cosine_similarity(&self, embedding1: &[f32], embedding2: &[f32]) -> f32 {
        if embedding1.len() != embedding2.len() {
            return 0.0;
        }

        let dot_product: f32 = embedding1
            .iter()
            .zip(embedding2.iter())
            .map(|(a, b)| a * b)
            .sum();

        // Since embeddings are L2-normalized, dot product = cosine similarity
        dot_product
    }

    /// Match a face embedding against the database
    fn match_face(&self, embedding: &[f32]) -> Result<Option<FaceMatch>> {
        let database = self
            .face_database
            .read()
            .map_err(|e| anyhow::anyhow!("Failed to lock face database: {}", e))?;

        let mut best_match: Option<FaceMatch> = None;
        let mut best_similarity = self.config.similarity_threshold;

        for enrolled_face in database.values() {
            let similarity = self.cosine_similarity(embedding, &enrolled_face.embedding);

            if similarity > best_similarity {
                best_similarity = similarity;
                best_match = Some(FaceMatch {
                    face_id: enrolled_face.face_id.clone(),
                    name: enrolled_face.name.clone(),
                    similarity,
                    metadata: enrolled_face.metadata.clone(),
                });
            }
        }

        Ok(best_match)
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

impl Default for FacialRecognitionPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AiPlugin for FacialRecognitionPlugin {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn id(&self) -> &'static str {
        "facial_recognition"
    }

    fn name(&self) -> &'static str {
        "Facial Recognition"
    }

    fn description(&self) -> &'static str {
        "Two-stage facial recognition: detection + embedding extraction with face matching"
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
                    "default": "models/face_detector.onnx",
                    "description": "Path to the face detection ONNX model"
                },
                "embedding_model_path": {
                    "type": "string",
                    "description": "Path to the face embedding ONNX model (optional)"
                },
                "confidence_threshold": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "default": 0.6,
                    "description": "Minimum confidence threshold for face detections"
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
                    "default": 50,
                    "description": "Maximum number of faces per frame"
                },
                "detection_input_size": {
                    "type": "integer",
                    "default": 640,
                    "description": "Detection model input size"
                },
                "embedding_input_size": {
                    "type": "integer",
                    "default": 112,
                    "description": "Embedding model input size"
                },
                "similarity_threshold": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "default": 0.5,
                    "description": "Cosine similarity threshold for face matching"
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
        if let Ok(provider) = std::env::var("FACE_RECOGNITION_EXECUTION_PROVIDER") {
            self.config.execution_provider = provider;
        }
        if let Ok(device_id) = std::env::var("FACE_RECOGNITION_DEVICE_ID") {
            if let Ok(id) = device_id.parse::<i32>() {
                self.config.device_id = id;
            }
        }

        // Initialize detection model
        let (detection_session, actual_provider) =
            self.create_session(&self.config.detection_model_path)?;
        self.detection_session = Some(Arc::new(tokio::sync::Mutex::new(detection_session)));
        *self.execution_provider_used.write().map_err(|e| anyhow::anyhow!("Failed to lock execution provider: {}", e))? = actual_provider.clone();

        tracing::info!(
            "Initialized face detection model - path: {}, provider: {}, device: {}",
            self.config.detection_model_path,
            actual_provider,
            self.config.device_id
        );

        // Initialize embedding model if provided
        if let Some(ref embedding_path) = self.config.embedding_model_path {
            let (embedding_session, embedding_provider) = self.create_session(embedding_path)?;
            self.embedding_session = Some(Arc::new(tokio::sync::Mutex::new(embedding_session)));

            tracing::info!(
                "Initialized face embedding model - path: {}, provider: {}",
                embedding_path,
                embedding_provider
            );
        } else {
            tracing::info!("Embedding model not configured - detection only mode");
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

        // Stage 1: Detect faces
        let input_array = self.preprocess_for_detection(&img)?;
        let input_tensor = Value::from_array(input_array)?;

        let inference_start = std::time::Instant::now();
        let mut detection_session = detection_session_lock.lock().await;
        let outputs = detection_session.run(ort::inputs![input_tensor])?;
        let detection_time = inference_start.elapsed();

        // Get detection output
        let output_value = outputs
            .get("output0")
            .or_else(|| outputs.get("output"))
            .or_else(|| outputs.get("boxes"))
            .context("No detection output tensor found")?;
        let (shape, data) = output_value.try_extract_tensor::<f32>()?;

        let shape_usize: Vec<usize> = shape.as_ref().iter().map(|&x| x as usize).collect();
        let output = Array::from_shape_vec(IxDyn(&shape_usize), data.to_vec())?;

        // Post-process detections
        let face_boxes = self.postprocess_detection(output, original_width, original_height)?;

        // Stage 2: Extract embeddings and match faces
        let mut detections = Vec::new();
        for (bbox, confidence) in face_boxes {
            // Crop face region
            let face_img = img.crop_imm(bbox.x, bbox.y, bbox.width, bbox.height);

            // Extract embedding and try to match
            let match_result = if self.embedding_session.is_some() {
                match self.extract_embedding(&face_img).await {
                    Ok(embedding) => self.match_face(&embedding).ok().flatten(),
                    Err(e) => {
                        tracing::warn!("Embedding extraction failed: {}", e);
                        None
                    }
                }
            } else {
                None
            };

            // Create detection result
            let (class, metadata) = if let Some(face_match) = match_result {
                (
                    face_match.name.clone(),
                    Some(serde_json::json!({
                        "face_id": face_match.face_id,
                        "similarity": face_match.similarity,
                        "matched": true,
                        "metadata": face_match.metadata,
                    })),
                )
            } else {
                (
                    "unknown".to_string(),
                    Some(serde_json::json!({
                        "matched": false,
                    })),
                )
            };

            detections.push(Detection {
                class,
                confidence,
                bbox,
                metadata,
            });
        }

        let processing_time_ms = start.elapsed().as_millis() as u64;

        // Calculate average confidence
        let avg_confidence = if !detections.is_empty() {
            detections.iter().map(|d| d.confidence).sum::<f32>() / detections.len() as f32
        } else {
            0.0
        };

        let execution_provider = self.execution_provider_used.read().map_err(|e| anyhow::anyhow!("Failed to lock execution provider: {}", e))?.clone();

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
                "embedding_model": self.config.embedding_model_path,
                "execution_provider": execution_provider,
                "device_id": self.config.device_id,
                "detection_time_ms": detection_time.as_millis() as u64,
                "database_size": self.database_size().unwrap_or(0)
            })),
        })
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(self.detection_session.is_some())
    }

    async fn shutdown(&mut self) -> Result<()> {
        tracing::info!("Shutting down Facial Recognition plugin");
        self.detection_session = None;
        self.embedding_session = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = FacialRecognitionConfig::default();
        assert_eq!(config.confidence_threshold, 0.6);
        assert_eq!(config.iou_threshold, 0.4);
        assert_eq!(config.max_detections, 50);
        assert_eq!(config.detection_input_size, 640);
        assert_eq!(config.embedding_input_size, 112);
        assert_eq!(config.similarity_threshold, 0.5);
    }

    #[test]
    fn test_calculate_iou() {
        let plugin = FacialRecognitionPlugin::new();

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
    fn test_cosine_similarity() {
        let plugin = FacialRecognitionPlugin::new();

        // Identical normalized embeddings
        let embedding1 = vec![0.6, 0.8, 0.0, 0.0]; // L2 norm = 1.0
        let similarity = plugin.cosine_similarity(&embedding1, &embedding1);
        assert!((similarity - 1.0).abs() < 0.001);

        // Orthogonal normalized embeddings
        let embedding2 = vec![1.0, 0.0, 0.0, 0.0]; // L2 norm = 1.0
        let embedding3 = vec![0.0, 1.0, 0.0, 0.0]; // L2 norm = 1.0
        let similarity_orth = plugin.cosine_similarity(&embedding2, &embedding3);
        assert!(similarity_orth.abs() < 0.001);
    }

    #[test]
    fn test_database_operations() {
        let plugin = FacialRecognitionPlugin::new();

        assert_eq!(plugin.database_size().ok(), Some(0));

        // Test list empty database
        let faces = plugin.list_faces().ok();
        assert_eq!(faces, Some(vec![]));
    }
}

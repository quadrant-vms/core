/// Crowd analytics plugin for person counting and density analysis
use super::AiPlugin;
use anyhow::{Context, Result};
use async_trait::async_trait;
use common::ai_tasks::{AiResult, BoundingBox, Detection, VideoFrame};
use base64::Engine;
use image::DynamicImage;
use ndarray::{Array, IxDyn};
use ort::{
    execution_providers::{CPUExecutionProvider, CUDAExecutionProvider, TensorRTExecutionProvider},
    session::{builder::GraphOptimizationLevel, Session},
    value::Value,
};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrowdAnalyticsConfig {
    /// Path to the YOLOv8 ONNX model file for person detection
    pub model_path: String,

    /// Confidence threshold for person detections (0.0 to 1.0)
    #[serde(default = "default_confidence")]
    pub confidence_threshold: f32,

    /// IoU threshold for NMS
    #[serde(default = "default_iou_threshold")]
    pub iou_threshold: f32,

    /// Model input size (width and height)
    #[serde(default = "default_input_size")]
    pub input_size: u32,

    /// Grid size for density heatmap (e.g., 10x10 grid)
    #[serde(default = "default_grid_size")]
    pub grid_size: usize,

    /// Assumed camera coverage area in square meters (for density calculation)
    #[serde(default = "default_coverage_area")]
    pub coverage_area_sqm: f32,

    /// Minimum cluster size for hotspot detection
    #[serde(default = "default_min_cluster_size")]
    pub min_cluster_size: usize,

    /// Clustering distance threshold (in pixels)
    #[serde(default = "default_cluster_distance")]
    pub cluster_distance_threshold: f32,

    /// Execution provider (CPU, CUDA, TensorRT)
    #[serde(default = "default_execution_provider")]
    pub execution_provider: String,

    /// GPU device ID
    #[serde(default)]
    pub device_id: i32,

    /// Number of intra-operation threads
    #[serde(default = "default_intra_threads")]
    pub intra_threads: usize,

    /// Number of inter-operation threads
    #[serde(default = "default_inter_threads")]
    pub inter_threads: usize,
}

fn default_confidence() -> f32 {
    0.5
}

fn default_iou_threshold() -> f32 {
    0.45
}

fn default_input_size() -> u32 {
    640
}

fn default_grid_size() -> usize {
    10
}

fn default_coverage_area() -> f32 {
    100.0 // 100 square meters default
}

fn default_min_cluster_size() -> usize {
    3
}

fn default_cluster_distance() -> f32 {
    100.0 // pixels
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

impl Default for CrowdAnalyticsConfig {
    fn default() -> Self {
        Self {
            model_path: "models/yolov8n.onnx".to_string(),
            confidence_threshold: 0.5,
            iou_threshold: 0.45,
            input_size: 640,
            grid_size: 10,
            coverage_area_sqm: 100.0,
            min_cluster_size: 3,
            cluster_distance_threshold: 100.0,
            execution_provider: "CUDA".to_string(),
            device_id: 0,
            intra_threads: 4,
            inter_threads: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrowdCluster {
    /// Center point of the cluster (x, y)
    pub center: (f32, f32),
    /// Number of people in this cluster
    pub count: usize,
    /// Bounding box containing all people in cluster
    pub bbox: BoundingBox,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DensityLevel {
    /// Density level: "low", "medium", "high", "critical"
    pub level: String,
    /// Density value (people per square meter)
    pub density: f32,
    /// Grid-based heatmap (grid_size x grid_size)
    pub heatmap: Vec<Vec<usize>>,
}

/// Crowd Analytics Plugin
pub struct CrowdAnalyticsPlugin {
    config: CrowdAnalyticsConfig,
    session: Option<Arc<Mutex<Session>>>,
    execution_provider_used: Arc<Mutex<String>>,
}

impl CrowdAnalyticsPlugin {
    pub fn new() -> Self {
        Self {
            config: CrowdAnalyticsConfig::default(),
            session: None,
            execution_provider_used: Arc::new(Mutex::new("CPU".to_string())),
        }
    }

    /// Preprocess image to YOLOv8 input format
    fn preprocess_image(&self, img: &DynamicImage) -> Result<Array<f32, IxDyn>> {
        let size = self.config.input_size;
        let resized = img.resize_exact(size, size, image::imageops::FilterType::Triangle);
        let rgb_img = resized.to_rgb8();

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

    /// Apply Non-Maximum Suppression
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

    /// Calculate IoU between two bounding boxes
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

    /// Post-process YOLOv8 output to detect people (class 0 in COCO)
    fn detect_people(
        &self,
        output: Array<f32, IxDyn>,
        original_width: u32,
        original_height: u32,
    ) -> Result<Vec<BoundingBox>> {
        let scale_x = original_width as f32 / self.config.input_size as f32;
        let scale_y = original_height as f32 / self.config.input_size as f32;

        let mut boxes = Vec::new();

        // YOLOv8 output format: [batch, 84, 8400]
        let num_predictions = output.shape()[2];

        for i in 0..num_predictions {
            // Person class is at index 0 in COCO dataset
            let person_score = output[[0, 4, i]];

            if person_score < self.config.confidence_threshold {
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
                person_score,
            ));
        }

        // Apply NMS
        let filtered_boxes = self.nms(boxes);
        Ok(filtered_boxes.into_iter().map(|(bbox, _)| bbox).collect())
    }

    /// Calculate density heatmap based on person locations
    fn calculate_density_heatmap(
        &self,
        people: &[BoundingBox],
        frame_width: u32,
        frame_height: u32,
    ) -> Vec<Vec<usize>> {
        let grid_size = self.config.grid_size;
        let mut heatmap = vec![vec![0; grid_size]; grid_size];

        let cell_width = frame_width as f32 / grid_size as f32;
        let cell_height = frame_height as f32 / grid_size as f32;

        for person in people {
            // Use center point of bounding box
            let center_x = person.x as f32 + person.width as f32 / 2.0;
            let center_y = person.y as f32 + person.height as f32 / 2.0;

            let grid_x = (center_x / cell_width).floor() as usize;
            let grid_y = (center_y / cell_height).floor() as usize;

            if grid_x < grid_size && grid_y < grid_size {
                heatmap[grid_y][grid_x] += 1;
            }
        }

        heatmap
    }

    /// Identify crowd clusters using simple distance-based clustering
    fn identify_clusters(&self, people: &[BoundingBox]) -> Vec<CrowdCluster> {
        if people.is_empty() {
            return vec![];
        }

        let mut clusters: Vec<Vec<usize>> = Vec::new();
        let mut assigned = vec![false; people.len()];

        for i in 0..people.len() {
            if assigned[i] {
                continue;
            }

            let mut cluster = vec![i];
            assigned[i] = true;

            // Find nearby people
            for j in 0..people.len() {
                if assigned[j] {
                    continue;
                }

                // Check distance to any member of current cluster
                for &cluster_member in &cluster {
                    let distance = self.calculate_distance(&people[cluster_member], &people[j]);
                    if distance < self.config.cluster_distance_threshold {
                        cluster.push(j);
                        assigned[j] = true;
                        break;
                    }
                }
            }

            if cluster.len() >= self.config.min_cluster_size {
                clusters.push(cluster);
            }
        }

        // Convert to CrowdCluster objects
        clusters
            .into_iter()
            .map(|cluster_indices| {
                let cluster_people: Vec<&BoundingBox> =
                    cluster_indices.iter().map(|&i| &people[i]).collect();

                // Calculate center and bounding box
                let center_x = cluster_people
                    .iter()
                    .map(|p| p.x + p.width / 2)
                    .sum::<u32>() as f32
                    / cluster_people.len() as f32;
                let center_y = cluster_people
                    .iter()
                    .map(|p| p.y + p.height / 2)
                    .sum::<u32>() as f32
                    / cluster_people.len() as f32;

                let min_x = cluster_people.iter().map(|p| p.x).min().unwrap_or(0);
                let min_y = cluster_people.iter().map(|p| p.y).min().unwrap_or(0);
                let max_x = cluster_people
                    .iter()
                    .map(|p| p.x + p.width)
                    .max()
                    .unwrap_or(0);
                let max_y = cluster_people
                    .iter()
                    .map(|p| p.y + p.height)
                    .max()
                    .unwrap_or(0);

                CrowdCluster {
                    center: (center_x, center_y),
                    count: cluster_people.len(),
                    bbox: BoundingBox {
                        x: min_x,
                        y: min_y,
                        width: max_x - min_x,
                        height: max_y - min_y,
                    },
                }
            })
            .collect()
    }

    /// Calculate Euclidean distance between centers of two bounding boxes
    fn calculate_distance(&self, box1: &BoundingBox, box2: &BoundingBox) -> f32 {
        let center1_x = box1.x as f32 + box1.width as f32 / 2.0;
        let center1_y = box1.y as f32 + box1.height as f32 / 2.0;
        let center2_x = box2.x as f32 + box2.width as f32 / 2.0;
        let center2_y = box2.y as f32 + box2.height as f32 / 2.0;

        let dx = center1_x - center2_x;
        let dy = center1_y - center2_y;

        (dx * dx + dy * dy).sqrt()
    }

    /// Calculate density level based on person count
    fn calculate_density_level(&self, person_count: usize) -> DensityLevel {
        let density = person_count as f32 / self.config.coverage_area_sqm;

        let level = if density < 0.1 {
            "low"
        } else if density < 0.3 {
            "medium"
        } else if density < 0.5 {
            "high"
        } else {
            "critical"
        };

        DensityLevel {
            level: level.to_string(),
            density,
            heatmap: vec![],
        }
    }
}

impl Default for CrowdAnalyticsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AiPlugin for CrowdAnalyticsPlugin {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn id(&self) -> &'static str {
        "crowd_analytics"
    }

    fn name(&self) -> &'static str {
        "Crowd Analytics"
    }

    fn description(&self) -> &'static str {
        "Person counting, crowd density analysis, and hotspot detection"
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
                    "description": "Path to YOLOv8 ONNX model for person detection"
                },
                "confidence_threshold": {
                    "type": "number",
                    "minimum": 0.0,
                    "maximum": 1.0,
                    "default": 0.5,
                    "description": "Confidence threshold for person detections"
                },
                "grid_size": {
                    "type": "integer",
                    "minimum": 5,
                    "maximum": 50,
                    "default": 10,
                    "description": "Grid size for density heatmap (NxN)"
                },
                "coverage_area_sqm": {
                    "type": "number",
                    "minimum": 1.0,
                    "default": 100.0,
                    "description": "Camera coverage area in square meters"
                },
                "min_cluster_size": {
                    "type": "integer",
                    "minimum": 2,
                    "default": 3,
                    "description": "Minimum people count for hotspot detection"
                },
                "cluster_distance_threshold": {
                    "type": "number",
                    "minimum": 10.0,
                    "default": 100.0,
                    "description": "Distance threshold for clustering (pixels)"
                }
            },
            "required": ["model_path"]
        }))
    }

    fn supported_formats(&self) -> Vec<String> {
        vec!["jpeg".to_string(), "png".to_string()]
    }

    fn requires_gpu(&self) -> bool {
        false
    }

    async fn init(&mut self, config: serde_json::Value) -> Result<()> {
        if !config.is_null() {
            self.config = serde_json::from_value(config)?;
        }

        // Override from environment variables
        if let Ok(provider) = std::env::var("CROWD_EXECUTION_PROVIDER") {
            self.config.execution_provider = provider;
        }
        if let Ok(device_id) = std::env::var("CROWD_DEVICE_ID") {
            if let Ok(id) = device_id.parse::<i32>() {
                self.config.device_id = id;
            }
        }

        // Initialize ONNX session with execution provider fallback
        let provider_preference = self.config.execution_provider.to_uppercase();
        let (session, actual_provider) = match provider_preference.as_str() {
            "TENSORRT" => {
                tracing::info!("Crowd Analytics: Attempting TensorRT (device: {})", self.config.device_id);
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
                        CPUExecutionProvider::default().build(),
                    ])?
                    .commit_from_file(&self.config.model_path);

                match result {
                    Ok(s) => {
                        tracing::info!("Crowd Analytics: Using TensorRT");
                        (s, "TensorRT".to_string())
                    }
                    Err(e) => {
                        tracing::warn!("TensorRT failed, trying CUDA: {}", e);
                        let cuda_result = Session::builder()?
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

                        match cuda_result {
                            Ok(s) => {
                                tracing::info!("Crowd Analytics: Using CUDA");
                                (s, "CUDA".to_string())
                            }
                            Err(e) => {
                                tracing::warn!("CUDA failed, using CPU: {}", e);
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
                tracing::info!("Crowd Analytics: Attempting CUDA (device: {})", self.config.device_id);
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
                    Ok(s) => {
                        tracing::info!("Crowd Analytics: Using CUDA");
                        (s, "CUDA".to_string())
                    }
                    Err(e) => {
                        tracing::warn!("CUDA failed, using CPU: {}", e);
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
                tracing::info!("Crowd Analytics: Using CPU");
                let session = Session::builder()?
                    .with_optimization_level(GraphOptimizationLevel::Level3)?
                    .with_intra_threads(self.config.intra_threads)?
                    .with_inter_threads(self.config.inter_threads)?
                    .commit_from_file(&self.config.model_path)?;
                (session, "CPU".to_string())
            }
        };

        self.session = Some(Arc::new(Mutex::new(session)));
        *self.execution_provider_used.lock().map_err(|e| anyhow::anyhow!("Lock error: {}", e))? = actual_provider.clone();

        tracing::info!(
            "Initialized Crowd Analytics - model: {}, provider: {}, grid: {}x{}",
            self.config.model_path,
            actual_provider,
            self.config.grid_size,
            self.config.grid_size
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

        // Convert to ONNX value
        let input_tensor = Value::from_array(input_array)?;

        // Run inference
        let inference_start = std::time::Instant::now();
        let mut session = session_lock
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock session: {}", e))?;
        let outputs = session.run(ort::inputs![input_tensor])?;
        let inference_time = inference_start.elapsed();

        // Get output tensor
        let output_value = outputs.get("output0").context("No output tensor found")?;
        let (shape, data) = output_value.try_extract_tensor::<f32>()?;

        let shape_usize: Vec<usize> = shape.as_ref().iter().map(|&x| x as usize).collect();
        let output = Array::from_shape_vec(IxDyn(&shape_usize), data.to_vec())?;

        // Detect people
        let people = self.detect_people(output, original_width, original_height)?;

        // Calculate density heatmap
        let heatmap = self.calculate_density_heatmap(&people, original_width, original_height);

        // Identify clusters
        let clusters = self.identify_clusters(&people);

        // Calculate density level
        let mut density_level = self.calculate_density_level(people.len());
        density_level.heatmap = heatmap.clone();

        let processing_time_ms = start.elapsed().as_millis() as u64;

        // Convert people to Detection objects for compatibility
        let detections: Vec<Detection> = people
            .iter()
            .enumerate()
            .map(|(i, bbox)| Detection {
                class: "person".to_string(),
                confidence: 0.0, // Confidence not tracked individually
                bbox: bbox.clone(),
                metadata: Some(serde_json::json!({
                    "detection_id": i
                })),
            })
            .collect();

        let execution_provider = self
            .execution_provider_used
            .lock()
            .map_err(|e| anyhow::anyhow!("Lock error: {}", e))?
            .clone();

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
            confidence: Some(1.0),
            processing_time_ms: Some(processing_time_ms),
            metadata: Some(serde_json::json!({
                "frame_width": original_width,
                "frame_height": original_height,
                "person_count": people.len(),
                "density_level": density_level.level,
                "density_value": density_level.density,
                "density_heatmap": density_level.heatmap,
                "clusters": clusters.iter().map(|c| serde_json::json!({
                    "center": c.center,
                    "count": c.count,
                    "bbox": c.bbox
                })).collect::<Vec<_>>(),
                "coverage_area_sqm": self.config.coverage_area_sqm,
                "execution_provider": execution_provider,
                "inference_time_ms": inference_time.as_millis() as u64
            })),
        })
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(self.session.is_some())
    }

    async fn shutdown(&mut self) -> Result<()> {
        tracing::info!("Shutting down Crowd Analytics plugin");
        self.session = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = CrowdAnalyticsConfig::default();
        assert_eq!(config.confidence_threshold, 0.5);
        assert_eq!(config.grid_size, 10);
        assert_eq!(config.coverage_area_sqm, 100.0);
        assert_eq!(config.min_cluster_size, 3);
    }

    #[test]
    fn test_density_heatmap() {
        let plugin = CrowdAnalyticsPlugin::new();

        let people = vec![
            BoundingBox {
                x: 50,
                y: 50,
                width: 30,
                height: 60,
            },
            BoundingBox {
                x: 200,
                y: 200,
                width: 30,
                height: 60,
            },
            BoundingBox {
                x: 500,
                y: 400,
                width: 30,
                height: 60,
            },
        ];

        let heatmap = plugin.calculate_density_heatmap(&people, 640, 480);
        assert_eq!(heatmap.len(), 10);
        assert_eq!(heatmap[0].len(), 10);

        // Should have counts in appropriate cells (all in different cells)
        let total_count: usize = heatmap.iter().map(|row| row.iter().sum::<usize>()).sum();
        assert_eq!(total_count, 3);
    }

    #[test]
    fn test_calculate_distance() {
        let plugin = CrowdAnalyticsPlugin::new();

        let box1 = BoundingBox {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        };
        let box2 = BoundingBox {
            x: 30,
            y: 40,
            width: 10,
            height: 10,
        };

        let distance = plugin.calculate_distance(&box1, &box2);
        // Distance between (5,5) and (35,45) = sqrt(30^2 + 40^2) = 50
        assert!((distance - 50.0).abs() < 1.0);
    }

    #[test]
    fn test_density_level() {
        let plugin = CrowdAnalyticsPlugin::new();

        let low = plugin.calculate_density_level(5);
        assert_eq!(low.level, "low");
        assert_eq!(low.density, 0.05);

        let high = plugin.calculate_density_level(40);
        assert_eq!(high.level, "high");
        assert_eq!(high.density, 0.4);

        let critical = plugin.calculate_density_level(60);
        assert_eq!(critical.level, "critical");
        assert_eq!(critical.density, 0.6);
    }

    #[test]
    fn test_cluster_identification() {
        let plugin = CrowdAnalyticsPlugin::new();

        // Three close people (should cluster) + one far away
        let people = vec![
            BoundingBox {
                x: 100,
                y: 100,
                width: 30,
                height: 60,
            },
            BoundingBox {
                x: 120,
                y: 110,
                width: 30,
                height: 60,
            },
            BoundingBox {
                x: 140,
                y: 100,
                width: 30,
                height: 60,
            },
            BoundingBox {
                x: 500,
                y: 500,
                width: 30,
                height: 60,
            },
        ];

        let clusters = plugin.identify_clusters(&people);
        assert_eq!(clusters.len(), 1); // Only one cluster (3 people close together)
        assert_eq!(clusters[0].count, 3);
    }
}

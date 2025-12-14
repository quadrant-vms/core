/// Integration tests for License Plate Recognition (LPR) plugin
use ai_service::{
    plugin::lpr::LprPlugin,
    plugin::AiPlugin,
};
use common::ai_tasks::VideoFrame;
use base64::Engine;

#[tokio::test]
async fn test_lpr_plugin_initialization() {
    let mut plugin = LprPlugin::new();

    // Test initialization with default config
    let config = serde_json::json!({
        "detection_model_path": "models/lpr_detector_test.onnx",
        "ocr_model_path": null,
        "confidence_threshold": 0.7
    });

    // Since we don't have real models in test environment, this should fail gracefully
    let result = plugin.init(config).await;

    // Expected to fail since model files don't exist in test environment
    assert!(result.is_err());
}

#[test]
fn test_lpr_plugin_metadata() {
    let plugin = LprPlugin::new();

    assert_eq!(plugin.id(), "lpr");
    assert_eq!(plugin.name(), "License Plate Recognition");
    assert_eq!(plugin.version(), "1.0.0");
    assert!(!plugin.description().is_empty());
}

#[test]
fn test_lpr_plugin_config_schema() {
    let plugin = LprPlugin::new();
    let schema = plugin.config_schema();

    assert!(schema.is_some());
    let schema_obj = schema.unwrap();
    assert_eq!(schema_obj["type"], "object");

    // Check required properties exist in schema
    let properties = &schema_obj["properties"];
    assert!(properties["detection_model_path"].is_object());
    assert!(properties["ocr_model_path"].is_object());
    assert!(properties["confidence_threshold"].is_object());
    assert!(properties["char_vocab"].is_object());
}

#[test]
fn test_lpr_plugin_supported_formats() {
    let plugin = LprPlugin::new();
    let formats = plugin.supported_formats();

    assert!(formats.contains(&"jpeg".to_string()));
    assert!(formats.contains(&"png".to_string()));
}

#[test]
fn test_lpr_plugin_gpu_requirement() {
    let plugin = LprPlugin::new();

    // LPR plugin can run on CPU, so GPU is optional
    assert!(!plugin.requires_gpu());
}

#[tokio::test]
async fn test_lpr_plugin_health_check_uninitialized() {
    let plugin = LprPlugin::new();

    // Health check should return Ok(false) when not initialized
    let health = plugin.health_check().await;
    assert!(health.is_ok());
    assert!(!health.unwrap());
}

#[tokio::test]
async fn test_lpr_plugin_process_frame_fails_when_uninitialized() {
    let plugin = LprPlugin::new();

    // Create a dummy frame
    let dummy_image = image::RgbImage::new(640, 480);
    let mut png_data = Vec::new();
    image::DynamicImage::ImageRgb8(dummy_image)
        .write_to(
            &mut std::io::Cursor::new(&mut png_data),
            image::ImageFormat::Png,
        )
        .unwrap();

    let frame = VideoFrame {
        source_id: "test-source".to_string(),
        timestamp: 1234567890,
        sequence: 1,
        data: base64::prelude::BASE64_STANDARD.encode(&png_data),
        format: "png".to_string(),
        width: 640,
        height: 480,
    };

    // Processing should fail when plugin is not initialized
    let result = plugin.process_frame(&frame).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not initialized"));
}

#[tokio::test]
async fn test_lpr_plugin_shutdown() {
    let mut plugin = LprPlugin::new();

    // Shutdown should succeed even if not initialized
    let result = plugin.shutdown().await;
    assert!(result.is_ok());
}

#[test]
fn test_lpr_config_defaults() {
    use ai_service::plugin::lpr::LprConfig;

    let config = LprConfig::default();

    assert_eq!(config.detection_model_path, "models/lpr_detector.onnx");
    assert_eq!(config.ocr_model_path, Some("models/lpr_ocr.onnx".to_string()));
    assert_eq!(config.confidence_threshold, 0.6);
    assert_eq!(config.iou_threshold, 0.4);
    assert_eq!(config.max_detections, 10);
    assert_eq!(config.detection_input_size, 640);
    assert_eq!(config.ocr_input_width, 200);
    assert_eq!(config.ocr_input_height, 64);
    assert!(config.char_vocab.contains("0123456789"));
    assert!(config.char_vocab.contains("ABCDEFGHIJKLMNOPQRSTUVWXYZ"));
    assert_eq!(config.execution_provider, "CUDA");
    assert_eq!(config.device_id, 0);
}

#[test]
fn test_lpr_config_serialization() {
    use ai_service::plugin::lpr::LprConfig;

    let config = LprConfig {
        detection_model_path: "test_detector.onnx".to_string(),
        ocr_model_path: Some("test_ocr.onnx".to_string()),
        confidence_threshold: 0.8,
        iou_threshold: 0.5,
        max_detections: 5,
        detection_input_size: 416,
        ocr_input_width: 150,
        ocr_input_height: 48,
        char_vocab: "0123456789".to_string(),
        execution_provider: "CPU".to_string(),
        device_id: 1,
        intra_threads: 2,
        inter_threads: 1,
    };

    // Test serialization
    let json = serde_json::to_value(&config).unwrap();
    assert_eq!(json["detection_model_path"], "test_detector.onnx");
    assert_eq!(json["confidence_threshold"], 0.8);

    // Test deserialization
    let deserialized: LprConfig = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized.detection_model_path, "test_detector.onnx");
    assert_eq!(deserialized.confidence_threshold, 0.8);
}

/// Test that the plugin info is correctly constructed
#[test]
fn test_lpr_plugin_info() {
    let plugin = LprPlugin::new();
    let info = plugin.info();

    assert_eq!(info.id, "lpr");
    assert_eq!(info.name, "License Plate Recognition");
    assert_eq!(info.version, "1.0.0");
    assert!(info.supported_formats.contains(&"jpeg".to_string()));
    assert!(info.supported_formats.contains(&"png".to_string()));
    assert!(!info.requires_gpu);
    assert!(info.config_schema.is_some());
}

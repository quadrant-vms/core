/// Integration tests for anomaly detection plugin
use ai_service::{
    plugin::anomaly_detection::AnomalyDetectorPlugin, plugin::registry::PluginRegistry,
    plugin::AiPlugin,
};
use common::ai_tasks::VideoFrame;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Helper function to create a test plugin with configuration
async fn setup_anomaly_detector(config: serde_json::Value) -> Arc<RwLock<AnomalyDetectorPlugin>> {
    let mut plugin = AnomalyDetectorPlugin::new();
    plugin.init(config).await.unwrap();
    Arc::new(RwLock::new(plugin))
}

#[tokio::test]
async fn test_anomaly_detector_registration() {
    let registry = PluginRegistry::new();
    let plugin = setup_anomaly_detector(serde_json::Value::Null).await;

    registry.register(plugin).await.unwrap();
    assert_eq!(registry.count().await, 1);
    assert!(registry.has_plugin("anomaly_detector").await);

    let plugin_info = registry
        .get("anomaly_detector")
        .await
        .unwrap()
        .read()
        .await
        .info();

    assert_eq!(plugin_info.id, "anomaly_detector");
    assert_eq!(plugin_info.name, "Anomaly Detector");
    assert!(!plugin_info.requires_gpu);
}

#[tokio::test]
async fn test_spatial_anomaly_detection_with_zones() {
    let config = serde_json::json!({
        "enable_spatial": true,
        "enable_temporal": false,
        "zones": [
            {
                "id": "restricted_zone_1",
                "name": "Secure Area",
                "bbox": {"x": 500, "y": 500, "width": 400, "height": 400},
                "restricted_classes": ["person", "vehicle"]
            },
            {
                "id": "parking_zone",
                "name": "Parking Lot",
                "bbox": {"x": 100, "y": 100, "width": 200, "height": 200},
                "allowed_classes": ["vehicle"],
            }
        ],
        "confidence_threshold": 0.5
    });

    let plugin = setup_anomaly_detector(config).await;

    // Create a frame with detections in restricted areas
    let frame = VideoFrame {
        source_id: "camera-1".to_string(),
        timestamp: 1234567890,
        sequence: 1,
        width: 1920,
        height: 1080,
        format: "jpeg".to_string(),
        data: serde_json::json!({
            "detections": [
                {
                    "class": "person",
                    "confidence": 0.95,
                    "bbox": {"x": 650, "y": 650, "width": 50, "height": 100},
                    "metadata": null
                },
                {
                    "class": "person",
                    "confidence": 0.9,
                    "bbox": {"x": 150, "y": 150, "width": 40, "height": 80},
                    "metadata": null
                },
                {
                    "class": "vehicle",
                    "confidence": 0.92,
                    "bbox": {"x": 700, "y": 700, "width": 100, "height": 80},
                    "metadata": null
                }
            ]
        })
        .to_string(),
    };

    let result = plugin.read().await.process_frame(&frame).await.unwrap();

    // Should detect 3 anomalies:
    // 1. Person in restricted zone 1
    // 2. Person in parking zone (only vehicles allowed)
    // 3. Vehicle in restricted zone 1
    assert_eq!(result.detections.len(), 3);
    assert!(result
        .detections
        .iter()
        .any(|d| d.class == "spatial_anomaly_person"));
    assert!(result
        .detections
        .iter()
        .any(|d| d.class == "spatial_anomaly_vehicle"));

    // Verify metadata
    let first_anomaly = &result.detections[0];
    let metadata = first_anomaly.metadata.as_ref().unwrap();
    assert_eq!(metadata["anomaly_type"], "spatial");
    assert_eq!(metadata["reason"], "restricted_object_in_zone");
}

#[tokio::test]
async fn test_temporal_anomaly_detection() {
    let config = serde_json::json!({
        "enable_spatial": false,
        "enable_temporal": true,
        "sensitivity": 2.0,
        "min_samples": 5,
        "history_size": 50,
        "tracked_classes": ["person"],
        "confidence_threshold": 0.6
    });

    let plugin = setup_anomaly_detector(config).await;

    // Feed 10 normal frames (1-2 persons each)
    for i in 0..10 {
        let frame = VideoFrame {
            source_id: "camera-1".to_string(),
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
                    },
                    {
                        "class": "person",
                        "confidence": 0.85,
                        "bbox": {"x": 200, "y": 200, "width": 50, "height": 100},
                        "metadata": null
                    }
                ]
            })
            .to_string(),
        };

        plugin.read().await.process_frame(&frame).await.unwrap();
    }

    // Feed an anomalous frame (20 persons - sudden crowd)
    let anomalous_frame = VideoFrame {
        source_id: "camera-1".to_string(),
        timestamp: 1234567890 + 11000,
        sequence: 11,
        width: 1920,
        height: 1080,
        format: "jpeg".to_string(),
        data: serde_json::json!({
            "detections": (0..20)
                .map(|i| serde_json::json!({
                    "class": "person",
                    "confidence": 0.9,
                    "bbox": {
                        "x": 100 + i * 50,
                        "y": 100 + i * 30,
                        "width": 50,
                        "height": 100
                    },
                    "metadata": null
                }))
                .collect::<Vec<_>>()
        })
        .to_string(),
    };

    let result = plugin
        .read()
        .await
        .process_frame(&anomalous_frame)
        .await
        .unwrap();

    // Should detect temporal anomaly for person count
    assert!(!result.detections.is_empty());

    let has_person_anomaly = result
        .detections
        .iter()
        .any(|d| d.class == "temporal_anomaly_person");
    let has_total_anomaly = result
        .detections
        .iter()
        .any(|d| d.class == "temporal_anomaly_total_count");

    assert!(
        has_person_anomaly || has_total_anomaly,
        "Should detect temporal anomaly for unusual person count"
    );

    // Verify metadata
    if let Some(anomaly) = result.detections.first() {
        let metadata = anomaly.metadata.as_ref().unwrap();
        assert_eq!(metadata["anomaly_type"], "temporal");
        assert!(metadata["current_value"].as_u64().unwrap() > 10);
    }
}

#[tokio::test]
async fn test_combined_spatial_and_temporal_anomalies() {
    let config = serde_json::json!({
        "enable_spatial": true,
        "enable_temporal": true,
        "sensitivity": 2.5,
        "min_samples": 3,
        "zones": [
            {
                "id": "zone1",
                "name": "Restricted",
                "bbox": {"x": 800, "y": 800, "width": 200, "height": 200},
                "restricted_classes": ["person"]
            }
        ]
    });

    let plugin = setup_anomaly_detector(config).await;

    // Feed a few normal frames
    for i in 0..5 {
        let frame = VideoFrame {
            source_id: "camera-1".to_string(),
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
            })
            .to_string(),
        };

        plugin.read().await.process_frame(&frame).await.unwrap();
    }

    // Feed anomalous frame with both spatial and temporal anomalies
    let anomalous_frame = VideoFrame {
        source_id: "camera-1".to_string(),
        timestamp: 1234567890 + 6000,
        sequence: 6,
        width: 1920,
        height: 1080,
        format: "jpeg".to_string(),
        data: serde_json::json!({
            "detections": [
                // Person in restricted zone (spatial anomaly)
                {
                    "class": "person",
                    "confidence": 0.95,
                    "bbox": {"x": 850, "y": 850, "width": 50, "height": 100},
                    "metadata": null
                },
                // Many additional persons (temporal anomaly)
                {
                    "class": "person",
                    "confidence": 0.9,
                    "bbox": {"x": 200, "y": 200, "width": 50, "height": 100},
                    "metadata": null
                },
                {
                    "class": "person",
                    "confidence": 0.9,
                    "bbox": {"x": 300, "y": 300, "width": 50, "height": 100},
                    "metadata": null
                },
                {
                    "class": "person",
                    "confidence": 0.9,
                    "bbox": {"x": 400, "y": 400, "width": 50, "height": 100},
                    "metadata": null
                },
                {
                    "class": "person",
                    "confidence": 0.9,
                    "bbox": {"x": 500, "y": 500, "width": 50, "height": 100},
                    "metadata": null
                },
                {
                    "class": "person",
                    "confidence": 0.9,
                    "bbox": {"x": 600, "y": 600, "width": 50, "height": 100},
                    "metadata": null
                }
            ]
        })
        .to_string(),
    };

    let result = plugin
        .read()
        .await
        .process_frame(&anomalous_frame)
        .await
        .unwrap();

    // Should detect both types of anomalies
    assert!(result.detections.len() >= 2);

    let has_spatial = result
        .detections
        .iter()
        .any(|d| d.class.starts_with("spatial_anomaly"));
    let has_temporal = result
        .detections
        .iter()
        .any(|d| d.class.starts_with("temporal_anomaly"));

    assert!(has_spatial, "Should detect spatial anomaly");
    assert!(has_temporal, "Should detect temporal anomaly");
}

#[tokio::test]
async fn test_no_anomalies_on_normal_activity() {
    let config = serde_json::json!({
        "enable_spatial": true,
        "enable_temporal": true,
        "sensitivity": 2.0,
        "min_samples": 5,
        "zones": [
            {
                "id": "zone1",
                "name": "Allowed Area",
                "bbox": {"x": 100, "y": 100, "width": 200, "height": 200},
                "allowed_classes": ["person", "vehicle"]
            }
        ]
    });

    let plugin = setup_anomaly_detector(config).await;

    // Feed consistent normal frames
    for i in 0..10 {
        let frame = VideoFrame {
            source_id: "camera-1".to_string(),
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
                        "bbox": {"x": 150, "y": 150, "width": 50, "height": 100},
                        "metadata": null
                    },
                    {
                        "class": "vehicle",
                        "confidence": 0.85,
                        "bbox": {"x": 180, "y": 180, "width": 80, "height": 60},
                        "metadata": null
                    }
                ]
            })
            .to_string(),
        };

        let result = plugin.read().await.process_frame(&frame).await.unwrap();

        // Should not detect any anomalies
        assert_eq!(result.detections.len(), 0);
    }
}

#[tokio::test]
async fn test_plugin_health_check() {
    let plugin = setup_anomaly_detector(serde_json::Value::Null).await;
    let is_healthy = plugin.read().await.health_check().await.unwrap();
    assert!(is_healthy);
}

#[tokio::test]
async fn test_plugin_shutdown() {
    let plugin = setup_anomaly_detector(serde_json::Value::Null).await;
    let result = plugin.write().await.shutdown().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_config_schema_validation() {
    let plugin = AnomalyDetectorPlugin::new();
    let schema = plugin.config_schema();

    assert!(schema.is_some());
    let schema_value = schema.unwrap();

    // Verify schema structure
    assert_eq!(schema_value["type"], "object");
    assert!(schema_value["properties"].is_object());
    assert!(schema_value["properties"]["enable_temporal"].is_object());
    assert!(schema_value["properties"]["enable_spatial"].is_object());
    assert!(schema_value["properties"]["sensitivity"].is_object());
    assert!(schema_value["properties"]["zones"].is_object());
}

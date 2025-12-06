# Quadrant VMS (Rust)

> ⚙️ A Video Management System (VMS) built in **Rust**.  
> The project aims to be **modular**, **cluster-ready**, and **AI-model friendly**.

---

## 🚧 Development Status
This project is **under active development**.

### ✅ Implemented
- `stream-node`: RTSP ingest → HLS (TS/fMP4) with S3 upload fallback.
- `coordinator`: lease-based scheduler with REST API
  - **Persistent lease store** with PostgreSQL backend
  - In-memory backend option for development/testing
  - Configurable via `LEASE_STORE_TYPE` environment variable (`memory` or `postgres`)
  - Automatic database migrations with sqlx
  - Atomic lease operations with PostgreSQL transactions
  - Efficient lease expiration and cleanup
- `admin-gateway`: REST facade that acquires leases, launches `stream-node`, and stops streams via worker HTTP calls.
- `recorder-node`: Complete recording pipeline implementation with:
  - FFmpeg-based recording from RTSP and HLS sources
  - Multi-format output support (MP4, HLS, MKV)
  - Automatic metadata extraction (duration, resolution, codecs, bitrate, fps)
  - Recording job lifecycle management with REST API
  - Storage path tracking and S3 integration
  - **Coordinator lease integration** for distributed recording management
  - Automatic lease acquisition, renewal (50% TTL interval), and release
  - Lease failure detection with error state transitions
  - Configurable via `COORDINATOR_URL` and `NODE_ID` environment variables
- `admin-gateway`: Recorder worker management integration
  - REST API for recorder operations (`/v1/recordings`)
  - Lease-based recorder resource management via coordinator
  - Automatic lease acquisition and renewal for recordings
  - Recording lifecycle orchestration (start/stop/list)
  - Worker health monitoring and error handling
- **Failover hardening and resilience features**:
  - Worker health verification: admin-gateway checks worker liveness during lease renewal
  - Automatic retry with exponential backoff: lease renewals retry up to 3 times before marking stream as failed
  - Enhanced health endpoints: `/readyz` endpoint verifies lease store connectivity
  - Graceful degradation: temporary coordinator unavailability doesn't immediately kill active streams
  - Comprehensive error handling with detailed error messages and logging
- **Multi-coordinator clustering with leader election**:
  - Leader election using Raft-inspired consensus algorithm
  - Automatic failover and re-election on leader failure
  - Heartbeat-based health monitoring between coordinator nodes
  - Randomized election timeouts to prevent split votes
  - Cluster status API endpoint (`/cluster/status`)
  - Configurable via environment variables (`CLUSTER_ENABLED`, `NODE_ID`, `CLUSTER_PEERS`)
  - Support for single-node and multi-node cluster deployments
  - Integration tests validating leader election in 1-node and 3-node clusters
    - **Request forwarding from followers to leader**: Follower coordinators automatically forward write operations (acquire/renew/release) to the elected leader, enabling clients to connect to any coordinator node without tracking leader status
- **Advanced Metrics and Observability**:
  - Prometheus metrics integration across all services
  - Comprehensive metric collection for coordinators, stream-nodes, recorder-nodes, admin-gateway, and ai-service
  - `/metrics` endpoint on all services for Prometheus scraping
  - Coordinator metrics: active leases, lease operations, cluster nodes, leader elections, forwarded requests
  - Stream-node metrics: active streams, HLS segments, S3 uploads, bytes processed
  - Recorder-node metrics: active recordings, recording operations, bytes recorded, completion status
  - Admin-gateway metrics: HTTP requests, request duration, active workers, worker operations
  - AI-service metrics: active tasks, frames processed, detections made, detection latency, plugin health
  - Centralized metrics registry in telemetry crate
- **AI Model Plugin Architecture**:
  - `ai-service`: Modular AI plugin system with extensible architecture
  - Plugin trait interface for custom AI model integrations
  - Plugin registry with dynamic plugin registration and management
  - Built-in mock object detection plugin for testing and demonstration
  - **YOLOv8 object detection plugin**:
    - Real-time object detection using YOLOv8 ONNX models
    - Support for all YOLOv8 model variants (nano, small, medium, large, extra-large)
    - CPU and GPU inference support via ONNX Runtime
    - Non-Maximum Suppression (NMS) for overlapping box filtering
    - Configurable confidence and IoU thresholds
    - 80 COCO classes detection (person, car, dog, etc.)
    - Automatic scaling to original image dimensions
    - Environment variable configuration (`YOLOV8_MODEL_PATH`, `YOLOV8_CONFIDENCE`)
  - REST API for AI task lifecycle management (`/v1/tasks`)
  - Coordinator lease integration for distributed AI task management
  - Automatic lease acquisition, renewal, and release for AI tasks
  - Support for multiple output formats (webhook, MQTT, RabbitMQ, local file)
  - Frame-based processing with configurable sampling rates
  - Comprehensive metrics for AI operations (tasks, frames, detections, latency)
  - Health check endpoints for plugin monitoring
  - Configurable via `AI_SERVICE_ADDR`, `COORDINATOR_URL`, and `NODE_ID` environment variables
  - Standalone or coordinator-integrated deployment modes
- CI-friendly test suite (`cargo test`) covering lease store logic, router contracts, recording lifecycle, pipeline configuration, recorder-coordinator integration, cluster leader election, metrics collection, AI plugin system, and end-to-end gateway↔coordinator↔worker↔recorder↔ai-service flows.

### 🔜 In Progress
- Operator UI & rule system
- Additional AI model integrations (pose estimation, facial recognition)
- Frame capture pipeline from stream-node/recorder-node to AI service
- GPU acceleration optimization for YOLOv8

---

## 💡 Follow Progress
Each milestone (camera compatibility, failover tests, AI plugin, etc.)
will unlock sequentially as community funding goals are reached.

Stay tuned.

---
© 2025 Quadrant Intelligence Studio

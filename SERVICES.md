# Quadrant VMS - Service Details

This document provides detailed information about each service in the Quadrant VMS architecture.

---

## Core Services

### `coordinator` - Lease-Based Job Scheduler
**Location**: `crates/coordinator/`
**Entry Point**: `crates/coordinator/src/main.rs`

#### Features
- Lease-based distributed job scheduling
- REST API for lease management (acquire/renew/release)
- Multiple backend options:
  - **PostgreSQL** (production): Persistent lease storage with atomic operations
  - **In-memory** (development): Fast, ephemeral storage for testing
- Automatic database migrations with sqlx
- Atomic lease operations with PostgreSQL transactions
- Efficient lease expiration and cleanup
- **Multi-coordinator clustering** with leader election:
  - Raft-inspired consensus algorithm
  - Automatic failover and re-election on leader failure
  - Heartbeat-based health monitoring between nodes
  - Randomized election timeouts to prevent split votes
  - Request forwarding from followers to leader
  - Cluster status API (`/cluster/status`)
- **StateStore HTTP API** for persisting stream/recording/AI task state

#### Configuration
- `LEASE_STORE_TYPE`: `memory` or `postgres`
- `DATABASE_URL`: PostgreSQL connection string (when using postgres backend)
- `CLUSTER_ENABLED`: Enable multi-node clustering
- `NODE_ID`: Unique node identifier for clustering
- `CLUSTER_PEERS`: Comma-separated list of peer coordinator URLs
- `ENABLE_STATE_STORE`: Enable state persistence (default: false)
- `ORPHAN_CLEANUP_INTERVAL_SECS`: Cleanup interval for orphaned resources (default: 300)

#### Metrics
- Active leases
- Lease operations (acquire/renew/release)
- Cluster nodes and leader elections
- Forwarded requests

---

### `admin-gateway` - REST API Facade
**Location**: `crates/admin-gateway/`
**Entry Point**: `crates/admin-gateway/src/main.rs`

#### Features
- REST API facade for all VMS operations
- Acquires leases from coordinator for distributed resource management
- Launches and manages worker lifecycle:
  - **stream-node**: Video streaming workers
  - **recorder-node**: Recording workers
- Worker health monitoring and error handling
- Automatic retry with exponential backoff (3 retries for lease renewals)
- Graceful degradation during temporary coordinator unavailability
- **StateStore integration**: Automatic state persistence on all state changes
- **Bootstrap logic**: Restore state from StateStore on startup
- **Automated orphan cleanup**: Detect and remove orphaned resources

#### API Endpoints
- `/v1/streams` - Stream management
- `/v1/recordings` - Recording management
- `/readyz` - Readiness check with lease store connectivity verification

#### Configuration
- `ADMIN_GATEWAY_ADDR`: Bind address (default: 127.0.0.1:8080)
- `COORDINATOR_URL`: Coordinator service URL
- `ENABLE_STATE_STORE`: Enable state persistence

#### Metrics
- HTTP requests and duration
- Active workers
- Worker operations

---

### `stream-node` - RTSP to HLS Transcoding
**Location**: `crates/stream-node/`
**Entry Point**: `crates/stream-node/src/main.rs`

#### Features
- RTSP video stream ingestion
- HLS transcoding with multiple format support:
  - **TS** (MPEG-TS): Traditional HLS format
  - **fMP4** (fragmented MP4): Modern HLS format
- S3 storage upload with automatic fallback
- **AI integration**: Periodic frame capture for live AI processing
  - Configurable capture intervals, resolution, and quality
  - Automatic frame submission to ai-service
  - Task management with lease coordination
  - Clean lifecycle management with cancellation tokens

#### Configuration
- `HLS_ROOT`: HLS output directory (default: ./data/hls)
- `S3_ENDPOINT`: S3-compatible storage endpoint (optional)
- `AI_SERVICE_ADDR`: AI service URL for frame processing (optional)

#### Metrics
- Active streams
- HLS segments generated
- S3 uploads (success/failure)
- Bytes processed

---

### `recorder-node` - Video Recording Pipeline
**Location**: `crates/recorder-node/`
**Entry Point**: `crates/recorder-node/src/main.rs`

#### Features
- FFmpeg-based recording pipeline
- Multi-source support:
  - **RTSP**: Direct camera streams
  - **HLS**: HTTP Live Streaming sources
- Multi-format output:
  - **MP4**: Standard video format
  - **HLS**: Segmented streaming format
  - **MKV**: Matroska container
- Automatic metadata extraction (duration, resolution, codecs, bitrate, fps)
- Recording job lifecycle management
- Storage path tracking and S3 integration
- **Coordinator lease integration**: Distributed recording management with automatic lease renewal
- **AI integration**: Frame capture from active recordings with configurable intervals
- **Thumbnail generation**: Extract preview images from recordings
  - Single thumbnail at specific timestamp
  - Thumbnail grid for timeline preview
  - Configurable resolution and JPEG quality
- **Storage & retention management**: Automated recording lifecycle
  - Time-based retention policies
  - Storage quota enforcement
  - Tiered storage support (hot/cold storage)
  - Dry-run mode for testing
  - Complete audit trail
- **Search & indexing**: Fast searchable index of recordings and events
  - Full-text search with PostgreSQL tsvector
  - Advanced filtering (device, zone, time-range, duration, tags)
  - Object-based search (find recordings with specific detections)
  - Automatic indexing with triggers

#### REST API
- `/v1/recordings` - Recording CRUD operations
- `/thumbnail` - Single thumbnail generation
- `/thumbnail/grid` - Thumbnail grid generation
- `/v1/retention/policies` - Retention policy management
- `/v1/storage/stats` - Storage statistics
- `/v1/search/recordings` - Search recordings
- `/v1/search/events` - Search events
- `/v1/search/objects` - Search by object type

#### Configuration
- `RECORDER_NODE_ADDR`: Bind address (default: 127.0.0.1:8081)
- `COORDINATOR_URL`: Coordinator service URL
- `NODE_ID`: Unique node identifier
- `AI_SERVICE_ADDR`: AI service URL for frame processing (optional)
- `RECORDING_STORAGE_ROOT`: Recording file storage (default: ./data/recordings)
- `DATABASE_URL`: PostgreSQL connection string (for retention/search features)

#### Metrics
- Active recordings
- Recording operations (start/stop)
- Bytes recorded
- Completion status (success/failure)

---

## AI & Intelligence Services

### `ai-service` - AI Model Plugin System
**Location**: `crates/ai-service/`
**Entry Point**: `crates/ai-service/src/main.rs`

#### Features
- **Modular plugin architecture**: Extensible AI model integration system
- **Plugin registry**: Dynamic plugin registration and management
- **Built-in plugins**:
  - **Mock object detection**: Testing and demonstration
  - **YOLOv8 object detection**: Real-time object detection
    - All YOLOv8 variants (nano/small/medium/large/extra-large)
    - 80 COCO classes (person, car, dog, etc.)
    - CPU and GPU inference (ONNX Runtime)
    - Non-Maximum Suppression (NMS)
    - Configurable confidence/IoU thresholds
  - **Pose estimation**: Human pose detection
    - COCO 17 keypoint format
    - Multiple pose detection per frame
    - Support for MoveNet, MediaPipe Pose, or similar ONNX models
    - Keypoint confidence tracking
    - Person ID tracking
  - **License Plate Recognition (LPR)**: Automated plate detection and OCR
    - Two-stage pipeline: detection + OCR
    - YOLOv8-based plate detection
    - CRNN/LSTM-based OCR for text recognition
    - Configurable character vocabulary (digits, letters, symbols)
    - CTC (Connectionist Temporal Classification) decoding
    - Multi-plate detection per frame
    - CPU and GPU inference with automatic fallback
    - Configurable confidence/IoU thresholds
  - **Facial Recognition**: Face detection, embedding extraction, and database matching
    - Two-stage pipeline: detection + embedding
    - RetinaFace/SCRFD-based face detection
    - ArcFace/FaceNet-based face embedding extraction (512-D vectors)
    - In-memory face database with L2-normalized embeddings
    - Cosine similarity-based face matching
    - Face enrollment, removal, and listing via REST API
    - Multi-face detection per frame
    - CPU and GPU inference with automatic fallback
    - Configurable similarity threshold for matching
  - **Action Recognition**: Temporal video analysis for human action detection
    - Detects 20+ action classes: walking, running, sitting, standing, waving, jumping, clapping, pointing, talking, phone_call, drinking, eating, reading, writing, pushing, pulling, carrying, throwing, catching, falling
    - Temporal frame buffering (configurable window, default 16 frames)
    - Sequence-based inference for temporal coherence
    - Multi-action detection per sequence with confidence scores
    - CPU and GPU inference with automatic fallback
    - Configurable confidence threshold (default: 0.6)
    - Supports standard action recognition ONNX models (e.g., SlowFast, TimeSformer, X3D)
  - **Crowd Analytics**: Person counting, density analysis, and hotspot detection
    - YOLOv8-based person detection
    - Real-time person counting with confidence tracking
    - Grid-based density heatmap (configurable NxN grid, default 10x10)
    - Crowd density levels: low/medium/high/critical (people per square meter)
    - Distance-based clustering for hotspot identification
    - Spatial distribution analysis with bounding box visualization
    - Configurable coverage area for density calculations (default: 100m²)
    - Configurable cluster size threshold (default: 3 people minimum)
    - CPU and GPU inference with automatic fallback
    - Ideal for public safety, retail analytics, and crowd management
- **Frame capture pipeline**: FFmpeg-based frame extraction with REST API
- **GPU acceleration optimization**:
  - CUDA and TensorRT execution providers
  - Automatic fallback (TensorRT → CUDA → CPU)
  - Multi-GPU support with device selection
  - Configurable thread pools and memory limits
  - Performance monitoring
- **Coordinator lease integration**: Distributed AI task management
- **Multi-output support**: Webhook, MQTT, RabbitMQ, local file
- **Comprehensive metrics**: Tasks, frames, detections, latency, plugin health

#### REST API
- `/v1/tasks` - AI task lifecycle management
- `/v1/tasks/:id/frames` - Frame submission for processing
- `/v1/faces` - Face enrollment and listing
  - `POST /v1/faces` - Enroll a new face with base64 image
  - `GET /v1/faces` - List all enrolled faces
  - `DELETE /v1/faces/:id` - Remove a face from database
- `/health` - Health check with plugin status

#### Configuration
- `AI_SERVICE_ADDR`: Bind address (default: 127.0.0.1:8088)
- `COORDINATOR_URL`: Coordinator service URL
- `NODE_ID`: Unique node identifier
- **YOLOv8 Object Detection**:
  - `YOLOV8_MODEL_PATH`: YOLOv8 ONNX model file path
  - `YOLOV8_CONFIDENCE`: Detection confidence threshold (default: 0.5)
  - `YOLOV8_EXECUTION_PROVIDER`: CPU/CUDA/TensorRT (default: CUDA)
  - `YOLOV8_DEVICE_ID`: GPU device ID (default: 0)
- **Pose Estimation**:
  - `POSE_MODEL_PATH`: Pose estimation ONNX model file path
  - `POSE_CONFIDENCE`: Pose confidence threshold (default: 0.5)
  - `POSE_KEYPOINT_CONFIDENCE`: Keypoint confidence threshold (default: 0.3)
- **License Plate Recognition (LPR)**:
  - `LPR_DETECTION_MODEL`: LPR detection ONNX model file path
  - `LPR_OCR_MODEL`: LPR OCR ONNX model file path (optional, detection-only if not set)
  - `LPR_CONFIDENCE`: Plate detection confidence threshold (default: 0.6)
  - `LPR_EXECUTION_PROVIDER`: CPU/CUDA/TensorRT (default: CUDA)
  - `LPR_DEVICE_ID`: GPU device ID (default: 0)
- **Facial Recognition**:
  - `FACE_DETECTION_MODEL`: Face detection ONNX model file path
  - `FACE_EMBEDDING_MODEL`: Face embedding ONNX model file path (optional, detection-only if not set)
  - `FACE_CONFIDENCE`: Face detection confidence threshold (default: 0.6)
  - `FACE_SIMILARITY_THRESHOLD`: Cosine similarity threshold for face matching (default: 0.5)
  - `FACE_RECOGNITION_EXECUTION_PROVIDER`: CPU/CUDA/TensorRT (default: CUDA)
  - `FACE_RECOGNITION_DEVICE_ID`: GPU device ID (default: 0)
- **Action Recognition**:
  - `ACTION_RECOGNITION_MODEL`: Action recognition ONNX model file path
  - `ACTION_CONFIDENCE`: Action detection confidence threshold (default: 0.6)
  - `ACTION_TEMPORAL_WINDOW`: Number of frames to buffer for temporal analysis (default: 16)
  - `ACTION_EXECUTION_PROVIDER`: CPU/CUDA/TensorRT (default: CUDA)
  - `ACTION_DEVICE_ID`: GPU device ID (default: 0)
- **Crowd Analytics**:
  - `CROWD_ANALYTICS_MODEL`: YOLOv8 ONNX model file path for person detection (default: models/yolov8n.onnx)
  - `CROWD_CONFIDENCE`: Person detection confidence threshold (default: 0.5)
  - `CROWD_GRID_SIZE`: Density heatmap grid size (NxN, default: 10)
  - `CROWD_COVERAGE_AREA`: Camera coverage area in square meters (default: 100.0)
  - `CROWD_MIN_CLUSTER_SIZE`: Minimum people count for hotspot detection (default: 3)
  - `CROWD_CLUSTER_DISTANCE`: Distance threshold for clustering in pixels (default: 100.0)
  - `CROWD_EXECUTION_PROVIDER`: CPU/CUDA/TensorRT (default: CUDA)
  - `CROWD_DEVICE_ID`: GPU device ID (default: 0)

#### Metrics
- Active AI tasks
- Frames processed
- Detections made
- Detection latency
- Plugin health status

---

## Security & Access Control

### `auth-service` - Authentication & Authorization
**Location**: `crates/auth-service/`
**Entry Point**: `crates/auth-service/src/main.rs`

#### Features
- **User management**: CRUD operations for users
- **JWT-based authentication**: Secure token-based API access
- **API tokens**: Long-lived tokens for service-to-service authentication
- **Role-Based Access Control (RBAC)**: Fine-grained permission system
  - Resource-based permissions (stream, recording, ai_task, device, user, role, tenant, audit)
  - Action-based controls (read, create, update, delete)
  - Built-in roles: System Administrator, Operator, Viewer
  - Custom role creation and management
  - Permission inheritance through roles
- **Multi-tenancy support**: Isolated tenant environments
  - Separate users and roles per tenant
  - Resource quotas (max_users, max_streams, max_recordings, max_ai_tasks)
  - Tenant-scoped audit logs
- **OIDC/OAuth2 SSO Integration**: Single Sign-On with external identity providers
  - Google Workspace / Google Identity
  - Microsoft Azure AD / Entra ID
  - Keycloak
  - Generic custom OIDC providers
  - Automatic user provisioning on first SSO login
  - OIDC identity linking and management
  - CSRF-protected authorization flow
- **Security features**:
  - Argon2 password hashing
  - Secure API token generation
  - Token expiration and revocation
  - Last login tracking
  - Audit log with IP address and user agent tracking
- **Audit logging**: Complete security audit trail for compliance

#### REST API
- `/v1/users` - User management
- `/v1/roles` - Role management
- `/v1/permissions` - Permission management
- `/v1/tenants` - Tenant management
- `/v1/auth/login` - JWT authentication
- `/v1/auth/tokens` - API token management
- `/v1/audit` - Audit log access
- `/v1/auth/oidc` - OIDC SSO endpoints

#### Default Setup
- **System tenant**: Pre-configured default tenant
- **Default admin user**: username: `admin`, password: `admin123` (CHANGE IN PRODUCTION!)
- **Built-in roles**: system-admin, operator, viewer
- **29 default permissions**: Covering all resources including device management

#### Configuration
- `DATABASE_URL`: PostgreSQL connection string
- `JWT_SECRET`: Secret key for JWT signing (CHANGE IN PRODUCTION!)
- `JWT_EXPIRATION_SECS`: Token expiration time (default: 3600)
- `AUTH_SERVICE_ADDR`: Bind address (default: 127.0.0.1:8083)

**Documentation**: See [docs/AUTHENTICATION.md](docs/AUTHENTICATION.md) for complete guide

---

## Device Management

### `device-manager` - Camera & Device Management
**Location**: `crates/device-manager/`
**Entry Point**: `crates/device-manager/src/main.rs`

#### Features
- **Device onboarding and registration**: Add cameras, NVRs, encoders
- **RTSP device probing**: Automatic capability detection
  - Video/audio codec detection
  - Resolution discovery
  - Metadata extraction (manufacturer, model)
- **Multi-protocol support**: RTSP, ONVIF, HTTP, RTMP, WebRTC
- **Device categorization**: Types, zones, tags
- **Health monitoring system**:
  - Automated periodic health checks
  - Configurable check intervals per device
  - Status tracking (online, offline, error, maintenance, provisioning)
  - Health history with timestamps and response times
  - Consecutive failure tracking
  - Automatic status transitions
- **PTZ control system**:
  - Pan, tilt, zoom, absolute/relative positioning
  - Home position support
  - Preset management (create/update/delete/navigate)
  - Tour system for automated patrol
  - Tour execution engine with background worker
  - ONVIF integration with SOAP-based communication
  - Mock client for testing
- **ONVIF device discovery**:
  - WS-Discovery protocol for network scanning
  - UDP multicast probe for automatic device detection
  - Device metadata extraction from ONVIF scopes
  - Asynchronous scanning with non-blocking API
  - Device import workflow
- **Camera configuration push**:
  - ONVIF imaging service integration
  - Video encoder configuration (codec, resolution, framerate, bitrate)
  - Image settings (brightness, contrast, saturation, sharpness)
  - Advanced features (IR mode, WDR control)
  - Audio and network configuration
  - Configuration history tracking
- **Firmware update management**:
  - Firmware file catalog with versioning
  - Upload and SHA-256 checksum validation
  - ONVIF firmware upgrades
  - Update progress tracking
  - History and audit trail
  - Retry mechanism with exponential backoff
  - Rollback support
- **Batch operations**: Update multiple devices simultaneously
- **PostgreSQL-backed storage**: Persistent device state, health history, events
- **Device event audit trail**: Tracks all device state changes
- **Secure credential management**: Encrypted storage of device passwords

#### REST API
- `/v1/devices` - Device CRUD operations
- `/v1/devices/:id/health` - Device health status and history
- `/v1/devices/:id/probe` - On-demand device probing
- `/v1/devices/:id/ptz` - PTZ control endpoints
- `/v1/devices/:id/ptz/presets` - PTZ preset management
- `/v1/devices/:id/ptz/tours` - PTZ tour management
- `/v1/devices/:id/ptz/tours/:id/start` - Start tour execution
- `/v1/discovery/scan` - ONVIF device discovery
- `/v1/devices/:id/configuration` - Camera configuration push
- `/v1/firmware` - Firmware management endpoints

#### Configuration
- `DATABASE_URL`: PostgreSQL connection string
- `DEVICE_MANAGER_ADDR`: Bind address (default: 127.0.0.1:8084)
- `PROBE_TIMEOUT_SECS`: Device probe timeout (default: 10)
- `HEALTH_CHECK_INTERVAL_SECS`: Global health check interval (default: 30)
- `MAX_CONSECUTIVE_FAILURES`: Failures before marking as error (default: 3)
- `PTZ_TIMEOUT_SECS`: PTZ command timeout (default: 10)
- `DISCOVERY_TIMEOUT_SECS`: Discovery scan timeout (default: 5)
- `FIRMWARE_STORAGE_ROOT`: Firmware file storage (default: ./data/firmware)

---

## Event & Notification System

### `alert-service` - Alert & Automation
**Location**: `crates/alert-service/`
**Entry Point**: `crates/alert-service/src/main.rs`

#### Features
- **Rule engine**: Flexible condition-based alert triggering
  - Multiple trigger types: device offline/online, motion detected, AI detections, recording/stream failures, health check failures, custom events
  - JSON-based condition matching with operator support (>, >=, <, <=, ==, !=)
  - Wildcard pattern matching for string fields
  - Multi-tenant alert rule isolation
- **Alert suppression and rate limiting**:
  - Configurable cooldown periods (suppress_duration_secs)
  - Rate limiting (max_alerts_per_hour)
  - Automatic suppression state management
  - Suppression reason tracking
- **Scheduling**: Cron-based time windows for when rules are active
- **Multi-channel notifications**:
  - **Email**: SMTP-based email with template support
  - **Webhook**: HTTP/HTTPS delivery with custom headers and templates
  - **MQTT**: MQTT broker integration with QoS support
  - **Slack**: Rich-formatted Slack messages via webhooks with severity color coding
  - **Discord**: Discord webhook integration with embed support and color coding
  - **SMS**: Twilio-based SMS notifications with template support
- **Alert history and auditing**:
  - Complete event history with context data
  - Notification delivery tracking (sent/failed counts)
  - Retry mechanisms with failure tracking

#### REST API
- `/v1/rules` - Alert rule management
- `/v1/actions` - Notification action management
- `/v1/events` - Alert event history
- `/v1/trigger` - Manual event triggering

#### Configuration
- `DATABASE_URL`: PostgreSQL connection string
- `ALERT_SERVICE_ADDR`: Bind address (default: 127.0.0.1:8085)
- `SMTP_HOST`, `SMTP_PORT`, `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_FROM`: Email channel configuration (optional)
- `TWILIO_ACCOUNT_SID`, `TWILIO_AUTH_TOKEN`, `TWILIO_FROM_NUMBER`: SMS/Twilio configuration (optional)

#### Notification Channel Details

**Email (SMTP)**
- Requires: `SMTP_HOST`, `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_FROM`, optional `SMTP_PORT` (default: 587)
- Config: `{"to": ["email@example.com"], "subject": "Alert", "template": "..."}`
- Template variables: `{severity}`, `{message}`, `{trigger_type}`, `{event_id}`, `{fired_at}`

**Webhook (HTTP/HTTPS)**
- Always available (no global configuration required)
- Config: `{"url": "https://...", "method": "POST", "headers": {...}, "template": "..."}`
- Default: JSON payload with full event details
- Template variables supported for custom payloads

**MQTT**
- Always available (no global configuration required)
- Config: `{"broker": "mqtt://...", "topic": "alerts/{severity}", "qos": 1, "username": "...", "password": "..."}`
- QoS levels: 0 (at most once), 1 (at least once), 2 (exactly once)
- Topic template variables: `{severity}`, `{trigger_type}`, `{tenant_id}`

**Slack**
- Always available (no global configuration required, uses webhook URLs)
- Config: `{"webhook_url": "https://hooks.slack.com/...", "channel": "#alerts", "username": "Quadrant VMS", "icon_emoji": ":camera:"}`
- Features: Color-coded attachments based on severity, rich field formatting
- Template variables: `{severity}`, `{message}`, `{trigger_type}`, `{event_id}`, `{fired_at}`

**Discord**
- Always available (no global configuration required, uses webhook URLs)
- Config: `{"webhook_url": "https://discord.com/api/webhooks/...", "username": "Quadrant Bot", "avatar_url": "..."}`
- Features: Rich embeds with severity-based color coding, timestamp formatting
- Template variables: `{severity}`, `{message}`, `{trigger_type}`, `{event_id}`, `{fired_at}`

**SMS (Twilio)**
- Requires: `TWILIO_ACCOUNT_SID`, `TWILIO_AUTH_TOKEN`, `TWILIO_FROM_NUMBER`
- Config: `{"to": ["+15551234567"], "template": "[{severity}] {message}"}`
- Phone numbers must be in E.164 format (e.g., +15551234567)
- Template variables: `{severity}`, `{message}`, `{trigger_type}`, `{event_id}`, `{fired_at}`
- Default format: `[SEVERITY] trigger_type: message`

---

## Playback & Delivery

### `playback-service` - Multi-Protocol Playback
**Location**: `crates/playback-service/`
**Entry Point**: `crates/playback-service/src/main.rs`

#### Features
- **Multi-protocol playback**: HLS and RTSP delivery
- **Session management**: Playback session lifecycle with state tracking
- **HLS delivery**:
  - Live stream HLS playback from stream-node outputs
  - Recording HLS playback with on-demand transcoding
  - Static file serving for HLS segments and playlists
- **Edge caching**:
  - In-memory LRU cache for HLS segments and playlists
  - Configurable TTL per content type (playlists: 2s, segments: 60s)
  - Size limits (max items and total bytes)
  - HTTP cache headers (ETag, Cache-Control) for client-side caching
  - Cache hit/miss tracking with Prometheus metrics
  - Automatic eviction based on LRU policy
  - Cache statistics endpoint at `/metrics/cache`
- **LL-HLS (Low-Latency HLS)**:
  - Partial segment support for sub-second latency (~1-2 seconds)
  - Blocking playlist reload (CAN-BLOCK-RELOAD) for reduced request overhead
  - Preload hints for upcoming segments
  - HLS version 9+ compliance with EXT-X-PART tags
  - Configurable part duration and segments-per-part ratio
  - Query parameter support (_HLS_msn, _HLS_part) for client synchronization
- **RTSP proxy**: RTSP proxy server for live streams and recording playback
- **WebRTC playback**:
  - WHEP (WebRTC-HTTP Egress Protocol) support for ultra-low-latency streaming
  - SDP offer/answer exchange via HTTP POST
  - WebRTC peer connection management with automatic cleanup
  - H.264 video and Opus audio codec support
  - STUN server integration for NAT traversal
  - Session-based connection lifecycle (create, maintain, delete)
- **DVR time-shift playback**:
  - Rewind and replay live streams within a configurable buffer window
  - Segment timeline tracking with automatic buffer management
  - Seek to absolute timestamps or relative offsets from live edge
  - Jump to live edge functionality
  - DVR window information API for available time ranges
  - Configurable buffer limits (default: 5 minutes, up to 1 hour)
  - Support for both timestamp-based and time-offset seeking
- **Time-based navigation**: Seek support for recordings with timestamp control
- **Time-axis preview**:
  - Generate evenly-spaced thumbnail previews along recording timelines
  - Configurable thumbnail count, dimensions, and quality
  - Base64-encoded JPEG thumbnails with position metadata
  - Position percentage calculation for timeline UI rendering
  - Automatic video duration detection
  - Support for MP4, MKV, and HLS recordings
- **Playback controls**: Pause, resume, stop, and speed control
- **PostgreSQL-backed storage**: Persistent playback session state
- **Multi-source support**: Playback from both live streams and recordings

#### REST API
- `/v1/playback/start` - Start playback session
- `/v1/playback/stop` - Stop playback session
- `/v1/playback/seek` - Seek to timestamp (recordings only)
- `/v1/playback/control` - Pause/resume/stop controls
- `/v1/playback/sessions` - List active playback sessions

#### DVR API
- `/v1/dvr/window` - Get DVR window information (available time range)
- `/v1/dvr/seek` - Seek to timestamp or offset from live edge
- `/v1/dvr/jump_to_live` - Return to live edge from DVR mode

#### Time-Axis Preview API
- `/v1/preview/time_axis` - Generate time-axis preview thumbnails (POST)
  - Request body:
    ```json
    {
      "source_id": "recording-123",
      "source_type": "recording",
      "count": 10,
      "width": 320,
      "height": 180,
      "quality": 5
    }
    ```
  - Response:
    ```json
    {
      "source_id": "recording-123",
      "source_type": "recording",
      "duration_secs": 120.0,
      "thumbnails": [
        {
          "timestamp_secs": 12.0,
          "position_percent": 0.1,
          "width": 320,
          "height": 180,
          "image_data": "base64-encoded-jpeg-data"
        }
      ]
    }
    ```

#### WebRTC (WHEP) API
- `/whep/stream/{stream_id}` - WHEP endpoint for live stream playback (POST SDP offer)
- `/whep/recording/{recording_id}` - WHEP endpoint for recording playback (POST SDP offer)
- `/whep/session/{session_id}` - Delete WHEP session (DELETE)

#### HLS File Serving
- `/hls/streams/{stream_id}/index.m3u8` - Live stream playlists (standard HLS)
- `/hls/recordings/{recording_id}/index.m3u8` - Recording playlists (standard HLS)
- `/ll-hls/streams/{stream_id}/playlist.m3u8` - LL-HLS playlists with blocking support
  - Query parameters:
    - `_HLS_msn={n}` - Wait for media sequence number n
    - `_HLS_part={p}` - Wait for part p within the segment
    - `_HLS_skip=YES` - Skip older segments for faster loading

#### Configuration
- `DATABASE_URL`: PostgreSQL connection string (optional)
- `PLAYBACK_SERVICE_ADDR`: Bind address (default: 127.0.0.1:8087)
- `HLS_BASE_URL`: Base URL for HLS delivery (default: http://localhost:8087/hls)
- `RTSP_BASE_URL`: Base URL for RTSP delivery (default: rtsp://localhost:8554)
- `HLS_ROOT`: HLS files directory (default: ./data/hls)
- `RECORDING_STORAGE_ROOT`: Recording files directory (default: ./data/recordings)
- `NODE_ID`: Playback node identifier (auto-generated if not set)
- `LL_HLS_ENABLED`: Enable LL-HLS support (default: false)
- `PLAYBACK_SERVICE_URL`: Base URL for WebRTC WHEP endpoints (default: http://localhost:8087)

#### Edge Cache Configuration
- `EDGE_CACHE_ENABLED`: Enable edge caching (default: true)
- `EDGE_CACHE_MAX_ITEMS`: Maximum number of cached items (default: 10000)
- `EDGE_CACHE_MAX_SIZE_MB`: Maximum cache size in megabytes (default: 1024)
- `EDGE_CACHE_PLAYLIST_TTL_SECS`: TTL for .m3u8 playlists in seconds (default: 2)
- `EDGE_CACHE_SEGMENT_TTL_SECS`: TTL for .ts/.m4s segments in seconds (default: 60)

#### DVR Configuration

DVR time-shift is enabled per playback session via the `dvr` field in `PlaybackConfig`:
- **enabled**: Enable DVR mode for this session (default: false)
- **rewind_limit_secs**: Maximum rewind time in seconds (default: 3600 = 1 hour, None = unlimited)
- **buffer_window_secs**: Rolling buffer window size (default: 300 = 5 minutes)

Example playback start request with DVR:
```json
{
  "config": {
    "session_id": "session-123",
    "source_type": "stream",
    "source_id": "camera-01",
    "protocol": "hls",
    "dvr": {
      "enabled": true,
      "rewind_limit_secs": 1800,
      "buffer_window_secs": 600
    }
  }
}
```

DVR features:
- **Automatic segment tracking**: Scans HLS segments and builds timeline
- **Timestamp-based seeking**: Seek to absolute Unix timestamps
- **Relative seeking**: Seek relative to live edge (e.g., -30 seconds)
- **Live edge detection**: Automatically tracks latest available content
- **Buffer management**: Automatic trimming based on buffer_window_secs

#### LL-HLS Configuration

LL-HLS behavior is configured via `LlHlsConfig` with these defaults:
- **Part target duration**: 0.33 seconds (330ms parts for ~1s total latency)
- **Parts per segment**: 6 parts (~2 second full segments)
- **Server push**: Disabled (requires HTTP/2)
- **Blocking reload**: Enabled (CAN-BLOCK-RELOAD=YES)
- **Max blocking duration**: 3.0 seconds (maximum wait time for new segments)

These values optimize for low latency while maintaining reliability. For ultra-low latency:
- Reduce `part_target_duration` to 0.2s (200ms)
- Increase `max_blocking_duration` for slower networks
- Enable `enable_server_push` if using HTTP/2

---

## Shared Libraries

### `common` - Shared Utilities
**Location**: `crates/common/`

#### Features
- Shared types and utilities across all services
- Contract definitions for inter-service communication
- Lease types and state management
- Stream, recording, and AI task types
- **auth_middleware**: Shared authentication middleware
  - JWT token verification and validation
  - Permission checking utilities
  - Request context injection
  - Support for both JWT and API token authentication
- **Frame capture utilities**: FFmpeg-based frame extraction
  - Base64-encoded JPEG frame transport
  - Automatic frame dimension probing
  - Configurable quality and scaling
- **StateStore client**: HTTP client for remote state access
  - State save/retrieve/update operations
  - List by node_id filtering
  - Connection pooling and retry logic

---

### `telemetry` - Observability Infrastructure
**Location**: `crates/telemetry/`

#### Features
- **Centralized structured logging** with multiple output formats:
  - **JSON**: Machine-readable format for log aggregation systems (ELK, Loki, Datadog)
  - **Pretty**: Human-readable format with colors for local development
  - **Compact**: Condensed text format for resource-constrained environments
- **Distributed tracing** with OpenTelemetry support:
  - **OTLP backend**: OpenTelemetry Protocol for collector integration (supports Jaeger, Zipkin, and other OTLP collectors)
  - **Automatic span propagation**: Trace context flows across all HTTP requests
  - **Configurable sampling**: Control trace sampling rate (0.0-1.0)
  - **Service metadata**: Automatic service name, version, environment tagging
- **HTTP tracing middleware**: Request/response logging with latency tracking
- **Correlation ID middleware**: Automatic request tracing with `x-correlation-id` header propagation
- **Contextual logging**: Service name, version, environment, and node ID in every log entry
- **Log rotation**: File-based logging with daily rotation (optional)
- **Prometheus metrics registry**: Centralized metric collection across all services
- **Configurable log filtering**: Environment-based log level control per module
- **Service Level Objective (SLO) metrics** with comprehensive monitoring across four dimensions:
  - **Availability**: Service uptime, health checks, dependency status (by tenant and node)
  - **Latency**: Request processing time, database query latency, external API latency, TTFB
  - **Error Rate**: Failed requests (4xx/5xx), database errors, external API errors, pipeline failures
  - **Throughput**: Request rate, bytes processed, concurrent operations, queue depth
  - **Resource Utilization**: CPU, memory, disk I/O, network bandwidth
- **Pre-built Grafana dashboards** for SLO monitoring:
  - **Overview dashboard**: Cross-service SLO monitoring with tenant/node filtering
  - **Tenant-specific dashboards**: SLO compliance and error budgets per tenant
  - **Node-specific dashboards**: Resource utilization and workload distribution per node
  - **Custom metrics aggregation**: Flexible dashboard generation for any tenant/node combination

#### Configuration (Environment Variables)

**Logging:**
- `LOG_FORMAT`: Output format (`json`, `pretty`, `compact`) - default: `pretty`
- `SERVICE_VERSION`: Service version for log context
- `NODE_ID`: Node identifier for distributed systems
- `ENVIRONMENT`: Deployment environment (`development`, `staging`, `production`)
- `LOG_SPAN_EVENTS`: Enable span enter/exit events - default: `false`
- `LOG_TO_FILE`: Enable file logging - default: `false`
- `LOG_DIR`: Log file directory when file logging is enabled
- `RUST_LOG`: Standard tracing filter (e.g., `info`, `debug`, `module=debug`)

**Distributed Tracing:**
- `TRACING_BACKEND`: Backend type (`otlp` or unset to disable) - default: none
- `OTLP_ENDPOINT`: OTLP collector endpoint (compatible with Jaeger/Zipkin) - default: `http://localhost:4317`
- `TRACE_SAMPLE_RATE`: Sampling rate (0.0 = none, 1.0 = all) - default: `1.0`

#### Usage Example

**With Distributed Tracing:**
```rust
use telemetry::{TracingConfig, TracingBackend, trace_http_request};
use axum::{Router, middleware};
use tower::ServiceBuilder;

// Initialize distributed tracing
let tracing_config = TracingConfig::new("my-service")
    .with_version(env!("CARGO_PKG_VERSION"))
    .with_backend(TracingBackend::Otlp {
        endpoint: "http://localhost:4317".to_string(),
    })
    .with_sample_rate(0.5)
    .with_environment("production");

telemetry::init_distributed_tracing(tracing_config)?;

// Add HTTP tracing middleware
let app = Router::new()
    .route("/health", get(health_check))
    .layer(
        ServiceBuilder::new()
            .layer(middleware::from_fn(trace_http_request))
    );

// Log with structured fields (automatically includes trace context)
tracing::info!(
    user_id = %user.id,
    action = "login",
    "User logged in successfully"
);

// Shutdown on exit
telemetry::shutdown_tracing();
```

**With Structured Logging Only:**
```rust
use telemetry::{LogConfig, LogFormat};

// Initialize structured logging (no distributed tracing)
let log_config = LogConfig::new("my-service")
    .with_version(env!("CARGO_PKG_VERSION"))
    .with_format(LogFormat::Json)
    .with_environment("production")
    .with_node_id("node-1");
telemetry::init_structured_logging(log_config);
```

**With SLO Metrics Tracking:**
```rust
use telemetry::SloTracker;
use std::time::Instant;

// Create an SLO tracker for your service
let slo_tracker = SloTracker::new("playback-service", "node-1");

// Mark service as up
slo_tracker.set_service_status(true, Some("tenant-123"));

// Track request latency and status
let start = Instant::now();
let result = handle_request().await;
let duration = start.elapsed().as_secs_f64();

slo_tracker.record_request_latency(
    "/api/playback/start",
    duration,
    Some("tenant-123")
);

slo_tracker.record_request(
    "/api/playback/start",
    "POST",
    200,
    Some("tenant-123")
);

// Track database operations
let db_start = Instant::now();
let sessions = db.query_sessions().await?;
let db_duration = db_start.elapsed().as_secs_f64();

slo_tracker.record_db_latency("select", db_duration, Some("tenant-123"));

// Track bytes processed
slo_tracker.record_bytes_processed("stream", 1024 * 1024, Some("tenant-123"));

// Track concurrent operations
slo_tracker.set_concurrent_operations("playback_sessions", 5, Some("tenant-123"));

// Record pipeline failures
if let Err(e) = pipeline.run().await {
    slo_tracker.record_pipeline_failure(
        "hls_delivery",
        "segment_not_found",
        Some("tenant-123")
    );
}

// Export SLO metrics at /metrics/slo endpoint
use telemetry::encode_slo_metrics;
let metrics_output = encode_slo_metrics()?;
```

**Grafana Dashboard Generation:**
```rust
use telemetry::{generate_slo_dashboard, generate_tenant_slo_dashboard};
use std::fs;

// Generate overview dashboard JSON
let overview_dashboard = generate_slo_dashboard();
fs::write("grafana-slo-overview.json", serde_json::to_string_pretty(&overview_dashboard)?)?;

// Generate tenant-specific dashboard
let tenant_dashboard = generate_tenant_slo_dashboard("tenant-123");
fs::write("grafana-tenant-123.json", serde_json::to_string_pretty(&tenant_dashboard)?)?;
```

#### Distributed Tracing

**Correlation ID Tracing:**
Every HTTP request is automatically assigned a correlation ID that propagates through:
- Incoming requests via `x-correlation-id` or `x-request-id` headers
- Automatic generation if not present
- Tracing spans for all downstream operations
- Response headers for client-side tracing

**OpenTelemetry Spans:**
When distributed tracing is enabled, all HTTP requests and service operations create OpenTelemetry spans that:
- Automatically propagate trace context across service boundaries
- Include service metadata (name, version, environment, node ID)
- Track request latency, status codes, and errors
- Support both Jaeger and OTLP backends for trace collection
- Allow configurable sampling rates for production environments

**Example Trace Flow:**
```
admin-gateway → coordinator (acquire lease) → stream-node (start stream)
   └─ span_id: abc123       └─ span_id: def456      └─ span_id: ghi789
      trace_id: xyz (propagated across all services)
```

This enables **end-to-end distributed request tracing** across all microservices in the VMS cluster.

---

## State Management

### StateStore System
The StateStore system provides persistent state management for stateless architectures and high availability deployments.

#### Features
- **PostgreSQL-backed storage**: Persistent state across restarts
- **HTTP API**: RESTful interface for state operations
- **StateStore client**: HTTP client in common crate
- **Multi-instance coordination**: Shared state across coordinator instances
- **Automated orphan cleanup**: Detect and remove orphaned resources
- **Bootstrap logic**: Restore state on startup for all services
- **Backward compatible**: Works with or without StateStore enabled

#### State Migration Tools
The `state-migrate` binary provides command-line tools for state management:
- `check` - Verify database schema and migrations
- `list-orphans` - List orphaned resources with filtering
- `cleanup-orphans` - Clean up orphans (with dry-run mode)
- `export` - Export all state to JSON for backup/migration
- `import` - Import state from JSON (with skip-existing option)
- `vacuum` - Database maintenance (VACUUM ANALYZE)
- `stats` - Show comprehensive state store statistics

#### Configuration
- `ENABLE_STATE_STORE`: Enable state persistence (default: false)
- `ORPHAN_CLEANUP_INTERVAL_SECS`: Cleanup interval (default: 300)
- `DATABASE_URL`: PostgreSQL connection string

**Documentation**: See [docs/HA_DEPLOYMENT.md](docs/HA_DEPLOYMENT.md) for complete guide

---

## Metrics & Observability

All services expose a `/metrics` endpoint for Prometheus scraping with comprehensive metrics:

- **Coordinator**: Active leases, operations, cluster nodes, leader elections, forwarded requests
- **Stream-node**: Active streams, HLS segments, S3 uploads, bytes processed
- **Recorder-node**: Active recordings, operations, bytes recorded, completion status
- **Admin-gateway**: HTTP requests, duration, active workers, worker operations
- **AI-service**: Active tasks, frames processed, detections, latency, plugin health
- **Device-manager**: Device health status, health check operations
- **Alert-service**: Alert rules, triggered events, notification delivery
- **Playback-service**: Active sessions, playback operations, bytes delivered, cache hit/miss rates, cache size, evictions

**Documentation**: See [docs/GPU_ACCELERATION.md](docs/GPU_ACCELERATION.md) for GPU metrics and optimization

---

## Integration Testing

The project includes comprehensive integration tests covering:
- Unit tests for lease store logic, router contracts, recording lifecycle
- Integration tests for coordinator clustering, recorder integration, metrics collection
- AI plugin system tests, frame capture pipeline tests
- Full end-to-end tests (`tests/full_pipeline_e2e.rs`):
  - Complete pipeline: stream → recording → AI processing
  - Multi-service interaction with coordinator orchestration
  - Lease management across all service types
  - Health check verification
  - Multi-component error handling

**Run tests**: `cargo test` or `make test`

---

For deployment instructions, see [docs/HA_DEPLOYMENT.md](docs/HA_DEPLOYMENT.md)
For authentication setup, see [docs/AUTHENTICATION.md](docs/AUTHENTICATION.md)
For GPU acceleration, see [docs/GPU_ACCELERATION.md](docs/GPU_ACCELERATION.md)

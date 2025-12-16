# Quadrant VMS (Rust)

> ⚙️ A Video Management System (VMS) built in **Rust**.
> The project aims to be **modular**, **cluster-ready**, and **AI-model friendly**.

---

## 🚧 Development Status
This project is **under active development**.

---

## 🏗️ Architecture Overview

Quadrant VMS is built as a **Cargo workspace** with multiple specialized services:

### Core Services
- **`coordinator`** - Lease-based distributed job scheduler with multi-node clustering and leader election
- **`admin-gateway`** - REST API facade for orchestrating all VMS operations
- **`stream-node`** - RTSP to HLS transcoding with S3 storage and AI frame capture
- **`recorder-node`** - FFmpeg-based recording pipeline with retention management and search indexing
- **`playback-service`** - Multi-protocol playback delivery (HLS/RTSP) for live streams and recordings
- **`operator-ui`** - Web-based dashboard for monitoring and managing all VMS operations

### Intelligence & Automation
- **`ai-service`** - Modular AI plugin system with YOLOv8 object detection, pose estimation, and GPU acceleration
- **`alert-service`** - Event-driven alert and automation system with multi-channel notifications

### Security & Management
- **`auth-service`** - Authentication, authorization (RBAC), multi-tenancy, and OIDC/OAuth2 SSO integration
- **`device-manager`** - Camera/device management with ONVIF discovery, PTZ control, health monitoring, and firmware updates

### Shared Libraries
- **`common`** - Shared utilities, types, auth middleware, and state management clients
- **`telemetry`** - Centralized logging and Prometheus metrics infrastructure

**For detailed service documentation, see [SERVICES.md](SERVICES.md)**

---

## ✅ Key Features Implemented

### Distributed Architecture
- **Multi-coordinator clustering** with Raft-inspired leader election and automatic failover
- **Lease-based resource management** for distributed stream, recording, and AI task coordination
- **StateStore system** for stateless architecture and high-availability deployments
- **Automated orphan cleanup** with configurable retention policies

### Video Management
- **Live streaming**: RTSP → HLS (TS/fMP4) with S3 storage and fallback
- **Recording pipeline**: Multi-format support (MP4/HLS/MKV) with metadata extraction
- **Playback delivery**: HLS and RTSP delivery with seek, pause, resume controls
- **WebRTC playback**: WHEP protocol support for ultra-low-latency WebRTC streaming
- **LL-HLS support**: Low-latency HLS with partial segments and blocking playlist reload for sub-second latency
- **DVR time-shift**: Rewind and replay live streams with configurable buffer windows
- **Edge caching**: In-memory LRU cache for HLS segments/playlists with configurable TTL and size limits
- **Thumbnail generation**: Single frame and grid thumbnails for timeline preview
- **Time-axis preview**: Evenly-spaced thumbnail previews along recording timelines for video scrubbing and navigation
- **Retention management**: Time-based policies, storage quotas, tiered storage
- **Search & indexing**: Full-text search for recordings and AI events

### AI & Intelligence
- **YOLOv8 object detection**: Real-time detection with 80 COCO classes
- **Pose estimation**: Human pose detection with COCO 17 keypoint format
- **Action recognition**: Temporal video analysis detecting 20+ human actions (walking, running, sitting, waving, etc.)
- **License plate recognition (LPR)**: Two-stage detection and OCR for automatic plate reading
- **Facial recognition**: Two-stage face detection and embedding extraction with face database matching
- **Crowd analytics**: Person counting, crowd density analysis, hotspot detection, and spatial distribution heatmaps
- **Anomaly detection**: Temporal and spatial anomaly detection for unusual patterns, restricted zone violations, and abnormal object counts
- **GPU acceleration**: CUDA and TensorRT support with automatic fallback
- **Frame capture pipeline**: Automatic frame extraction from live streams and recordings
- **Modular plugin architecture**: Extensible system for custom AI models

### Device Management
- **ONVIF device discovery**: Automatic network scanning with WS-Discovery protocol
- **Health monitoring**: Automated periodic checks with status tracking
- **PTZ control**: Pan/tilt/zoom with presets and automated tour system
- **Camera configuration push**: Remote video encoder, image settings, and network configuration
- **Firmware update management**: Automated ONVIF firmware upgrades with rollback support

### Security & Access Control
- **JWT authentication** with API token support
- **Role-Based Access Control (RBAC)**: Fine-grained permissions across 29 resources
- **Multi-tenancy**: Isolated tenant environments with resource quotas
- **OIDC/OAuth2 SSO**: Integration with Google, Azure AD, Keycloak, and custom providers
- **Audit logging**: Complete security audit trail for compliance

### Alerts & Automation
- **Rule engine**: Flexible condition-based triggering with JSON matching
- **Multi-channel notifications**: Email (SMTP), Webhook, MQTT, Slack, Discord, SMS (Twilio)
- **Alert suppression**: Cooldown periods and rate limiting
- **Scheduling**: Cron-based time windows for active rules

### Resilience & Observability
- **Worker health verification** with liveness checks during lease renewal
- **Automatic retry** with exponential backoff (up to 3 retries)
- **Graceful degradation** during temporary coordinator unavailability
- **Prometheus metrics** across all services with 38+ comprehensive tests
- **Health check endpoints** (`/readyz`) with dependency verification
- **Centralized structured logging** with JSON/pretty/compact formats, correlation IDs for request tracing, and configurable log aggregation
- **Distributed tracing** with OpenTelemetry OTLP support (compatible with Jaeger, Zipkin, and other OTLP collectors), automatic span propagation across services, and configurable sampling rates
- **Service Level Objective (SLO) metrics** with comprehensive monitoring across availability, latency, error rate, throughput, and resource utilization dimensions - all labeled by tenant and node for granular insights
- **Pre-built Grafana dashboards** for SLO monitoring with overview, tenant-specific, and node-specific views including error budget tracking and custom metrics aggregation

---

## 📊 System Status

### ✅ Production-Ready Features
- Core video pipeline (streaming, recording, playback)
- Distributed coordination with clustering
- AI processing with GPU acceleration
- Device management with ONVIF support
- Authentication & authorization
- Alert system with multi-channel notifications
- Retention & search capabilities
- Stateless architecture with HA support

### Operator Dashboard
- **Web-based Operator UI**: Responsive React dashboard for monitoring and management
- **Multi-view interface**: Devices, live streams, recordings, AI tasks, alerts, and incidents
- **Real-time WebSocket updates**: Live data streaming for dashboard statistics
- **Incident workflow system**: Create, acknowledge, resolve incidents with notes and timeline
- **Search capabilities**: Full-text search for recordings and AI detections
- **Alert rule management**: Enable/disable alert rules directly from UI
- **Stream control**: Start/stop live streams from dashboard

---

## 🚀 Quick Start

### Prerequisites
- Rust 1.70+ (`cargo --version`)
- FFmpeg 4.0+ (`ffmpeg -version`)
- PostgreSQL 13+ (for persistent storage)
- Docker & Docker Compose (optional, for containerized deployment)

### Build & Test
```bash
# Build all crates
cargo build --release

# Run tests
cargo test
# or
make test
```

### Launch Individual Services
```bash
# Stream node (RTSP → HLS)
HLS_ROOT=./data/hls cargo run -p stream-node

# Coordinator (lease scheduler)
cargo run -p coordinator

# Admin gateway (REST API)
COORDINATOR_URL="http://localhost:8082" cargo run -p admin-gateway

# Recorder node (recording pipeline)
COORDINATOR_URL="http://localhost:8082" cargo run -p recorder-node

# AI service (AI processing)
COORDINATOR_URL="http://localhost:8082" cargo run -p ai-service

# Auth service (authentication)
cargo run -p auth-service

# Device manager (device management)
cargo run -p device-manager

# Alert service (alerts & automation)
cargo run -p alert-service

# Playback service (playback delivery)
cargo run -p playback-service

# Operator UI (web dashboard)
cargo run -p operator-ui
```

### Docker Compose Deployment (Recommended)

**For complete VMS deployment with all services:**

```bash
# 1. Initialize environment configuration
make docker-init

# 2. Review and customize .env file
vim .env

# 3. Build all service images
make docker-build

# 4. Start the entire stack
make docker-up

# 5. Check service status
make docker-status

# 6. View logs
make docker-logs
```

**Access the services:**
- **Operator UI Dashboard**: http://localhost:8090
- **Admin Gateway API**: http://localhost:8081
- **MinIO Console**: http://localhost:9001 (credentials in `.env`)

**Common operations:**
```bash
# Stop all services
make docker-down

# Restart all services
make docker-restart

# View specific service logs
make logs-ui          # Operator UI
make logs-coordinator # Coordinator
make logs-gateway     # Admin Gateway

# Clean up (removes all data!)
make docker-clean
```

**📚 For detailed deployment instructions, see [DOCKER_DEPLOYMENT.md](DOCKER_DEPLOYMENT.md)**

---

## 📁 Project Structure

```
quadrant-vms/core/
├── crates/
│   ├── admin-gateway/      # REST API facade
│   ├── ai-service/          # AI plugin system
│   ├── alert-service/       # Alert & automation
│   ├── auth-service/        # Authentication & RBAC
│   ├── common/              # Shared utilities
│   ├── coordinator/         # Lease scheduler
│   ├── device-manager/      # Device management
│   ├── operator-ui/         # Web dashboard
│   ├── playback-service/    # Playback delivery
│   ├── recorder-node/       # Recording pipeline
│   ├── stream-node/         # RTSP → HLS transcoding
│   └── telemetry/           # Observability
├── tests/                   # Integration tests
├── profiles/                # Deployment profiles
├── docs/                    # Documentation
├── Cargo.toml               # Workspace manifest
├── Makefile                 # Build & deployment shortcuts
├── README.md                # This file
├── SERVICES.md              # Detailed service documentation
└── CLAUDE.md                # Development guide for Claude Code
```

---

## 📖 Documentation

- **[SERVICES.md](SERVICES.md)** - Detailed documentation for each service
- **[docs/AUTHENTICATION.md](docs/AUTHENTICATION.md)** - Authentication & authorization setup
- **[docs/HA_DEPLOYMENT.md](docs/HA_DEPLOYMENT.md)** - High-availability deployment guide
- **[docs/GPU_ACCELERATION.md](docs/GPU_ACCELERATION.md)** - GPU acceleration setup for AI workloads
- **[CLAUDE.md](CLAUDE.md)** - Development guide for Claude Code (AI assistant)

---

## 🧪 Testing

The project includes comprehensive test coverage:
- **Unit tests**: Co-located with source files for core logic
- **Integration tests**: Service interaction and API contracts (`tests/` directory)
- **End-to-end tests**: Complete pipeline validation (`tests/full_pipeline_e2e.rs`)

**Total**: 38+ tests covering lease management, recording lifecycle, AI processing, clustering, metrics, and multi-service orchestration.

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name
```

---

## 🛠️ Configuration

Services are configured via environment variables. Key variables:

- `DATABASE_URL` - PostgreSQL connection string (used by most services)
- `COORDINATOR_URL` - Coordinator service URL for worker nodes
- `JWT_SECRET` - Secret key for JWT signing (auth-service)
- `HLS_ROOT` - HLS output directory (stream-node)
- `RECORDING_STORAGE_ROOT` - Recording storage location (recorder-node)
- `ENABLE_STATE_STORE` - Enable state persistence for HA (default: false)
- `CLUSTER_ENABLED` - Enable multi-node clustering (coordinator)
- `TRACING_BACKEND` - Distributed tracing backend: `otlp` or unset for disabled (default: none)
- `OTLP_ENDPOINT` - OTLP collector endpoint for Jaeger/OTLP collectors (default: http://localhost:4317)
- `TRACE_SAMPLE_RATE` - Trace sampling rate 0.0-1.0 (default: 1.0)

**For complete configuration options, see [SERVICES.md](SERVICES.md)**

Example `.env` file:
```bash
DATABASE_URL=postgresql://postgres:postgres@localhost:5432/quadrant_vms
COORDINATOR_URL=http://localhost:8082
JWT_SECRET=your-secret-key-change-in-production
ENABLE_STATE_STORE=true
CLUSTER_ENABLED=true
```

---

## 🔧 Development

### Makefile Commands
```bash
make test          # Run tests
make launch        # Launch stream-node locally
make init-dc       # Initialize Docker Compose stack
make status-dc     # Check Docker Compose status
```

### Common Development Tasks
- **Adding a new feature**: See [CLAUDE.md](CLAUDE.md) for development workflow
- **Debugging inter-service communication**: Check contract definitions in `crates/common/src/`
- **Modifying lease logic**: Focus on `crates/coordinator/src/store.rs`
- **Running integration tests**: `cargo test --test <test_name>`

---

## 📊 Metrics & Monitoring

All services expose Prometheus metrics at `/metrics`:
- Coordinator: Active leases, cluster status, operations
- Stream-node: Active streams, segments, S3 uploads, bytes processed
- Recorder-node: Active recordings, bytes recorded, completion status
- AI-service: Active tasks, frames processed, detections, latency
- Device-manager: Device health, operations
- Alert-service: Rules, events, notifications
- Playback-service: Sessions, bytes delivered

**For detailed metrics, see [SERVICES.md](SERVICES.md)**

---

## 🤝 Contributing

We welcome contributions! Please ensure:
1. All tests pass (`cargo test`)
2. Code follows existing patterns and style
3. New features include tests
4. Update documentation (README.md, SERVICES.md, CLAUDE.md) as needed

---

## 💡 Follow Progress
Each milestone (camera compatibility, failover tests, AI plugins, etc.)
will unlock sequentially as community funding goals are reached.

Stay tuned.

---

© 2025 Quadrant Intelligence Studio

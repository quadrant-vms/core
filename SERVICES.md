# Quadrant VMS - Service Details

This document provides detailed information about each service in the Quadrant VMS architecture.

---

## Core Services

### `operator-ui` - Web Dashboard

**Location**: `crates/operator-ui/`
**Entry Point**: `crates/operator-ui/src/main.rs`
**Frontend**: `crates/operator-ui/frontend/`

#### Features
- **Responsive React dashboard** with modern dark UI theme
- **Real-time WebSocket updates** for live dashboard statistics
- **Multi-view interface**:
  - **Dashboard**: System-wide statistics and health overview
  - **Devices**: Device monitoring with health status
  - **Live Streams**: Active stream monitoring and control
  - **Recordings**: Recording management with search and playback links
  - **AI Tasks**: AI task monitoring and detection visualization
  - **Alerts**: Alert history and rule management
  - **Incidents**: Incident workflow system with notes and timeline
- **Incident Management**:
  - Create incidents from alerts or manually
  - Acknowledge and resolve workflow
  - Add notes and track timeline
  - Link to devices and alerts
  - Severity and status tracking
- **Search Capabilities**:
  - Full-text search for recordings
  - Filter by device, date range
  - AI detection queries
- **Stream Control**:
  - Stop active streams
  - View stream details and status
- **Alert Rule Management**:
  - Enable/disable rules
  - View rule configuration
  - Monitor alert history

#### Architecture
- **Backend**: Rust + Axum web framework
- **Frontend**: React + Vite + JavaScript
- **Communication**: REST API + WebSocket for real-time updates
- **State Management**: In-memory incident store (can be extended to PostgreSQL)
- **Service Integration**: Proxies requests to all backend services

#### Configuration
Environment Variables:
- `OPERATOR_UI_ADDR`: Bind address (default: `0.0.0.0:8090`)
- `FRONTEND_DIR`: Frontend build directory (default: `./crates/operator-ui/frontend/dist`)
- `DEVICE_MANAGER_URL`: Device manager URL (default: `http://localhost:8087`)
- `ADMIN_GATEWAY_URL`: Admin gateway URL (default: `http://localhost:8080`)
- `RECORDER_NODE_URL`: Recorder node URL (default: `http://localhost:8085`)
- `AI_SERVICE_URL`: AI service URL (default: `http://localhost:8088`)
- `ALERT_SERVICE_URL`: Alert service URL (default: `http://localhost:8089`)
- `AUTH_SERVICE_URL`: Auth service URL (default: `http://localhost:8081`)
- `PLAYBACK_SERVICE_URL`: Playback service URL (default: `http://localhost:8084`)

#### API Endpoints
- `GET /health` - Health check
- `GET /api/dashboard/stats` - Dashboard statistics
- `GET /api/devices` - List devices
- `GET /api/devices/:id` - Get device details
- `GET /api/devices/:id/health` - Get device health
- `GET /api/streams` - List active streams
- `GET /api/streams/:id` - Get stream details
- `POST /api/streams/:id/stop` - Stop a stream
- `GET /api/recordings` - List recordings
- `POST /api/recordings/search` - Search recordings
- `GET /api/recordings/:id` - Get recording details
- `GET /api/recordings/:id/thumbnail` - Get recording thumbnail
- `GET /api/ai/tasks` - List AI tasks
- `GET /api/ai/tasks/:id` - Get AI task details
- `GET /api/ai/detections` - List AI detections
- `GET /api/alerts` - List alerts
- `GET /api/alerts/:id` - Get alert details
- `GET /api/alerts/rules` - List alert rules
- `GET /api/alerts/rules/:id` - Get rule details
- `POST /api/alerts/rules/:id/enable` - Enable alert rule
- `POST /api/alerts/rules/:id/disable` - Disable alert rule
- `GET /api/incidents` - List incidents
- `POST /api/incidents` - Create incident
- `GET /api/incidents/:id` - Get incident details
- `POST /api/incidents/:id` - Update incident
- `POST /api/incidents/:id/acknowledge` - Acknowledge incident
- `POST /api/incidents/:id/resolve` - Resolve incident
- `POST /api/incidents/:id/notes` - Add note to incident
- `GET /ws` - WebSocket connection for real-time updates

#### Frontend Development
```bash
# Navigate to frontend directory
cd crates/operator-ui/frontend

# Install dependencies
npm install

# Start development server (with proxy to backend)
npm run dev

# Build for production
npm run build
```

#### Deployment
```bash
# Build frontend
cd crates/operator-ui/frontend
npm install
npm run build

# Build and run backend (serves frontend from dist/)
cd ../..
cargo build --release -p operator-ui
OPERATOR_UI_ADDR=0.0.0.0:8090 ./target/release/operator-ui
```

#### WebSocket Protocol
The WebSocket endpoint (`/ws`) supports real-time updates:

**Client → Server Messages**:
```json
{"type": "ping"}
{"type": "subscribe", "topics": ["dashboard", "devices", "streams"]}
{"type": "unsubscribe", "topics": ["dashboard"]}
```

**Server → Client Messages**:
```json
{"type": "pong"}
{"type": "update", "topic": "dashboard", "data": {...}}
{"type": "error", "message": "error description"}
```

#### Integration Tests
```bash
# Run integration tests (requires operator-ui to be running)
cargo test --test operator_ui -- --ignored
```

---

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
- Multi-coordinator clustering with leader election
- StateStore HTTP API for persisting stream/recording/AI task state

#### Configuration
- `LEASE_STORE_TYPE`: `memory` or `postgres`
- `DATABASE_URL`: PostgreSQL connection string (when using postgres backend)
- `CLUSTER_ENABLED`: Enable multi-node clustering
- `NODE_ID`: Unique node identifier for clustering
- `CLUSTER_PEERS`: Comma-separated list of peer coordinator URLs
- `ENABLE_STATE_STORE`: Enable state persistence (default: false)
- `ORPHAN_CLEANUP_INTERVAL_SECS`: Cleanup interval for orphaned resources (default: 300)

---

### `admin-gateway` - REST API Facade

**Location**: `crates/admin-gateway/`
**Entry Point**: `crates/admin-gateway/src/main.rs`

#### Features
- REST API for stream management
- Lease acquisition from coordinator
- Worker lifecycle management
- Stream metadata persistence

---

### `stream-node` - Live Stream Transcoding

**Location**: `crates/stream-node/`
**Entry Point**: `crates/stream-node/src/main.rs`

#### Features
- RTSP to HLS transcoding (TS/fMP4 formats)
- S3 storage upload with fallback
- AI frame capture pipeline
- Lease heartbeat with coordinator

---

### `recorder-node` - Recording Pipeline

**Location**: `crates/recorder-node/`
**Entry Point**: `crates/recorder-node/src/main.rs`

#### Features
- FFmpeg-based recording (MP4/HLS/MKV)
- Metadata extraction with ffprobe
- Recording job management
- Retention policies and cleanup

---

### `playback-service` - Playback Delivery

**Location**: `crates/playback-service/`
**Entry Point**: `crates/playback-service/src/main.rs`

#### Features
- HLS and RTSP playback
- WebRTC playback (WHEP protocol)
- LL-HLS support (Low-Latency HLS)
- DVR time-shift functionality
- Edge caching for HLS segments
- Playback session management

---

## Intelligence & Automation

### `ai-service` - AI Plugin System

**Location**: `crates/ai-service/`
**Entry Point**: `crates/ai-service/src/main.rs`

#### Features
- Modular plugin architecture
- YOLOv8 object detection
- Pose estimation
- Action recognition
- License plate recognition
- Facial recognition
- Crowd analytics
- Anomaly detection
- GPU acceleration (CUDA/TensorRT)

---

### `alert-service` - Alert & Automation System

**Location**: `crates/alert-service/`
**Entry Point**: `crates/alert-service/src/main.rs`

#### Features
- Event-driven alert system
- Rule engine with JSON condition matching
- Multi-channel notifications (Email, Webhook, MQTT, Slack, Discord, SMS)
- Alert suppression and rate limiting
- Scheduling with cron expressions

---

## Security & Management

### `auth-service` - Authentication & Authorization

**Location**: `crates/auth-service/`
**Entry Point**: `crates/auth-service/src/main.rs`

#### Features
- JWT authentication with API tokens
- Role-Based Access Control (RBAC)
- Multi-tenancy support
- OIDC/OAuth2 SSO integration
- Audit logging

---

### `device-manager` - Device Management

**Location**: `crates/device-manager/`
**Entry Point**: `crates/device-manager/src/main.rs`

#### Features
- Camera/device management
- ONVIF device discovery
- Health monitoring
- PTZ control
- Camera configuration push
- Firmware update management

---

## Shared Libraries

### `common` - Shared Utilities

**Location**: `crates/common/`

#### Features
- Contract definitions for inter-service communication
- Shared types (leases, streams, recordings, AI tasks)
- Validation utilities
- Auth middleware
- StateStore clients

---

### `telemetry` - Observability

**Location**: `crates/telemetry/`

#### Features
- Centralized logging infrastructure
- Prometheus metrics registry
- Distributed tracing (OpenTelemetry OTLP)
- SLO metrics and dashboards

---

## Development

### Running All Services
```bash
# Start PostgreSQL
docker run -d -p 5432:5432 -e POSTGRES_PASSWORD=postgres postgres:13

# Start coordinator
DATABASE_URL=postgresql://postgres:postgres@localhost:5432/quadrant_vms cargo run -p coordinator

# Start all other services
cargo run -p admin-gateway
cargo run -p stream-node
cargo run -p recorder-node
cargo run -p ai-service
cargo run -p auth-service
cargo run -p device-manager
cargo run -p alert-service
cargo run -p playback-service

# Start operator UI (frontend + backend)
cargo run -p operator-ui
# Access at http://localhost:8090
```

### Integration Tests
```bash
# Run all tests
cargo test

# Run specific service tests
cargo test --test coordinator
cargo test --test operator_ui -- --ignored
```

---

For more information, see [README.md](README.md) and [CLAUDE.md](CLAUDE.md).

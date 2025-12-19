# Environment Variable Reference

**Last Updated**: 2025-12-19

This document provides the canonical environment variable names for all Quadrant VMS services. The **code is the source of truth** - all deployment manifests (K8s, Docker Compose, etc.) must use these exact names.

---

## Global Infrastructure

### PostgreSQL Database
```bash
DATABASE_URL=postgresql://postgres:postgres@localhost:5432/quadrant_vms
POSTGRES_HOST=localhost
POSTGRES_PORT=5432
POSTGRES_USER=postgres
POSTGRES_PASSWORD=postgres
POSTGRES_DB=quadrant_vms
```

### S3/MinIO Storage
```bash
S3_ENDPOINT=http://localhost:9000
S3_ACCESS_KEY=minio
S3_SECRET_KEY=minio123
S3_REGION=us-east-1
S3_BUCKET=vms                    # ⚠️ NOT S3_BUCKET_NAME
```

### Redis Cache
```bash
REDIS_HOST=localhost
REDIS_PORT=6379
REDIS_URL=redis://localhost:6379
```

### NATS Messaging
```bash
NATS_HOST=localhost
NATS_PORT=4222
NATS_URL=nats://localhost:4222
```

---

## Service-Specific Configuration

### Coordinator (Port 8082)
**Source**: `crates/coordinator/src/config.rs`
```bash
COORDINATOR_ADDR=0.0.0.0:8082
LEASE_STORE_TYPE=postgres              # or "memory"
LEASE_DEFAULT_TTL_SECS=30
LEASE_MAX_TTL_SECS=300
DATABASE_URL=postgresql://...

# Clustering
CLUSTER_ENABLED=true
NODE_ID=coordinator-0
PEER_ADDRS=coordinator-1:8082,coordinator-2:8082
ELECTION_TIMEOUT_MS=5000
HEARTBEAT_INTERVAL_MS=1000

# State Store
ENABLE_STATE_STORE=true
ORPHAN_CLEANUP_INTERVAL_SECS=300
```

### Admin Gateway (Port 8081)
**Source**: `crates/admin-gateway/src/config.rs`
```bash
ADMIN_GATEWAY_ADDR=0.0.0.0:8081
COORDINATOR_ENDPOINT=http://127.0.0.1:8082    # ⚠️ NOT COORDINATOR_URL
STREAM_WORKER_ENDPOINT=http://127.0.0.1:8080  # ⚠️ Required but often missing
RECORDER_WORKER_ENDPOINT=http://127.0.0.1:8083  # ⚠️ Required but often missing
NODE_ID=gateway-node-1
DATABASE_URL=postgresql://...
ENABLE_STATE_STORE=true
```

### Stream Node (Port 8080 or 8083)
**Source**: `crates/stream-node/src/config.rs`, `crates/stream-node/src/storage/uploader.rs`
```bash
STREAM_NODE_ADDR=0.0.0.0:8083
HLS_ROOT=./data/hls

# S3 Configuration
S3_ENDPOINT=http://localhost:9000
S3_ACCESS_KEY=minio
S3_SECRET_KEY=minio123
S3_REGION=us-east-1
S3_BUCKET=vms                    # ⚠️ NOT S3_BUCKET_NAME
```

### Recorder Node (Port 8085)
**Source**: `crates/recorder-node/src/main.rs`
```bash
RECORDER_NODE_ADDR=127.0.0.1:8085
RECORDING_STORAGE_ROOT=./data/recordings
DATABASE_URL=postgresql://...
```

### Auth Service (Port 8087)
**Source**: `crates/auth-service/src/config.rs`
```bash
AUTH_SERVICE_ADDR=0.0.0.0:8087
DATABASE_URL=postgresql://...
JWT_SECRET=your-secret-key-here
JWT_EXPIRATION_SECS=3600
API_TOKEN_EXPIRATION_SECS=86400
```

### Device Manager (Port 8088)
**Source**: `crates/device-manager/src/config.rs`
```bash
DEVICE_MANAGER_ADDR=127.0.0.1:8084
DATABASE_URL=postgresql://...
HEALTH_CHECK_INTERVAL_SECS=60
RTSP_TIMEOUT_SECS=10
```

### AI Service (Port 8084)
**Source**: `crates/ai-service/src/main.rs`
```bash
AI_SERVICE_ADDR=0.0.0.0:8084
DATABASE_URL=postgresql://...
COORDINATOR_URL=http://localhost:8082
NODE_ID=ai-node-1
```

### Alert Service (Port 8089)
**Source**: `crates/alert-service/src/config.rs`
```bash
ALERT_SERVICE_ADDR=0.0.0.0:8089
DATABASE_URL=postgresql://...

# MQTT Notifications
MQTT_BROKER_URL=mqtt://localhost:1883
MQTT_CLIENT_ID=alert-service
```

### Playback Service (Port 8086)
**Source**: `crates/playback-service/src/main.rs`
```bash
PLAYBACK_SERVICE_ADDR=127.0.0.1:8087
NODE_ID=playback-node-1
HLS_BASE_URL=http://localhost:8087/hls
RTSP_BASE_URL=rtsp://localhost:8554
HLS_ROOT=./data/hls
RECORDING_STORAGE_ROOT=./data/recordings

# Low-Latency HLS
LL_HLS_ENABLED=false

# Edge Cache Configuration
EDGE_CACHE_ENABLED=true                 # ⚠️ NOT CACHE_ENABLED
EDGE_CACHE_MAX_ITEMS=10000              # ⚠️ NOT CACHE_MAX_ITEMS
EDGE_CACHE_MAX_SIZE_MB=1024             # ⚠️ NOT CACHE_MAX_SIZE_MB
EDGE_CACHE_PLAYLIST_TTL_SECS=2          # ⚠️ NOT CACHE_TTL_SECS
EDGE_CACHE_SEGMENT_TTL_SECS=60          # ⚠️ NOT CACHE_TTL_SECS

# Optional PostgreSQL for session persistence
DATABASE_URL=postgresql://...
```

### Operator UI (Port 8090)
**Source**: `crates/operator-ui/src/config.rs`
```bash
OPERATOR_UI_ADDR=0.0.0.0:8090
FRONTEND_DIR=./frontend/dist

# Backend service URLs
ADMIN_GATEWAY_URL=http://localhost:8081
AUTH_SERVICE_URL=http://localhost:8087
DEVICE_MANAGER_URL=http://localhost:8088
RECORDER_SERVICE_URL=http://localhost:8085
AI_SERVICE_URL=http://localhost:8084
ALERT_SERVICE_URL=http://localhost:8089
PLAYBACK_SERVICE_URL=http://localhost:8086
COORDINATOR_URL=http://localhost:8082
```

---

## Common Pitfalls and Corrections

### ❌ WRONG → ✅ CORRECT

**Admin Gateway**:
- ❌ `COORDINATOR_URL` → ✅ `COORDINATOR_ENDPOINT`

**Stream Node**:
- ❌ `S3_BUCKET_NAME` → ✅ `S3_BUCKET`

**Playback Service**:
- ❌ `CACHE_ENABLED` → ✅ `EDGE_CACHE_ENABLED`
- ❌ `CACHE_MAX_ITEMS` → ✅ `EDGE_CACHE_MAX_ITEMS`
- ❌ `CACHE_MAX_SIZE_MB` → ✅ `EDGE_CACHE_MAX_SIZE_MB`
- ❌ `CACHE_TTL_SECS` → ✅ `EDGE_CACHE_PLAYLIST_TTL_SECS` + `EDGE_CACHE_SEGMENT_TTL_SECS`

---

## Kubernetes Service Discovery

When deploying to Kubernetes, use service names instead of `localhost`:

```bash
# Admin Gateway in K8s
COORDINATOR_ENDPOINT=http://coordinator:8082
STREAM_WORKER_ENDPOINT=http://stream-node:8083
RECORDER_WORKER_ENDPOINT=http://recorder-node:8085

# AI Service in K8s
COORDINATOR_URL=http://coordinator:8082

# Operator UI in K8s
ADMIN_GATEWAY_URL=http://admin-gateway:8081
AUTH_SERVICE_URL=http://auth-service:8087
DEVICE_MANAGER_URL=http://device-manager:8088
# ... etc
```

---

## Validation Checklist

Before deploying to K8s or Docker Compose:

- [ ] All env vars match the canonical names in this document
- [ ] No `_URL` suffixes where code expects `_ENDPOINT` (or vice versa)
- [ ] No `S3_BUCKET_NAME` - use `S3_BUCKET`
- [ ] No `CACHE_*` for playback service - use `EDGE_CACHE_*`
- [ ] Service discovery URLs use K8s service names (not `localhost`)
- [ ] All database connections use the shared `DATABASE_URL` secret

---

## Updating This Document

When adding a new service or configuration option:

1. Find the config struct in `crates/<service>/src/config.rs` or `main.rs`
2. Extract the `env::var()` calls to identify canonical names
3. Add them to the appropriate section above
4. Mark any non-obvious naming requirements with ⚠️
5. Update the "Common Pitfalls" section if there are known misnaming issues

# Docker Build Guide

This document describes how to build and run containerized services for Quadrant VMS.

## Overview

All services have been containerized using multi-stage builds with:
- **Builder stage**: Rust 1.84 Alpine for compilation
- **Runtime stage**: Alpine 3.20 for minimal image size
- **Security**: Non-root user (`vms:vms` with UID/GID 1000)
- **Health checks**: HTTP health endpoints for all services

## Services and Their Requirements

### Services with Special Dependencies

#### 1. stream-node
- **Runtime dependencies**: GStreamer, FFmpeg
- **Image size**: ~200-300MB
- **Port**: 8080
- **Data volume**: `/data/hls`

#### 2. recorder-node
- **Runtime dependencies**: FFmpeg
- **Image size**: ~100-150MB
- **Port**: 8085
- **Data volume**: `/data/recordings`

#### 3. operator-ui
- **Build dependencies**: Node.js 20 (frontend)
- **Runtime dependencies**: None (static files served by Rust)
- **Image size**: ~50-80MB
- **Port**: 8091
- **Static files**: `/app/static`

### Services with Minimal Runtime

These services only need `ca-certificates` and `libgcc`:

- **coordinator** (port 8082)
- **admin-gateway** (port 8083)
- **ai-service** (port 8084)
- **auth-service** (port 8087)
- **device-manager** (port 8088)
- **alert-service** (port 8089)
- **playback-service** (port 8090)

**Image size**: ~20-40MB each

## Building Images

### Build All Services

```bash
# Build individual service
docker build -f crates/coordinator/Dockerfile -t quadrant-vms/coordinator:latest .
docker build -f crates/stream-node/Dockerfile -t quadrant-vms/stream-node:latest .
docker build -f crates/recorder-node/Dockerfile -t quadrant-vms/recorder-node:latest .
docker build -f crates/admin-gateway/Dockerfile -t quadrant-vms/admin-gateway:latest .
docker build -f crates/ai-service/Dockerfile -t quadrant-vms/ai-service:latest .
docker build -f crates/auth-service/Dockerfile -t quadrant-vms/auth-service:latest .
docker build -f crates/device-manager/Dockerfile -t quadrant-vms/device-manager:latest .
docker build -f crates/alert-service/Dockerfile -t quadrant-vms/alert-service:latest .
docker build -f crates/playback-service/Dockerfile -t quadrant-vms/playback-service:latest .
docker build -f crates/operator-ui/Dockerfile -t quadrant-vms/operator-ui:latest .
```

### Build Script

```bash
#!/bin/bash
# build-all.sh - Build all Docker images

SERVICES=(
  "coordinator"
  "stream-node"
  "recorder-node"
  "admin-gateway"
  "ai-service"
  "auth-service"
  "device-manager"
  "alert-service"
  "playback-service"
  "operator-ui"
)

for service in "${SERVICES[@]}"; do
  echo "Building $service..."
  docker build -f crates/$service/Dockerfile -t quadrant-vms/$service:latest .
done
```

## Running Containers

### Example: Run Coordinator

```bash
docker run -d \\
  --name quadrant-coordinator \\
  -p 8082:8082 \\
  -e DATABASE_URL="postgresql://postgres:postgres@postgres:5432/quadrant_vms" \\
  -e RUST_LOG=info \\
  quadrant-vms/coordinator:latest
```

### Example: Run Stream Node

```bash
docker run -d \\
  --name quadrant-stream-node \\
  -p 8080:8080 \\
  -v $(pwd)/data/hls:/data/hls \\
  -e S3_ENDPOINT="http://minio:9000" \\
  -e S3_ACCESS_KEY="minio" \\
  -e S3_SECRET_KEY="minio123" \\
  -e RUST_LOG=info \\
  quadrant-vms/stream-node:latest
```

### Example: Run Operator UI

```bash
docker run -d \\
  --name quadrant-operator-ui \\
  -p 8091:8091 \\
  -e COORDINATOR_URL="http://coordinator:8082" \\
  -e STREAM_NODE_URL="http://stream-node:8080" \\
  -e RUST_LOG=info \\
  quadrant-vms/operator-ui:latest
```

## Environment Variables

All services support the following common environment variables:

- `RUST_LOG` - Log level (trace, debug, info, warn, error)
- `*_ADDR` - Bind address (e.g., `COORDINATOR_ADDR=0.0.0.0:8082`)

Service-specific environment variables are documented in [SERVICES.md](SERVICES.md).

## Health Checks

All services expose a `/health` endpoint that returns 200 OK when healthy.

Health check configuration in Docker:
```dockerfile
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \\
    CMD wget --no-verbose --tries=1 --spider http://localhost:PORT/health || exit 1
```

## Multi-Stage Build Details

### Stage 1: Builder

```dockerfile
FROM rust:1.84-alpine AS builder
RUN apk add --no-cache musl-dev pkgconfig openssl-dev ca-certificates
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY crates ./crates
RUN cargo build --release -p SERVICE_NAME --bin SERVICE_NAME
```

### Stage 2: Runtime

```dockerfile
FROM alpine:3.20
RUN apk add --no-cache ca-certificates libgcc [additional-deps]
RUN addgroup -g 1000 vms && adduser -D -u 1000 -G vms vms
COPY --from=builder /build/target/release/SERVICE_NAME /usr/local/bin/SERVICE_NAME
USER vms
CMD ["/usr/local/bin/SERVICE_NAME"]
```

## Docker Compose

See `docker-compose.yml` in the project root for a complete stack deployment example.

## Optimization Tips

### Build Cache

The Dockerfiles are optimized for layer caching:
1. Dependencies are installed first (rarely change)
2. Source code is copied last (changes frequently)
3. Cargo build uses workspace for shared dependencies

### Image Size

- Use Alpine Linux for minimal base image
- Multi-stage builds discard build artifacts
- Only include necessary runtime dependencies
- No debug symbols in release builds

### Security

- Non-root user for all services
- Minimal attack surface with Alpine
- Health checks for container orchestration
- No unnecessary tools in runtime image

## Troubleshooting

### Build Fails with "edition2024 required"

Make sure you're using Rust 1.84 or later:
```bash
docker pull rust:1.84-alpine
```

### GStreamer Not Found (stream-node)

The Dockerfile installs all required GStreamer packages. If you see errors, check:
```bash
docker run --rm quadrant-vms/stream-node:latest sh -c "gst-inspect-1.0 --version"
```

### FFmpeg Not Found (recorder-node)

Check FFmpeg installation:
```bash
docker run --rm quadrant-vms/recorder-node:latest sh -c "ffmpeg -version"
```

### Frontend Build Fails (operator-ui)

Ensure Node.js dependencies are installed:
```bash
cd crates/operator-ui/frontend
npm install
```

## GitHub Container Registry (GHCR) Push

```bash
# Login to GHCR
echo $GHCR_TOKEN | docker login ghcr.io -u $GHCR_USER --password-stdin

# Tag and push
for service in coordinator stream-node recorder-node admin-gateway ai-service auth-service device-manager alert-service playback-service operator-ui; do
  docker tag quadrant-vms/$service:latest ghcr.io/$GHCR_USER/quadrant-vms-$service:latest
  docker push ghcr.io/$GHCR_USER/quadrant-vms-$service:latest
done
```

## See Also

- [README.md](README.md) - Project overview
- [SERVICES.md](SERVICES.md) - Service documentation
- [HA_DEPLOYMENT.md](docs/HA_DEPLOYMENT.md) - High availability deployment

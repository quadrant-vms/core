# Quadrant VMS - Claude Code Guide

## Project Overview

**Quadrant VMS** is a modular, cluster-ready Video Management System built in Rust.

### Architecture

This is a **Cargo workspace** with multiple crates:

1. **stream-node** (`crates/stream-node/`)
   - RTSP video stream ingestion
   - HLS transcoding (TS/fMP4 formats)
   - S3 storage upload with fallback
   - Entry point: `crates/stream-node/src/main.rs`

2. **coordinator** (`crates/coordinator/`)
   - Lease-based job scheduler
   - In-memory lease store (PostgreSQL/Redis planned)
   - REST API for lease management
   - Entry point: `crates/coordinator/src/main.rs`

3. **admin-gateway** (`crates/admin-gateway/`)
   - REST API facade
   - Acquires leases from coordinator
   - Launches stream-node workers
   - Manages worker lifecycle via HTTP
   - Entry point: `crates/admin-gateway/src/main.rs`

4. **common** (`crates/common/`)
   - Shared utilities and types
   - Contract definitions for inter-service communication

5. **telemetry** (`crates/telemetry/`)
   - Logging and monitoring infrastructure

### Key Files

- `Cargo.toml` - Workspace manifest
- `Makefile` - Docker Compose and cargo shortcuts
- `tests/gateway_coordinator.rs` - End-to-end integration tests
- `.env` / `example.env` - Configuration (not in git)
- `profiles/` - Deployment profiles (compose/desktop/k8s)
- `data/hls/` - HLS output directory (runtime generated)

### Development Workflow

```bash
# Run tests
make test  # or: cargo test

# Launch stream-node locally
make launch  # or: HLS_ROOT=./data/hls cargo run -p stream-node

# Docker Compose stack
make init-dc
make status-dc
```

### Common Tasks

**Adding a new feature to a crate:**
1. Read relevant files in `crates/<crate-name>/src/`
2. Check `crates/<crate-name>/Cargo.toml` for dependencies
3. Run `cargo test` to verify changes
4. Update integration tests in `tests/` if needed

**Debugging inter-service communication:**
1. Check contract definitions in `crates/common/src/`
2. Review routes in `crates/*/src/routes.rs`
3. Check integration tests in `tests/gateway_coordinator.rs`

**Modifying lease logic:**
1. Focus on `crates/coordinator/src/store.rs`
2. Check `crates/admin-gateway/src/coordinator.rs` for client side
3. Run integration tests to verify

### Testing

- Unit tests are co-located with source files
- Integration tests are in `tests/` directory
- Run `cargo test` to execute all tests
- CI-friendly test suite covers lease store, router contracts, and end-to-end flows

### Dependencies

- Uses standard Rust ecosystem (tokio, axum, etc.)
- FFmpeg/GStreamer for video processing
- S3-compatible storage for media

### Ignore Patterns

See `.claudeignore` for files to skip during context gathering to save tokens.

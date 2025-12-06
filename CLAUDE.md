# Quadrant VMS - Claude Code Guide

## Project Overview

**Quadrant VMS** is a modular, cluster-ready Video Management System built in Rust.

## 🚨 MANDATORY Development Rules

### 1. ALWAYS Commit & Push After Completing Work
- **CRITICAL**: After completing ANY work (features, fixes, optimizations, tests), you MUST:
  1. Run `git add` to stage changes
  2. Run `git commit` with descriptive message
  3. Run `git push` to remote repository
- **NEVER FORGET THIS STEP** - The user should not need to remind you
- This is the FIRST and MOST IMPORTANT rule
- If you complete work without committing and pushing, you have failed

### 2. Always Update README.md After Feature Completion
- **CRITICAL**: After completing ANY feature or milestone, update `README.md`
- Move completed items from "🔜 In Progress" to "✅ Implemented"
- Add specific details about what was implemented
- Add new sub-items if the feature introduced new components
- Keep README.md as the single source of truth for project status

### 3. Update This File (CLAUDE.md) When Needed
- **Keep this guide current**: If project structure changes, update this file
- Add new crates to the Architecture section
- Update Common Tasks if new patterns emerge
- Add new development workflows as they're established
- This file should evolve with the project

### 4. Self-Contained Context for New Sessions
- **Goal**: User should NEVER need to repeat instructions across chat sessions
- All project context, rules, and workflows must be in CLAUDE.md
- New Claude sessions should read this file first to understand everything
- If you find yourself asking the user to repeat something, add it here

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
   - Lease types, stream types, and recording types

5. **recorder-node** (`crates/recorder-node/`)
   - FFmpeg-based recording pipeline (RTSP/HLS sources → MP4/HLS/MKV)
   - Recording job management and lifecycle
   - Automatic metadata extraction using ffprobe
   - REST API for recording operations (start/stop/list)
   - Entry point: `crates/recorder-node/src/main.rs`
   - **Status**: Pipeline implementation complete

6. **telemetry** (`crates/telemetry/`)
   - Logging and monitoring infrastructure

### Key Files

- `Cargo.toml` - Workspace manifest
- `Makefile` - Docker Compose and cargo shortcuts
- `tests/gateway_coordinator.rs` - End-to-end integration tests
- `.env` / `example.env` - Configuration (not in git)
- `profiles/` - Deployment profiles (compose/desktop/k8s)
- `data/hls/` - HLS output directory (runtime generated)

## Development Workflow

### Standard Development Cycle

When implementing a new feature, follow this sequence:

1. **Plan & Explore**
   - Read README.md to understand current progress
   - Explore relevant crates and existing patterns
   - Use Task tool with Explore agent for codebase discovery

2. **Implement**
   - Create or modify files in appropriate crate
   - Follow existing code patterns and architecture
   - Add unit tests co-located with code
   - Add integration tests in `tests/` if needed

3. **Test**
   - Run `cargo test` to ensure all tests pass
   - Fix any warnings or compilation errors
   - Verify integration tests pass

4. **Document & Commit** ⚠️ NEVER SKIP THIS STEP
   - **MANDATORY**: Always commit and push after completing work
   - Update README.md with completed feature (if applicable)
   - Update CLAUDE.md if structure changed
   - Run: `git add <files>`
   - Run: `git commit -m "descriptive message"`
   - Run: `git push`
   - **Failure to commit and push means the work is incomplete**

### Commands

```bash
# Run tests
make test  # or: cargo test

# Launch stream-node locally
make launch  # or: HLS_ROOT=./data/hls cargo run -p stream-node

# Docker Compose stack
make init-dc
make status-dc
```

### Current Development Priority

**Recently Completed**: Recorder Pipeline Implementation
- ✅ recorder-node crate structure created
- ✅ Recording job manager with lifecycle management
- ✅ REST API (start/stop/list recordings)
- ✅ FFmpeg-based recording pipeline (RTSP/HLS → MP4/HLS/MKV)
- ✅ Metadata extraction using ffprobe
- ✅ Storage path tracking and file management

**Next Feature**: Coordinator & Gateway Integration for Recorder (per README.md)
- 🔜 Integration with coordinator for recorder lease management
- 🔜 Admin-gateway integration for recorder worker management
- 🔜 End-to-end tests for recorder workflow

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

---

## Quick Start for New Claude Sessions

1. **Read this file first** - Contains all context and rules
2. **Check README.md** - See current implementation status
3. **Run `cargo test`** - Verify current state
4. **Continue from "Current Development Priority"** section above
5. **Remember**: Always update README.md when completing features

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

### 2. Always Update Documentation After Feature Completion
- **CRITICAL**: After completing ANY feature or milestone, update documentation:
  - Update `README.md` - Keep high-level status current
  - Update `SERVICES.md` - Add detailed service features and configuration
  - Move completed items from "🔜 Upcoming Features" to appropriate sections
  - Add specific details about what was implemented
- README.md is the high-level overview, SERVICES.md has detailed service documentation

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

### 5. 🔒 MANDATORY Reliability & Safety Rules

**Context**: After a comprehensive reliability audit (see RELIABILITY_AUDIT.md), 120 critical issues were identified that could cause cascading failures similar to Cloudflare-style outages. ALL code must follow these rules to prevent panics, crashes, and service failures.

#### 5.1 NEVER Use `.unwrap()` or `.expect()` in Production Code

**❌ FORBIDDEN**:
```rust
// WRONG: These will panic and crash the service
let value = some_option.unwrap();
let parsed = Uuid::parse_str(&id).unwrap();
let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
let data = vec[index];  // Index without bounds checking
```

**✅ REQUIRED**:
```rust
// CORRECT: Use pattern matching or return errors
let value = some_option.ok_or_else(|| anyhow!("value missing"))?;
let parsed = validation::parse_uuid(&id, "field_name")?;
let timestamp = validation::safe_unix_timestamp();
let data = vec.get(index).ok_or_else(|| anyhow!("index out of bounds"))?;
```

**Exceptions**: `.unwrap()` is ONLY allowed in:
- Test code (`#[cfg(test)]`)
- Code with SAFETY comments explaining why panic is impossible
- Use `.expect("BUG: explain why this is guaranteed to be Some/Ok")` for internal invariants

#### 5.2 ALWAYS Validate External Inputs

**External inputs include**: HTTP request bodies/headers, environment variables, config files, database data, file paths, user-provided regex, URLs, and any data from external sources.

**❌ FORBIDDEN**:
```rust
// WRONG: No validation, allows attacks
let id = req.id;  // Could be 10GB string → OOM
let uri = req.uri;  // Could contain shell metacharacters → command injection
let path = PathBuf::from(req.path);  // Could be "../../etc/passwd" → path traversal
```

**✅ REQUIRED**:
```rust
use common::validation;

// CORRECT: Validate all inputs
validation::validate_id(&req.id, "stream_id")?;
validation::validate_uri(&req.uri, "source_uri")?;
validation::validate_path_components(&PathBuf::from(req.path), Some(base_dir), "file_path")?;
validation::validate_regex_pattern(&req.pattern)?;
```

**Available validation functions** (see `crates/common/src/validation.rs`):
- `validate_id()` - Resource IDs (max 256 bytes, no path traversal)
- `validate_name()` - Names (max 512 bytes)
- `validate_uri()` - URIs (max 4KB, no shell metacharacters)
- `validate_path()` / `validate_path_components()` - File paths (prevent traversal)
- `validate_email()` - Email addresses
- `validate_regex_pattern()` - Regex (prevent ReDoS attacks)
- `validate_port()` - Port numbers (1-65535)
- `validate_range()` - Numeric range validation
- `parse_uuid()` - Safe UUID parsing
- `safe_unix_timestamp()` - Safe time operations

#### 5.3 ALWAYS Use Safe Time Operations

**❌ FORBIDDEN**:
```rust
// WRONG: Panics if system clock is before 1970 (happens in containers)
let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
```

**✅ REQUIRED**:
```rust
use common::validation;

// CORRECT: Safe fallback for clock errors
let now = validation::safe_unix_timestamp();  // Returns 0 and logs warning on error
```

#### 5.4 ALWAYS Use Bounded Collections

**❌ FORBIDDEN**:
```rust
// WRONG: Unbounded HashMap → OOM if attacker creates 1M sessions
pub struct SessionManager {
    sessions: HashMap<String, Session>,  // No size limit!
}
```

**✅ REQUIRED**:
```rust
// CORRECT: Enforce limits
const MAX_CONCURRENT_SESSIONS: usize = 10_000;

pub struct SessionManager {
    sessions: HashMap<String, Session>,
    max_sessions: usize,
}

impl SessionManager {
    pub fn add_session(&mut self, id: String, session: Session) -> Result<()> {
        if self.sessions.len() >= self.max_sessions {
            return Err(anyhow!("Maximum concurrent sessions ({}) exceeded", self.max_sessions));
        }
        self.sessions.insert(id, session);
        Ok(())
    }
}
```

**Required limits**:
- `MAX_CONCURRENT_RECORDINGS` - Limit active recordings
- `MAX_CONCURRENT_STREAMS` - Limit active streams
- `MAX_CONCURRENT_PLAYBACK_SESSIONS` - Limit playback sessions
- `MAX_MQTT_CLIENTS` - Limit MQTT client cache (use LRU eviction)

#### 5.5 NEVER Use `std::sync::Mutex` in Async Code

**❌ FORBIDDEN**:
```rust
use std::sync::Mutex;

// WRONG: Poisoned mutex causes cascading failures
let clients = self.clients.lock().unwrap();  // Panics if poisoned
```

**✅ REQUIRED**:
```rust
use tokio::sync::RwLock;

// CORRECT: Use async-aware locks
let clients = self.clients.read().await;  // Never panics from poisoning
```

**Why**: `std::sync::Mutex` can become "poisoned" if any thread panics while holding the lock. This causes ALL future `.lock().unwrap()` calls to panic, creating cascading failures across the entire service cluster.

#### 5.6 ALWAYS Handle Errors in Spawned Tasks

**❌ FORBIDDEN**:
```rust
// WRONG: Panic in spawned task is silent, resources leak
tokio::spawn(async move {
    pipeline.run().await.unwrap();  // Silent crash
});
```

**✅ REQUIRED**:
```rust
// CORRECT: Log errors, don't panic
tokio::spawn(async move {
    if let Err(e) = pipeline.run().await {
        tracing::error!(error = %e, "pipeline failed");
        // Optionally: send error to parent via channel
    }
});
```

#### 5.7 ALWAYS Sanitize Command Arguments

**❌ FORBIDDEN**:
```rust
// WRONG: Command injection vulnerability
let output = Command::new("ffmpeg")
    .arg("-i")
    .arg(user_provided_uri)  // Could be: "file.mp4; rm -rf /"
    .output()?;
```

**✅ REQUIRED**:
```rust
// CORRECT: Validate URI first
validation::validate_uri(&user_provided_uri, "source_uri")?;  // Blocks shell metacharacters
let output = Command::new("ffmpeg")
    .arg("-i")
    .arg(user_provided_uri)
    .output()?;
```

#### 5.8 Test Code CAN Use `.unwrap()`

**✅ ALLOWED in tests**:
```rust
#[test]
fn test_example() {
    let value = some_function().unwrap();  // OK in tests
    assert_eq!(value, expected);
}
```

#### Quick Checklist for New Code

Before committing ANY code, verify:
- [ ] No `.unwrap()` or `.expect()` in production paths (only in tests or with SAFETY comments)
- [ ] All external inputs validated using `common::validation::*`
- [ ] All collections have size limits or LRU eviction
- [ ] All time operations use `validation::safe_unix_timestamp()`
- [ ] No `std::sync::Mutex` in async code (use `tokio::sync::RwLock`)
- [ ] All spawned tasks handle errors gracefully
- [ ] All command arguments sanitized against injection
- [ ] All file paths checked for traversal attacks
- [ ] All regex patterns validated against ReDoS

**Failure to follow these rules WILL cause production outages.**

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
   - Centralized Prometheus metrics registry

7. **ai-service** (`crates/ai-service/`)
   - AI plugin system with extensible architecture
   - Plugin trait for custom AI model integrations
   - Built-in mock object detection plugin
   - REST API for AI task management
   - Coordinator lease integration
   - Entry point: `crates/ai-service/src/main.rs`
   - **Status**: Core plugin architecture complete

8. **auth-service** (`crates/auth-service/`)
   - Centralized authentication and authorization service
   - JWT-based authentication with API token support
   - Role-Based Access Control (RBAC) system
   - Multi-tenancy support with resource quotas
   - Audit logging for security compliance
   - PostgreSQL-backed user/role/permission storage
   - Entry point: `crates/auth-service/src/main.rs`
   - **Status**: Core auth system complete (OIDC/OAuth2 pending)

9. **device-manager** (`crates/device-manager/`)
   - Camera and device management system
   - Device onboarding and RTSP probing
   - Automated health monitoring
   - Multi-protocol support (RTSP, ONVIF, HTTP, RTMP, WebRTC)
   - PostgreSQL-backed device storage
   - REST API for device operations
   - Entry point: `crates/device-manager/src/main.rs`
   - **Status**: Core device management complete

10. **alert-service** (`crates/alert-service/`)
   - Event-driven alert and automation system
   - Rule engine with condition-based triggering
   - Multi-channel notifications (email, webhook, MQTT)
   - Alert suppression and rate limiting
   - PostgreSQL-backed alert storage
   - Entry point: `crates/alert-service/src/main.rs`
   - **Status**: Complete (with WebRTC support)

11. **playback-service** (`crates/playback-service/`)
   - Multi-protocol playback delivery (HLS, RTSP)
   - Playback session management with state tracking
   - Live stream and recording playback
   - Time-based navigation with seek support
   - HLS file serving and RTSP proxy
   - PostgreSQL-backed session storage (optional)
   - REST API for playback operations
   - Entry point: `crates/playback-service/src/main.rs`
   - **Status**: Complete (with WebRTC support)

12. **operator-ui** (`crates/operator-ui/`)
   - Web-based operator dashboard for VMS monitoring and management
   - React + Vite frontend with JavaScript
   - Real-time WebSocket updates for live data
   - Multi-view interface: Dashboard, Devices, Streams, Recordings, AI Tasks, Alerts, Incidents
   - Incident workflow system with notes and timeline
   - Entry point: `crates/operator-ui/src/main.rs`
   - Frontend: `crates/operator-ui/frontend/`
   - **Status**: Complete

### Key Files

- `Cargo.toml` - Workspace manifest
- `Makefile` - Docker Compose and cargo shortcuts
- `README.md` - High-level project overview and quick start guide
- `SERVICES.md` - Detailed documentation for each service (features, API, configuration)
- `CLAUDE.md` - Development guide for Claude Code (this file)
- `tests/gateway_coordinator.rs` - End-to-end integration tests
- `tests/ai_service.rs` - AI service integration tests
- `tests/operator_ui.rs` - Operator UI integration tests
- `.env` / `example.env` - Configuration (not in git)
- `profiles/` - Deployment profiles (compose/desktop/k8s)
- `data/hls/` - HLS output directory (runtime generated)
- `docs/` - Additional documentation (AUTHENTICATION.md, HA_DEPLOYMENT.md, GPU_ACCELERATION.md)

## Development Workflow

### Standard Development Cycle

When implementing a new feature, follow this sequence:

1. **Plan & Explore**
   - Read README.md for high-level project status
   - Read SERVICES.md for detailed service documentation
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
   - Update README.md with high-level status (if applicable)
   - Update SERVICES.md with detailed feature documentation (if applicable)
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


**Recently Completed**: Full Kubernetes Deployment Support
- ✅ Complete Kubernetes manifests for all 10 services
- ✅ Helm chart with configurable values for easy installation
- ✅ HorizontalPodAutoscaler for auto-scaling under load
- ✅ PodDisruptionBudget for high availability during cluster maintenance
- ✅ NetworkPolicy for secure pod-to-pod communication
- ✅ RBAC with ServiceAccount, Roles, and RoleBindings
- ✅ ServiceMonitor for Prometheus Operator integration
- ✅ Ingress manifest with TLS support for external access
- ✅ Production-ready configuration with resource limits and health checks
- ✅ Comprehensive Kubernetes documentation

**Previous Milestones**:
- **Operator UI Web Dashboard**: React-based dashboard with real-time WebSocket updates
- **Edge Caching for Playback Service**: LRU cache with configurable TTL and size limits
- **WebRTC Playback Support**: WHEP protocol implementation for ultra-low-latency streaming
- **LL-HLS Support**: Low-latency HLS with partial segments and blocking playlist reload
- **Time-Axis Preview Thumbnails**: Evenly-spaced thumbnail generation for video scrubbing
- **Distributed Observability**: Centralized logging, OpenTelemetry tracing, and SLO metrics

**All Core Features Complete**: The project is now production-ready with full Kubernetes support

### Common Tasks
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
- FFmpeg for video processing (transcoding, recording, probing)
- S3-compatible storage for media

### Ignore Patterns

See `.claudeignore` for files to skip during context gathering to save tokens.

---

## Quick Start for New Claude Sessions

1. **Read this file first** - Contains all context and rules
2. **Check README.md** - See high-level project status and features
3. **Check SERVICES.md** - See detailed service documentation
4. **Run `cargo test`** - Verify current state
5. **Continue from "Current Development Priority"** section above
6. **Remember**: Always update documentation (README.md and SERVICES.md) when completing features

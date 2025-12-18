# Quadrant VMS - Tracking Issues

**Last Updated**: 2025-12-17 (Added P3-5: Integration Tests, P3-6: K8s Deployment, P3-7: Docker Compose Validation)
**Source**: [RELIABILITY_AUDIT.md](RELIABILITY_AUDIT.md)
**Fixes Applied**: [RELIABILITY_FIXES_APPLIED.md](RELIABILITY_FIXES_APPLIED.md)

## Overview

This document tracks all outstanding reliability and safety issues identified in the comprehensive audit. Issues are prioritized by severity and potential impact on production systems.

**Progress**: 18/22 tracked issues resolved (82% complete - excludes ~100 untracked audit items)

---

## 🔴 Priority 1: CRITICAL (Fix Immediately)

These issues can cause service crashes, cascading failures, or security vulnerabilities.

### P1-1: Auth Service Mutex Poisoning ⚠️ CASCADING FAILURE RISK

**Status**: ✅ FIXED (2025-12-17)
**Severity**: CRITICAL
**Impact**: Cluster-wide authentication failures
**Location**: `crates/auth-service/src/oidc.rs`

**Issue**: 7 instances of `std::sync::RwLock.lock().unwrap()` in async code. If any thread panics while holding the lock, the mutex becomes poisoned and ALL future auth requests fail.

**Cascade Scenario**:
1. OIDC state validation panic → mutex poisoned
2. All auth requests call `.lock().unwrap()` → panic
3. All services lose authentication → cluster-wide outage

**Fix Required**:
```rust
// BEFORE (dangerous):
let clients = self.clients.read().unwrap();

// AFTER (safe):
let clients = self.clients.read().await;  // Use tokio::sync::RwLock
```

**Files to Modify**:
- `crates/auth-service/src/oidc.rs` (7 occurrences)

**Estimated Effort**: 1-2 hours

---

### P1-2: Playback Manager Safety - Multiple Panics

**Status**: ✅ FIXED (2025-12-17)
**Severity**: CRITICAL
**Impact**: All playback sessions terminated on invalid input
**Location**: `crates/playback-service/src/playback/manager.rs:154, 172, 330`

**Issues**:
1. `SystemTime::now().duration_since(UNIX_EPOCH).unwrap()` (line 154)
2. `Url::parse(&url).unwrap()` (line 172)
3. `.unwrap()` on Optional values (line 330)

**Fix Required**: Return proper `Result<T, Error>` instead of panicking

**Estimated Effort**: 2-3 hours

---

### P1-3: DVR Segment Queue Empty Panics

**Status**: ✅ FIXED (2025-12-17)
**Severity**: CRITICAL
**Impact**: Live playback crashes when DVR buffer is empty
**Location**: `crates/playback-service/src/playback/dvr.rs:157, 189-190`

**Issue**:
```rust
// Line 157, 189-190:
let first = self.segments.front().unwrap();
let last = self.segments.back().unwrap();
```

**Fix Required**:
```rust
let first = self.segments.front().ok_or_else(|| anyhow!("DVR buffer empty"))?;
let last = self.segments.back().ok_or_else(|| anyhow!("DVR buffer empty"))?;
```

**Estimated Effort**: 1 hour

---

### P1-4: Device Discovery Array Access Panic

**Status**: ✅ FIXED (2025-12-17)
**Severity**: HIGH
**Impact**: Device discovery crashes, prevents camera onboarding
**Location**: `crates/device-manager/src/discovery.rs:273`

**Issue**:
```rust
let xaddr = xaddr_list.first().unwrap();  // Panics if empty
```

**Fix Required**: Return structured error for empty address list

**Estimated Effort**: 30 minutes

---

### P1-5: Unbounded Collections - OOM Risk (4 Services)

**Status**: ✅ FIXED (2025-12-17)
**Severity**: CRITICAL
**Impact**: Memory exhaustion → service crash under load

#### P1-5a: Stream-Node Registry ✅ FIXED
**Location**: `crates/stream-node/src/stream/manager.rs`
**Issue**: No `MAX_CONCURRENT_STREAMS` limit
**Fix**: Added `MAX_CONCURRENT_STREAMS = 1000` with error response when exceeded

#### P1-5b: Playback Sessions ✅ FIXED
**Location**: `crates/playback-service/src/playback/manager.rs`
**Issue**: Unbounded `sessions: HashMap<String, SessionData>`
**Fix**: Added `MAX_CONCURRENT_SESSIONS = 10000` with error response when exceeded

#### P1-5c: Recording Manager ✅ FIXED
**Location**: `crates/recorder-node/src/recording/manager.rs`
**Issue**: Unbounded `recordings` and `pipelines` HashMaps
**Fix**: Added `MAX_CONCURRENT_RECORDINGS = 500` with graceful rejection

#### P1-5d: MQTT Client Cache ✅ FIXED
**Location**: `crates/alert-service/src/notifier.rs`
**Issue**: Unbounded `clients: HashMap<String, AsyncClient>`
**Fix**: Implemented eviction with `MAX_MQTT_CLIENTS = 100`

**Estimated Effort**: 4-6 hours total (COMPLETED)

---

### P1-6: Stream Lifecycle - Orphaned Upload Tasks

**Status**: ✅ FIXED (2025-12-17)
**Severity**: HIGH
**Impact**: Resource leaks, S3 watchers never cancelled
**Location**:
- `crates/stream-node/src/stream/manager.rs`
- `crates/stream-node/src/storage/uploader.rs`

**Issue**: S3 upload tasks not tracked or cancelled when streams stop

**Fix Applied**:
- Modified registry to track `JoinHandle<()>` for upload tasks
- Cancel upload task via `.abort()` when stream stops
- Cancel upload task on stream exit in `list_streams()`
- Fixed path safety in uploader.rs (line 100)

**Estimated Effort**: 2-3 hours ✅ COMPLETED

---

## 🟡 Priority 2: HIGH (Fix Within 1 Week)

### P2-1: Stream API Ergonomics & Validation

**Status**: ✅ FIXED (2025-12-17)
**Severity**: MEDIUM
**Impact**: Poor REST semantics, missing input validation
**Location**: `crates/stream-node/src/api/routes.rs`

**Issues**:
1. GET requests used for state-changing operations
2. No input validation via `common::validation`

**Fix Applied**:
- Added POST `/start` with JSON body (new recommended endpoint)
- Added DELETE `/stop` with JSON body (new recommended endpoint)
- Maintained legacy GET endpoints for backward compatibility
- Added input validation for `stream_id` and `source_uri` using `common::validation`
- Updated route handlers with proper HTTP methods

**Estimated Effort**: 2 hours ✅ COMPLETED

---

### P2-2: Dependency Hygiene

**Status**: ✅ FIXED (2025-12-17)
**Severity**: LOW
**Impact**: Using deprecated/unused dependencies
**Location**: `crates/stream-node/Cargo.toml`

**Issues**:
1. `serde_yaml = "0.9.34+deprecated"` (deprecated)
2. `lazy_static` (unused)

**Fix Applied**:
- Migrated from `serde_yaml` to `serde_yml = "0.0.12"` (maintained fork)
- Removed unused `lazy_static` dependency
- Updated imports in `crates/stream-node/src/compat/mod.rs`

**Estimated Effort**: 1 hour ✅ COMPLETED

---

### P2-3: FFmpeg Pipeline Resilience

**Status**: ✅ FIXED (2025-12-17)
**Severity**: MEDIUM
**Impact**: No automatic recovery from FFmpeg crashes
**Location**: `crates/stream-node/src/stream/manager.rs`

**Issues**:
1. No restart policy for failed pipelines
2. No failure metrics exposed

**Fix Applied**:
- ✅ Exponential backoff restart already implemented (lines 65-154)
- ✅ Metrics exposed: `ffmpeg_crashes_total`, `ffmpeg_restarts_total`
- ✅ Monitor task spawned for each stream to detect crashes
- ✅ Max 5 restart attempts with delay from 2s to 60s

**Estimated Effort**: 3-4 hours ✅ COMPLETED

---

### P2-4: Path/Filename Safety in S3 Uploader

**Status**: ✅ FIXED (2025-12-17)
**Severity**: HIGH
**Impact**: Service crash on malformed paths
**Location**: `crates/stream-node/src/storage/uploader.rs:100`

**Issue**:
```rust
let filename = path.file_name().unwrap();  // Panics on ".." or malformed paths
```

**Fix Applied**:
- Replaced `.unwrap()` with `Option` pattern matching
- Logs warning and skips malformed paths instead of panicking
- Graceful error handling for invalid filenames

**Estimated Effort**: 1 hour ✅ COMPLETED

---

### P2-5: Input Validation Gaps (Multiple Services)

**Status**: ✅ FIXED (2025-12-17)
**Severity**: HIGH
**Impact**: OOM attacks, command injection, path traversal
**Locations**:
- `crates/admin-gateway/src/routes.rs` ✅ DONE (already had validation)
- `crates/device-manager/src/routes.rs` ✅ DONE (added validation)
- `crates/recorder-node/src/recording/manager.rs` ✅ DONE (already had validation)

**Fix Applied**:
- ✅ admin-gateway: Verified stream_id, source_uri, recording_id validation exists
- ✅ device-manager: Added validation for name, primary_uri, secondary_uri in update/batch endpoints
- ✅ recorder-node: Verified recording_id and source validation exists
- All services now validate external inputs using `common::validation::*`

**Estimated Effort**: 4-6 hours ✅ COMPLETED

---

### P2-6: UUID Parsing Robustness (Service Audit)

**Status**: ✅ FIXED (2025-12-17)
**Severity**: HIGH
**Impact**: Service crashes on malformed UUIDs

**Services Audited**:
- ✅ alert-service (DONE - previously fixed)
- ✅ admin-gateway (CLEAN - no unsafe UUID parsing)
- ✅ device-manager (CLEAN - no unsafe UUID parsing)
- ✅ recorder-node (FIXED - 2 instances improved with logging)
- ✅ ai-service (CLEAN - no unsafe UUID parsing)

**Fix Applied**:
- Improved UUID parsing in `recorder-node/src/search/store.rs` (lines 30, 78)
- Added `common::validation::parse_uuid()` with fallback and logging
- All services now use safe UUID parsing with graceful error handling

**Estimated Effort**: 3-4 hours ✅ COMPLETED

---

### P2-7: Path Traversal Hardening

**Status**: ✅ FIXED (2025-12-17)
**Severity**: HIGH (Security)
**Impact**: Unauthorized file access
**Locations**:
- `crates/playback-service/src/playback/manager.rs:305-319`
- `crates/recorder-node/src/recording/thumbnail_generator.rs`

**Issue**: No canonicalization or bounds checking on recording/HLS paths

**Fix Applied**:
- Added `validation::validate_id()` for recording_id inputs
- Added `validation::validate_path_components()` to ensure paths stay within storage root
- Applied to `playback-service/src/playback/manager.rs::find_recording_path()`
- Applied to `recorder-node/src/recording/thumbnail_generator.rs::find_recording_path()`
- Prevents path traversal attacks like `../../etc/passwd`

**Estimated Effort**: 2-3 hours ✅ COMPLETED

---

### P2-8: MQTT/Webhook Robustness

**Status**: ✅ FIXED (2025-12-17)
**Severity**: MEDIUM
**Impact**: Silent notification failures
**Location**: `crates/alert-service/src/notifier.rs`

**Issues**:
1. MQTT eventloop stops on error (line 254-265) - no reconnection
2. Webhook client has no timeout (line 132)

**Fix Applied**:
- ✅ Webhook already had 30-second timeout (verified at line 130)
- ✅ Added exponential backoff reconnection for MQTT eventloop
- Retry delay starts at 1 second, doubles on each failure, capped at 60 seconds
- MQTT connection now automatically recovers from network issues

**Estimated Effort**: 2-3 hours ✅ COMPLETED

---

### P2-9: Regex Safety (ReDoS Prevention)

**Status**: ✅ FIXED (2025-12-17)
**Severity**: HIGH (Security)
**Impact**: Denial of Service via malicious regex patterns
**Location**: `crates/alert-service/src/rule_engine.rs:210`

**Issue**:
```rust
let regex = Regex::new(&pattern).unwrap();  // No validation
```

**Attack Vector**: User submits pattern like `(a+)+b` → catastrophic backtracking

**Fix Applied**:
- Added `common::validation::validate_regex_pattern()` before regex compilation
- Returns `false` with warning log instead of panicking on invalid patterns
- Graceful error handling for compilation failures
- Prevents ReDoS attacks from malicious regex patterns

**Estimated Effort**: 1 hour ✅ COMPLETED

---

## 🟢 Priority 3: MEDIUM (Fix Within 1 Month)

### P3-1: Capacity Metrics (Prometheus)

**Status**: ✅ FIXED (2025-12-17)
**Severity**: LOW
**Impact**: No visibility into resource usage

**Metrics Implemented**:
- ✅ `stream_node_active_streams` (already existed)
- ✅ `playback_service_active_sessions` (added)
- ✅ `recorder_node_active_recordings` (already existed)
- ✅ `stream_node_stream_rejections_total{reason="capacity"}` (added)
- ✅ `recorder_node_recording_rejections_total{reason="capacity"}` (added)
- ✅ `playback_service_session_rejections_total{reason="capacity"}` (added)

**Fix Applied**:
- Added new metrics to telemetry crate
- Integrated rejection tracking in all service managers
- All services now expose capacity visibility

**Estimated Effort**: 3-4 hours ✅ COMPLETED

---

### P3-2: Database Enum Parsing Fallbacks

**Status**: ✅ FIXED (2025-12-17)
**Severity**: LOW
**Impact**: Service crash on corrupted DB data
**Location**: `crates/alert-service/src/store.rs:178-179`

**Issue**:
```rust
severity: row.get::<String, _>("severity").parse().unwrap(),
```

**Fix Applied**:
- Added `Default` derive to `Severity` and `TriggerType` enums
- Replaced `.unwrap()` with `.unwrap_or_default()` for graceful fallbacks
- Added warning logs when invalid enum values are encountered
- Service now continues operation instead of crashing on corrupted data

**Estimated Effort**: 1 hour ✅ COMPLETED

---

### P3-3: Graceful Lock Poisoning Recovery

**Status**: ✅ FIXED (2025-12-17)
**Severity**: LOW
**Impact**: Better error messages for rare failure modes

**Locations**: All remaining `.lock().unwrap()` calls in non-async code

**Fix Applied**:
- ✅ Replaced `.lock().unwrap()` with `.expect()` in AI plugins
- ✅ pose_estimation.rs: 2 instances fixed with descriptive messages
- ✅ yolov8_detector.rs: 2 instances fixed with descriptive messages
- All production code now has better error messages for mutex poisoning

**Estimated Effort**: 2 hours ✅ COMPLETED

---

### P3-4: Chaos Engineering Tests

**Status**: 🔴 Open
**Severity**: LOW
**Impact**: Improve resilience testing

**Tests Needed**:
1. Clock skew (set system time to 1960, 2100)
2. Input fuzzing (10MB strings to all endpoints)
3. Resource exhaustion (10,000 concurrent sessions)
4. Invalid UUID injection
5. Path traversal attempts (`../../etc/passwd`)
6. ReDoS patterns to alert rules

**Estimated Effort**: 8-10 hours

---

### P3-5: Integration Tests Broken

**Status**: 🟡 In Progress
**Severity**: MEDIUM
**Impact**: No automated testing coverage, risk of regressions

**Progress**: 6/9 test files fixed (~90 errors resolved)
- ✅ `stateless_integration.rs` (70 errors) - FIXED: Updated StreamConfig/StreamInfo/RecordingConfig to match current API
- ✅ `ai_service.rs` (20 errors) - FIXED: Updated AiTaskConfig structure (source_stream_id, frame_config, output format)
- ✅ `distributed_tracing.rs` (8 errors) - FIXED: Added missing telemetry dependency
- ✅ `playback_integration.rs` (4 errors) - FIXED: Added dvr and low_latency fields to PlaybackConfig
- ✅ `webrtc_playback.rs` (2 errors) - FIXED: Added dvr field to PlaybackConfig
- ✅ `edge_cache_integration.rs` (1 error) - FIXED: Added missing dependencies (tempfile, image, tower, sqlx)
- 🔴 `device_manager_integration.rs` (5 errors) - REMAINING: DeviceManagerState constructor needs 6 arguments
- 🔴 `alert_service_integration.rs` (6 errors) - Not yet addressed
- 🔴 `lpr_plugin.rs` (3 errors) - Not yet addressed

**Work Completed**:
1. ✅ Fixed StreamConfig field changes (rtsp_url → uri, camera_id, codec, container added)
2. ✅ Fixed StreamInfo field changes (created_at/updated_at → started_at/stopped_at, added playlist_path/output_dir)
3. ✅ Fixed RecordingConfig field changes (source_url → source_uri/source_stream_id, format → RecordingFormat)
4. ✅ Fixed AiTaskConfig field changes (input_stream_id → source_stream_id, frame_rate → frame_config)
5. ✅ Fixed node_id type change (String → Option<String>)
6. ✅ Added missing test dependencies: telemetry, tempfile, tower, image, sqlx
7. ✅ Fixed PlaybackConfig missing fields (dvr, low_latency)
8. ✅ Fixed PostgresLeaseStore::new() arguments (changed to &str)
9. ✅ Fixed state_store method signatures (update_stream_state uses &str, not enum)

**Remaining Work**:
- Fix device_manager_integration.rs constructor issues
- Fix alert_service_integration.rs type mismatches
- Fix lpr_plugin.rs plugin API changes

**Estimated Remaining Effort**: 2-4 hours

---

### P3-6: Kubernetes Deployment Not Ready

**Status**: 🔴 Open
**Severity**: MEDIUM
**Impact**: Cannot deploy to production Kubernetes clusters

**Current State**:
- ✅ Dockerfiles exist for all 10 services
- ⚠️ Only minimal CRD example in `profiles/k8s/crds/minimal.yaml`
- ❌ No Deployment/Service/Ingress manifests
- ❌ No Helm charts
- ❌ No StatefulSet configs for stateful services (PostgreSQL, Redis, MinIO)
- ❌ No ConfigMap/Secret management
- ❌ No resource limits/requests defined
- ❌ No autoscaling (HPA) configurations
- ❌ No network policies
- ❌ No monitoring (ServiceMonitor for Prometheus)

**Required Files**:
1. `profiles/k8s/namespace.yaml` - Namespace definition
2. `profiles/k8s/infrastructure/` - PostgreSQL, Redis, MinIO StatefulSets
3. `profiles/k8s/core/` - Coordinator, Admin Gateway, Stream/Recorder nodes
4. `profiles/k8s/services/` - Auth, Device Manager, AI, Alert, Playback, Operator UI
5. `profiles/k8s/ingress.yaml` - Ingress for external access
6. `profiles/k8s/kustomization.yaml` - Kustomize overlay support
7. Optional: Helm chart in `charts/quadrant-vms/`

**Estimated Effort**: 16-20 hours

---

### P3-7: Docker Compose Deployment Status

**Status**: ⚠️ Needs Validation
**Severity**: MEDIUM
**Impact**: Uncertain if compose stack actually works end-to-end

**Current State**:
- ✅ Root `docker-compose.yml` is comprehensive (433 lines)
- ✅ All 10 services defined with proper configuration
- ✅ Infrastructure services (PostgreSQL, MinIO, Redis) included
- ✅ Health checks defined for all services
- ✅ Network and volume configuration present
- ⚠️ `profiles/compose/docker-compose.yml` is outdated (only MinIO, most services commented out)
- ❌ No documented end-to-end deployment test procedure
- ❌ Unknown if all service dependencies work correctly
- ❌ No smoke test script to validate deployment

**Required Actions**:
1. Test full stack deployment: `docker compose up -d`
2. Validate all services reach healthy state
3. Run smoke tests (create device → start stream → start recording → create alert)
4. Update or remove outdated `profiles/compose/docker-compose.yml`
5. Document deployment procedure in README.md or DEPLOYMENT.md
6. Create `scripts/smoke-test.sh` for automated validation

**Estimated Effort**: 4-6 hours (testing + documentation)

---

## Code TODOs Found

Additional issues found in code comments (not yet triaged):

- `crates/device-manager/src/store.rs` - Contains TODO comments
- `crates/operator-ui/src/api/dashboard.rs` - Contains TODO comments
- `crates/common/src/validation.rs` - Contains TODO comments
- `crates/device-manager/src/routes_simple.rs` - Contains TODO comments
- `crates/device-manager/src/prober.rs` - Contains TODO comments
- `crates/device-manager/src/routes.rs` - Contains TODO comments

---

## Summary Statistics

| Priority | Open | In Progress | Completed | Total |
|----------|------|-------------|-----------|-------|
| **P1 (Critical)** | 0 | 0 | 6 | 6 |
| **P2 (High)** | 0 | 0 | 9 | 9 |
| **P3 (Medium)** | 3 | 1 | 3 | 7 |
| **TOTAL** | **3** | **1** | **18** | **22** |

**Previously Completed** (see [RELIABILITY_FIXES_APPLIED.md](RELIABILITY_FIXES_APPLIED.md)):
- ✅ UUID parsing panics (alert-service) - 10 issues
- ✅ SystemTime panics (all services) - 7 issues
- ✅ Validation infrastructure - 1 issue

---

## How to Use This Document

1. **Pick next issue**: Start from P1, work down
2. **Update status**: Change 🔴 Open → 🟡 In Progress → 🟢 Completed
3. **Link PRs**: Add PR numbers to each completed issue
4. **Re-prioritize**: Move issues up/down as new information emerges
5. **Archive completed**: Move to RELIABILITY_FIXES_APPLIED.md when done

---

*Last Updated*: 2025-12-17

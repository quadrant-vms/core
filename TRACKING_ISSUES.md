# Quadrant VMS - Tracking Issues

**Last Updated**: 2025-12-17
**Source**: [RELIABILITY_AUDIT.md](RELIABILITY_AUDIT.md)
**Fixes Applied**: [RELIABILITY_FIXES_APPLIED.md](RELIABILITY_FIXES_APPLIED.md)

## Overview

This document tracks all outstanding reliability and safety issues identified in the comprehensive audit. Issues are prioritized by severity and potential impact on production systems.

**Progress**: 26/120 issues resolved (22% complete)

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

**Status**: 🔴 Open
**Severity**: HIGH
**Impact**: Resource leaks, S3 watchers never cancelled
**Location**:
- `crates/stream-node/src/stream/manager.rs`
- `crates/stream-node/src/storage/uploader.rs`

**Issue**: S3 upload tasks not tracked or cancelled when streams stop

**Fix Required**: Track `JoinHandle`s and cancel on stream stop

**Estimated Effort**: 2-3 hours

---

## 🟡 Priority 2: HIGH (Fix Within 1 Week)

### P2-1: Stream API Ergonomics & Validation

**Status**: 🔴 Open
**Severity**: MEDIUM
**Impact**: Poor REST semantics, missing input validation
**Location**: `crates/stream-node/src/api/routes.rs`

**Issues**:
1. GET requests used for state-changing operations
2. No input validation via `common::validation`

**Fix Required**:
- Change `/start` → POST with JSON body
- Change `/stop` → DELETE with JSON body
- Add `validation::validate_id()`, `validation::validate_uri()`

**Estimated Effort**: 2 hours

---

### P2-2: Dependency Hygiene

**Status**: 🔴 Open
**Severity**: LOW
**Impact**: Using deprecated/unused dependencies
**Location**: `crates/stream-node/Cargo.toml`

**Issues**:
1. `serde_yaml = "0.9.34+deprecated"` (deprecated)
2. `lazy_static` (unused)

**Fix Required**: Migrate to maintained crate, remove unused deps

**Estimated Effort**: 1 hour

---

### P2-3: FFmpeg Pipeline Resilience

**Status**: 🔴 Open
**Severity**: MEDIUM
**Impact**: No automatic recovery from FFmpeg crashes
**Location**: `crates/stream-node/src/stream/manager.rs`

**Issues**:
1. No restart policy for failed pipelines
2. No failure metrics exposed

**Fix Required**:
- Add exponential backoff retry
- Add Prometheus metrics: `ffmpeg_crashes_total`, `ffmpeg_restarts_total`

**Estimated Effort**: 3-4 hours

---

### P2-4: Path/Filename Safety in S3 Uploader

**Status**: 🔴 Open
**Severity**: HIGH
**Impact**: Service crash on malformed paths
**Location**: `crates/stream-node/src/storage/uploader.rs:100`

**Issue**:
```rust
let filename = path.file_name().unwrap();  // Panics on ".." or malformed paths
```

**Fix Required**: Sanitize S3 object keys, handle path errors gracefully

**Estimated Effort**: 1 hour

---

### P2-5: Input Validation Gaps (Multiple Services)

**Status**: 🟡 Partial (validation utilities exist, enforcement needed)
**Severity**: HIGH
**Impact**: OOM attacks, command injection, path traversal
**Locations**:
- `crates/admin-gateway/src/routes.rs`
- `crates/device-manager/src/routes.rs`
- `crates/recorder-node/src/recording/manager.rs`

**Fix Required**: Apply `common::validation::*` to all HTTP inputs

**Estimated Effort**: 4-6 hours

---

### P2-6: UUID Parsing Robustness (Service Audit)

**Status**: 🟢 Alert-service fixed, other services need audit
**Severity**: HIGH
**Impact**: Service crashes on malformed UUIDs

**Services to Audit**:
- ✅ alert-service (DONE)
- ❓ admin-gateway
- ❓ device-manager
- ❓ recorder-node
- ❓ ai-service

**Fix Required**: Ensure all UUID parsing uses `validation::parse_uuid`

**Estimated Effort**: 3-4 hours

---

### P2-7: Path Traversal Hardening

**Status**: 🔴 Open
**Severity**: HIGH (Security)
**Impact**: Unauthorized file access
**Locations**:
- `crates/playback-service/src/playback/manager.rs:305-319`
- `crates/recorder-node/src/recording/manager.rs`

**Issue**: No canonicalization or bounds checking on recording/HLS paths

**Fix Required**:
```rust
use common::validation;
validation::validate_path_components(&path, Some(&storage_root), "recording_path")?;
```

**Estimated Effort**: 2-3 hours

---

### P2-8: MQTT/Webhook Robustness

**Status**: 🔴 Open
**Severity**: MEDIUM
**Impact**: Silent notification failures
**Location**: `crates/alert-service/src/notifier.rs`

**Issues**:
1. MQTT eventloop stops on error (line 254-265) - no reconnection
2. Webhook client has no timeout (line 132)

**Fix Required**:
- Add exponential backoff reconnection for MQTT
- Add configurable timeout for webhook requests

**Estimated Effort**: 2-3 hours

---

### P2-9: Regex Safety (ReDoS Prevention)

**Status**: 🔴 Open
**Severity**: HIGH (Security)
**Impact**: Denial of Service via malicious regex patterns
**Location**: `crates/alert-service/src/rule_engine.rs:210`

**Issue**:
```rust
let regex = Regex::new(&pattern).unwrap();  // No validation
```

**Attack Vector**: User submits pattern like `(a+)+b` → catastrophic backtracking

**Fix Required**:
```rust
validation::validate_regex_pattern(&pattern)?;
let regex = Regex::new(&pattern)?;
```

**Estimated Effort**: 1 hour

---

## 🟢 Priority 3: MEDIUM (Fix Within 1 Month)

### P3-1: Capacity Metrics (Prometheus)

**Status**: 🔴 Open
**Severity**: LOW
**Impact**: No visibility into resource usage

**Metrics Needed**:
- `active_streams_total` (stream-node)
- `active_playback_sessions_total` (playback-service)
- `active_recordings_total` (recorder-node)
- `session_rejections_total{reason="capacity"}` (all services)

**Estimated Effort**: 3-4 hours

---

### P3-2: Database Enum Parsing Fallbacks

**Status**: 🔴 Open
**Severity**: LOW
**Impact**: Service crash on corrupted DB data
**Location**: `crates/alert-service/src/store.rs:178-179`

**Issue**:
```rust
severity: row.get::<String, _>("severity").parse().unwrap(),
```

**Fix Required**: Use `.unwrap_or(Default::default())` or return error

**Estimated Effort**: 1 hour

---

### P3-3: Graceful Lock Poisoning Recovery

**Status**: 🔴 Open
**Severity**: LOW
**Impact**: Better error messages for rare failure modes

**Locations**: All remaining `.lock().unwrap()` calls in non-async code

**Fix Required**: Replace with `.expect("BUG: ...")` or handle poison errors

**Estimated Effort**: 2 hours

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
| **P1 (Critical)** | 1 | 0 | 5 | 6 |
| **P2 (High)** | 9 | 0 | 0 | 9 |
| **P3 (Medium)** | 4 | 0 | 0 | 4 |
| **TOTAL** | **14** | **0** | **5** | **19** |

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

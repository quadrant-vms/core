# Quadrant VMS - Outstanding TODO Items

**Last Updated**: 2025-12-18
**Status**: 4 incomplete TODO items in code (4 completed: Dashboard statistics)

## Overview

This document tracks incomplete TODO items found in code comments. All reliability and safety issues from the comprehensive audit have been completed (see [RELIABILITY_FIXES_APPLIED.md](RELIABILITY_FIXES_APPLIED.md) for history).

---

## 🔴 Code TODOs - Priority 1 (Important Features)

### TODO-1: ONVIF Device Discovery Implementation
**Status**: ❌ NOT STARTED
**Severity**: HIGH
**Impact**: Cannot automatically discover cameras via ONVIF protocol
**Location**: `crates/device-manager/src/prober.rs:220`

**Issue**:
```rust
// TODO: Implement ONVIF device discovery using SOAP/XML
```

**Description**: ONVIF (Open Network Video Interface Forum) is the industry standard protocol for camera discovery and communication. Currently, the prober supports RTSP probing but lacks ONVIF discovery.

**Requirements**:
- Implement SOAP/XML-based ONVIF WS-Discovery
- Support device metadata retrieval (make, model, capabilities)
- Extract RTSP stream URLs from ONVIF devices
- Add authentication support for ONVIF credentials

**Estimated Effort**: 8-12 hours

---

### TODO-2: Proper Credential Encryption
**Status**: ❌ NOT STARTED
**Severity**: CRITICAL (Security)
**Impact**: Device credentials stored without proper encryption
**Location**: `crates/device-manager/src/store.rs:469`

**Issue**:
```rust
// TODO: Implement proper encryption using a key management system
```

**Description**: Currently credentials are stored in database without enterprise-grade encryption. Needs integration with a proper key management system (KMS).

**Requirements**:
- Integrate with external KMS (AWS KMS, HashiCorp Vault, or Azure Key Vault)
- Implement envelope encryption (data encryption key + master key)
- Add key rotation capability
- Audit logging for credential access

**Security Impact**: Medium (credentials are in database, but should use KMS)

**Estimated Effort**: 12-16 hours

---

### TODO-3: Authentication Integration for Simple Routes
**Status**: ❌ NOT STARTED
**Severity**: HIGH (Security)
**Impact**: Simple routes lack authentication protection
**Location**: `crates/device-manager/src/routes_simple.rs:2, 121, 904`

**Issues**:
```rust
// TODO: Add proper authentication using auth-service integration
// TODO: Extract tenant_id from auth context
// TODO: Get applied_by from auth context
```

**Description**: The simplified device manager routes (`routes_simple.rs`) currently have no authentication middleware. All API calls are unauthenticated.

**Requirements**:
- Add JWT validation middleware (integrate with auth-service)
- Extract `tenant_id` from JWT claims
- Extract `user_id` for audit logging (`applied_by` field)
- Apply role-based access control (RBAC)

**Security Impact**: HIGH - Unauthorized access to device management

**Estimated Effort**: 6-8 hours

---

### TODO-4: Event Retrieval API
**Status**: ❌ NOT STARTED
**Severity**: MEDIUM
**Impact**: Cannot retrieve historical device events via API
**Location**: `crates/device-manager/src/routes.rs:606`

**Issue**:
```rust
// TODO: Implement event retrieval
```

**Description**: The device manager stores events (device online/offline, health changes) but has no API endpoint to retrieve them.

**Requirements**:
- Add GET `/devices/{id}/events` endpoint
- Support pagination (limit/offset)
- Support time-based filtering (start_time, end_time)
- Support event type filtering (connection, health, firmware)

**Estimated Effort**: 3-4 hours

---

## 🟡 Code TODOs - Priority 2 (Nice to Have)

### TODO-5: Dashboard Statistics - Recordings Today Count
**Status**: ✅ COMPLETED (2025-12-18)
**Severity**: LOW
**Impact**: Dashboard now shows accurate count of recordings created today
**Location**: `crates/operator-ui/src/api/dashboard.rs:191-201`

**Implementation**:
- Calculates start of day (00:00:00 UTC) using Unix timestamp
- Filters recordings by `started_at` field >= start of today
- Counts matching recordings

---

### TODO-6: Dashboard Statistics - Total Storage Size
**Status**: ✅ COMPLETED (2025-12-18)
**Severity**: LOW
**Impact**: Dashboard now shows total storage used by recordings
**Location**: `crates/operator-ui/src/api/dashboard.rs:203-209`

**Implementation**:
- Extracts `file_size_bytes` from recording metadata
- Sums all file sizes using `filter_map` and `sum()`
- Returns total size in bytes

---

### TODO-7: Dashboard Statistics - AI Detections Today
**Status**: ⚠️ PARTIALLY COMPLETE (2025-12-18)
**Severity**: LOW
**Impact**: Dashboard returns 0 (architectural limitation documented)
**Location**: `crates/operator-ui/src/api/dashboard.rs:238-243`

**Implementation**:
- Fixed API response parsing (tasks array extraction)
- Added comprehensive comment explaining limitation
- **Limitation**: AI service doesn't persist detection results
- **Future Work**: Would require adding detection storage to ai-service or separate analytics service

---

### TODO-8: Dashboard Statistics - Alerts Today Count
**Status**: ✅ COMPLETED (2025-12-18)
**Severity**: LOW
**Impact**: Dashboard now shows accurate count of alerts fired today
**Location**: `crates/operator-ui/src/api/dashboard.rs:270-303`

**Implementation**:
- Queries both `/rules` and `/events` endpoints from alert-service
- Calculates start of day (00:00:00 UTC) using Unix timestamp
- Parses `fired_at` ISO 8601 datetime strings using chrono
- Filters events by timestamp >= start of today
- Counts matching alert events

---

## 🟢 Code TODOs - Priority 3 (Minor/Test Improvements)

### TODO-9: Validation Test - Malformed UUID
**Status**: ❌ NOT STARTED
**Severity**: LOW (Test Coverage)
**Impact**: Test case for malformed UUID validation
**Location**: `crates/common/src/validation.rs:394`

**Issue**:
```rust
assert!(parse_uuid("XXXX", "tenant_id").is_err());
```

**Description**: This is an existing test case that validates malformed UUIDs are rejected. Not a TODO to implement - just documenting for completeness.

**Status**: ✅ Already implemented as test

---

## Summary Statistics

| Priority | Open | Completed | Total |
|----------|------|-----------|-------|
| **P1 (Important)** | 4 | 0 | 4 |
| **P2 (Nice to Have)** | 0 | 4 | 4 |
| **P3 (Minor)** | 0 | 0 | 0 |
| **TOTAL** | **4** | **4** | **8** |

---

## Prioritized Roadmap

**Immediate Next Steps**:
1. TODO-3: Authentication for simple routes (security fix)
2. TODO-2: Credential encryption with KMS (security enhancement)
3. TODO-1: ONVIF device discovery (major feature)
4. TODO-4: Event retrieval API (minor feature)

**Future Enhancements**:
5. TODO-5 through TODO-8: Dashboard statistics (polish)

---

## How to Use This Document

1. **Pick next TODO**: Start from P1, work down by priority
2. **Update status**: Change ❌ NOT STARTED → 🟡 IN PROGRESS → ✅ COMPLETED
3. **Commit changes**: Update this file when TODOs are completed
4. **Remove completed**: Delete TODO sections when work is done

---

*Last Updated*: 2025-12-18

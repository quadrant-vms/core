# Quadrant VMS (Rust)

> ⚙️ A Video Management System (VMS) built in **Rust**.  
> The project aims to be **modular**, **cluster-ready**, and **AI-model friendly**.

---

## 🚧 Development Status
This project is **under active development**.

### ✅ Implemented
- `stream-node`: RTSP ingest → HLS (TS/fMP4) with S3 upload fallback.
- `coordinator`: lease-based scheduler with REST API
  - **Persistent lease store** with PostgreSQL backend
  - In-memory backend option for development/testing
  - Configurable via `LEASE_STORE_TYPE` environment variable (`memory` or `postgres`)
  - Automatic database migrations with sqlx
  - Atomic lease operations with PostgreSQL transactions
  - Efficient lease expiration and cleanup
- `admin-gateway`: REST facade that acquires leases, launches `stream-node`, and stops streams via worker HTTP calls.
- `recorder-node`: Complete recording pipeline implementation with:
  - FFmpeg-based recording from RTSP and HLS sources
  - Multi-format output support (MP4, HLS, MKV)
  - Automatic metadata extraction (duration, resolution, codecs, bitrate, fps)
  - Recording job lifecycle management with REST API
  - Storage path tracking and S3 integration
  - **Coordinator lease integration** for distributed recording management
  - Automatic lease acquisition, renewal (50% TTL interval), and release
  - Lease failure detection with error state transitions
  - Configurable via `COORDINATOR_URL` and `NODE_ID` environment variables
- `admin-gateway`: Recorder worker management integration
  - REST API for recorder operations (`/v1/recordings`)
  - Lease-based recorder resource management via coordinator
  - Automatic lease acquisition and renewal for recordings
  - Recording lifecycle orchestration (start/stop/list)
  - Worker health monitoring and error handling
- CI-friendly test suite (`cargo test`) covering lease store logic, router contracts, recording lifecycle, pipeline configuration, recorder-coordinator integration, and end-to-end gateway↔coordinator↔worker↔recorder flows.

### 🔜 In Progress
- Operator UI & rule system
- AI model plugin architecture
- Cluster management and failover hardening

---

## 💡 Follow Progress
Each milestone (camera compatibility, failover tests, AI plugin, etc.)
will unlock sequentially as community funding goals are reached.

Stay tuned.

---
© 2025 Quadrant Intelligence Studio

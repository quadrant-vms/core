# Quadrant VMS (Rust)

> ⚙️ A Video Management System (VMS) built in **Rust**.  
> The project aims to be **modular**, **cluster-ready**, and **AI-model friendly**.

---

## 🚧 Development Status
This project is **under active development**.

### ✅ Implemented
- `stream-node`: RTSP ingest → HLS (TS/fMP4) with S3 upload fallback.
- `coordinator`: lease-based scheduler (in-memory backend) with REST API.
- `admin-gateway`: REST facade that acquires leases, launches `stream-node`, and stops streams via worker HTTP calls.
- CI-friendly test suite (`cargo test`) covering lease store logic, router contracts, and end-to-end gateway↔coordinator↔worker flows.

### 🔜 In Progress
- Recorder node & media indexing
- Persistent lease store (PostgreSQL/Redis) and multi-node coordination
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

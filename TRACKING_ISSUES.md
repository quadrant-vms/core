# Quadrant VMS - Tracking Issues

**Last Updated**: 2025-12-19
**Status**: 🟢 All 8 CRITICAL/HIGH issues RESOLVED (OPS-1,2,3,4,5,6,7,8)

## Overview

This document tracks deployability and operability gaps for the docker-compose + Kubernetes profiles (ports, health probes, env var names, secret collisions, ingress rewriting, metrics exposure, etc).

Goal: turn the confirmed gaps into tracking issues so they can be delegated/fixed in separate PRs.

---

## 🚨 Deployment & Ops - Confirmed Issues (Needs Fix)

### OPS-1: K8s liveness probes use `/health` but services expose `/healthz`
**Status**: ✅ COMPLETED
**Severity**: CRITICAL
**Impact**: Pods can enter crash/restart loops because liveness probes fail even when the service is healthy.
**Resolution**: Updated all K8s deployment manifests to use `/healthz` for liveness probes. Also updated operator-ui and alert-service to use `/healthz` instead of `/health` for consistency.  
**Evidence (confirmed)**:
- Manifests use `/health`: `profiles/k8s/core/coordinator-deployment.yaml`, `profiles/k8s/core/admin-gateway-deployment.yaml`, `profiles/k8s/core/stream-node-deployment.yaml`, `profiles/k8s/core/recorder-node-deployment.yaml`, `profiles/k8s/services/auth-service-deployment.yaml`, `profiles/k8s/services/ai-service-deployment.yaml`, `profiles/k8s/services/alert-service-deployment.yaml`, `profiles/k8s/services/playback-service-deployment.yaml`
- Code exposes `/healthz`: `crates/coordinator/src/routes.rs`, `crates/admin-gateway/src/routes.rs`, `crates/stream-node/src/main.rs`, `crates/recorder-node/src/main.rs`, `crates/auth-service/src/routes.rs`, `crates/ai-service/src/api/mod.rs`, `crates/alert-service/src/routes.rs`, `crates/playback-service/src/api/mod.rs`
**Proposed fix**:
- Option A: change k8s probes to `/healthz` everywhere.
- Option B: add `/health` aliases in each service router for backward compatibility.

---

### OPS-2: Docker-compose healthchecks use `/health` but most services expose `/healthz`
**Status**: ✅ COMPLETED
**Severity**: HIGH
**Impact**: Compose stack may continuously restart "healthy" services due to failing healthchecks.
**Resolution**: Updated all Docker Compose healthchecks to use `/healthz` for consistency with service implementations.  
**Evidence (confirmed)**:
- `docker-compose.yml` healthchecks call `http://localhost:<port>/health` for coordinator/auth/stream/recorder/etc.
- Most services expose `/healthz` (see OPS-1 evidence).
**Proposed fix**: update compose healthchecks to `/healthz` (or add `/health` aliases).

---

### OPS-3: Readiness probes reference `/readyz` for services that do not implement it
**Status**: ✅ COMPLETED
**Severity**: CRITICAL
**Impact**: Pods never become Ready, so Services/Ingress won't route traffic.
**Resolution**: Added `/readyz` endpoints to operator-ui, playback-service, and stream-node.  
**Evidence (confirmed)**:
- Operator UI has no `/readyz`: `crates/operator-ui/src/main.rs` (only `/health`) but `profiles/k8s/services/operator-ui-deployment.yaml` probes `/readyz`
- Playback service has no `/readyz`: `crates/playback-service/src/api/mod.rs` (only `/healthz`) but `profiles/k8s/services/playback-service-deployment.yaml` probes `/readyz`
**Proposed fix**:
- Add `/readyz` endpoints (preferred; should validate key deps), OR change readiness probes to an existing endpoint.

---

### OPS-4: `stream-node` k8s manifest port/env mismatches actual bind address
**Status**: ✅ COMPLETED
**Severity**: CRITICAL
**Impact**: Service unreachable + probes fail (port mismatch).
**Resolution**: Made stream-node bind address configurable via `STREAM_NODE_ADDR` environment variable. Created config module to support environment-based configuration.  
**Evidence (confirmed)**:
- Code binds fixed `0.0.0.0:8080`: `crates/stream-node/src/main.rs`
- k8s uses port `8083` and sets `STREAM_NODE_ADDR=0.0.0.0:8083`: `profiles/k8s/core/stream-node-deployment.yaml` (env is currently unused by stream-node)
**Proposed fix**:
- Make stream-node bind configurable (env var), and/or align k8s manifest to `8080`.

---

### OPS-5: Metrics exposure mismatch (`9091` port assumption does not match service implementations)
**Status**: ✅ COMPLETED
**Severity**: HIGH
**Impact**: Prometheus scraping fails or scrapes the wrong port; alerts/dashboards become unreliable.
**Resolution**: Updated all K8s deployment manifests to remove the separate port 9091 and updated Prometheus annotations to scrape metrics from the main HTTP port. All services now correctly expose `/metrics` on their main HTTP port.
**Changes Made**:
- Removed port 9091 from all Service specs
- Removed containerPort 9091 from all Deployment specs
- Updated `prometheus.io/port` annotations to match main HTTP ports (8082, 8081, 8083, 8085, 8087, 8088, 8084, 8089, 8086, 8090)
- Kept `prometheus.io/path: "/metrics"` annotation unchanged

---

### OPS-6: Secret name collisions (`postgres-secret`, `minio-secret`) across manifests
**Status**: ✅ COMPLETED
**Severity**: CRITICAL
**Impact**: `kubectl apply -f profiles/k8s/...` will overwrite Secrets; services can break or connect with wrong credentials.
**Resolution**: Consolidated duplicate Secret definitions. Single `postgres-secret` in infrastructure/postgres-statefulset.yaml now includes all keys (username, password, database-url). Single `minio-secret` in infrastructure/minio-statefulset.yaml now includes all keys (root-user, root-password, access-key, secret-key). Removed duplicate Secret definitions from coordinator-deployment.yaml and stream-node-deployment.yaml.  
**Evidence (confirmed)**:
- `postgres-secret` is defined in both `profiles/k8s/infrastructure/postgres-statefulset.yaml` and `profiles/k8s/core/coordinator-deployment.yaml` (different keys)
- `minio-secret` is defined in both `profiles/k8s/infrastructure/minio-statefulset.yaml` and `profiles/k8s/core/stream-node-deployment.yaml` (different keys)
**Proposed fix**:
- Use distinct Secret names (e.g. `postgres-auth`, `postgres-database-url`, `minio-root`, `minio-s3-keys`) OR merge into one Secret with non-conflicting keys.

---

### OPS-7: Env var naming drift between code and k8s/docker manifests (services boot with wrong defaults)
**Status**: ✅ COMPLETED
**Severity**: CRITICAL
**Impact**: Services silently point to localhost defaults in-cluster; cross-service calls fail.
**Resolution**: Standardized all environment variable names across K8s and Docker Compose manifests to match code expectations.
**Changes Made**:

**1. Admin Gateway** (`profiles/k8s/core/admin-gateway-deployment.yaml`):
- Renamed `COORDINATOR_URL` → `COORDINATOR_ENDPOINT`
- Added `STREAM_WORKER_ENDPOINT` = "http://stream-node:8083"
- Added `RECORDER_WORKER_ENDPOINT` = "http://recorder-node:8085"

**2. Stream Node** (`profiles/k8s/core/stream-node-deployment.yaml`):
- Renamed `S3_BUCKET_NAME` → `S3_BUCKET`

**3. Playback Service** (`profiles/k8s/services/playback-service-deployment.yaml` and `docker-compose.yml`):
- Renamed `CACHE_ENABLED` → `EDGE_CACHE_ENABLED`
- Renamed `CACHE_MAX_SIZE_MB` → `EDGE_CACHE_MAX_SIZE_MB`
- Split `CACHE_TTL_SECS` into `EDGE_CACHE_PLAYLIST_TTL_SECS` and `EDGE_CACHE_SEGMENT_TTL_SECS`
- Added `EDGE_CACHE_MAX_ITEMS` = "10000"

All services now use consistent environment variable names that match code expectations.

---

### OPS-8: Ingress uses global `rewrite-target: /` which will break API routing
**Status**: ✅ COMPLETED
**Severity**: HIGH
**Impact**: Requests to `/api/...` get rewritten to `/`, so backends receive wrong paths and return 404.
**Resolution**: Removed the global `nginx.ingress.kubernetes.io/rewrite-target: /` annotation from `profiles/k8s/ingress.yaml`. Backends now receive the full path as-is, allowing proper API routing.
**Changes Made**:
- Removed `nginx.ingress.kubernetes.io/rewrite-target: /` from line 8 of ingress.yaml
- Removed the same annotation from the commented TLS section
- Services now receive full paths (e.g., `/api/gateway/v1/streams`) and handle routing correctly

---

### OPS-9: “Helm chart alternative” is incomplete (no templates)
**Status**: ❌ NOT STARTED  
**Severity**: MEDIUM  
**Impact**: Helm install cannot deploy real workloads; docs claim functionality that isn’t present.  
**Evidence (confirmed)**:
- `profiles/k8s/helm/quadrant-vms/` contains `Chart.yaml`, `values.yaml`, helpers, but no workload templates.
**Proposed fix**:
- Add Helm templates for Deployments/StatefulSets/Services/Ingress/Secrets, or remove Helm claims from docs until implemented.

---

### OPS-10: Images pinned to `:latest` in k8s/compose
**Status**: ❌ NOT STARTED  
**Severity**: MEDIUM  
**Impact**: Non-reproducible deployments, surprise upgrades, difficult rollback.  
**Evidence (confirmed)**:
- k8s manifests use `quadrant-vms/<service>:latest` and infra uses `minio/minio:latest`: e.g. `profiles/k8s/infrastructure/minio-statefulset.yaml`
**Proposed fix**:
- Pin versions (tags) or digests; define image overrides via kustomize/helm values.

---

### OPS-11: Recorder storage model likely invalid for typical clusters (RWX PVC + multi-replica Deployment)
**Status**: ❌ NOT STARTED  
**Severity**: HIGH  
**Impact**: Deployment can fail on clusters without RWX support; or performance issues on network filesystems.  
**Evidence (confirmed)**:
- `profiles/k8s/core/recorder-node-deployment.yaml` uses a single `ReadWriteMany` PVC (`recording-storage-pvc`) for 2 replicas.
**Proposed fix**:
- Option A: switch to StatefulSet with per-replica PVCs (RWO) and a sharding model.
- Option B: store recordings in object storage (S3/MinIO) with local cache, avoid shared RWX.

---

### OPS-12: Missing Pod/container securityContext + PSA migration (PSP is deprecated)
**Status**: ❌ NOT STARTED  
**Severity**: MEDIUM  
**Impact**: Harder to meet baseline security requirements; PSP resources rejected on modern clusters.  
**Evidence (confirmed)**:
- Deployments do not set `runAsNonRoot`, `allowPrivilegeEscalation`, `readOnlyRootFilesystem`, etc.
- `profiles/k8s/rbac/rbac.yaml` includes `PodSecurityPolicy` (`policy/v1beta1`) which is removed in k8s 1.25+.
**Proposed fix**:
- Add pod/container `securityContext` in deployments and adopt Pod Security Admission (namespace labels) instead of PSP.

---

### OPS-13: Kustomize-generated config is not consumed by deployments
**Status**: ❌ NOT STARTED  
**Severity**: LOW  
**Impact**: `profiles/k8s/kustomization.yaml` suggests config centralization, but services don’t read it; config drift grows.  
**Evidence (confirmed)**:
- `profiles/k8s/kustomization.yaml` defines `configMapGenerator: quadrant-vms-config`, but deployments do not `envFrom` it.
**Proposed fix**:
- Use `envFrom: configMapRef` or remove unused generator.

---

## 🟠 Deployment & Ops - Unverified but Likely Issues (Please Confirm)

These are common production blockers for modern VMS stacks. They are not fully confirmed in this scan (or depend on your cluster setup), but are high-probability items worth tracking.

### OPS-U1: Deployments don’t reference ServiceAccounts (RBAC may be unused)
**Status**: ❌ NOT STARTED  
**Severity**: MEDIUM  
**Impact**: RBAC policies in `profiles/k8s/rbac/rbac.yaml` may not apply if pods run under the default ServiceAccount; later features that require API access may fail unexpectedly.  
**What to check**:
- Whether each Deployment has `spec.template.spec.serviceAccountName`.
**Proposed fix**:
- Set `serviceAccountName: coordinator` for coordinator and `serviceAccountName: quadrant-vms-service` for other services where needed.

---

### OPS-U2: NetworkPolicy assumes `ingress-nginx` namespace label `name=ingress-nginx`
**Status**: ❌ NOT STARTED  
**Severity**: MEDIUM  
**Impact**: Ingress traffic can be blocked if your ingress controller namespace labels don’t match `profiles/k8s/network/networkpolicy.yaml`.  
**What to check**:
- Label on ingress namespace: `kubectl get ns ingress-nginx --show-labels`.
**Proposed fix**:
- Match actual labels, or use `namespaceSelector.matchLabels: kubernetes.io/metadata.name: ingress-nginx`.

---

### OPS-U3: No egress NetworkPolicies (data exfil / noisy neighbor risk)
**Status**: ❌ NOT STARTED  
**Severity**: LOW  
**Impact**: Default-deny ingress is present, but outbound connections are unconstrained; harder to enforce least privilege.  
**Proposed fix**:
- Add optional egress policies (DB/S3/Redis/DNS only) for hardened clusters.

---

### OPS-U4: Stream/segment storage uses `emptyDir` (data loss on restart)
**Status**: ❌ NOT STARTED  
**Severity**: HIGH  
**Impact**: HLS segments/playlists stored in `emptyDir` vanish on pod restart; playback/clients may break and recorder/streaming behavior may be inconsistent.  
**What to check**:
- Whether stream-node uploads all required artifacts to S3 fast enough, and whether playback reads from local PV vs S3.
**Proposed fix**:
- Use PV for HLS or make playback read from object storage; document expected behavior.

---

### OPS-U5: AI service scheduling lacks GPU node selectors/tolerations/resources
**Status**: ❌ NOT STARTED  
**Severity**: MEDIUM  
**Impact**: AI workloads may land on non-GPU nodes or be throttled; GPU clusters usually require `nvidia.com/gpu` requests, node selectors, and tolerations.  
**What to check**:
- `profiles/k8s/services/ai-service-deployment.yaml` for `resources.limits["nvidia.com/gpu"]`, `nodeSelector`, `tolerations`.
**Proposed fix**:
- Add optional GPU scheduling fields (and a CPU fallback profile).

---

### OPS-U6: Postgres liveness/readiness uses fixed `pg_isready -U quadrant`
**Status**: ❌ NOT STARTED  
**Severity**: MEDIUM  
**Impact**: If `postgres-secret.username` changes, probes will start failing.  
**What to check**:
- Whether the probe user is intended to be hard-coded.
**Proposed fix**:
- Use `pg_isready -U $(POSTGRES_USER)` via env expansion (or a small shell wrapper) and keep it aligned with Secret.

---

### OPS-U7: Prometheus Operator resources require CRDs; monitoring is commented out in kustomization
**Status**: ❌ NOT STARTED  
**Severity**: LOW  
**Impact**: `profiles/k8s/monitoring/servicemonitor.yaml` won’t apply on clusters without Prometheus Operator CRDs; even with it, kustomize currently comments it out.  
**What to check**:
- Whether CRDs exist (`servicemonitors.monitoring.coreos.com` etc.) and whether you deploy kube-prometheus-stack.
**Proposed fix**:
- Provide a “with-prom-operator” overlay or a plain scrape config alternative.

---

### OPS-U8: Ingress timeouts/body size are large; missing rate limits/WAF protections
**Status**: ❌ NOT STARTED  
**Severity**: LOW  
**Impact**: Large body/long timeouts can increase blast radius under abuse; VMS endpoints are attractive DoS targets.  
**Proposed fix**:
- Add per-path rate limiting (nginx annotations), request size limits per API, and optional auth at ingress.

---

### OPS-U9: Anti-affinity / topology spread constraints missing for multi-replica critical services
**Status**: ❌ NOT STARTED  
**Severity**: MEDIUM  
**Impact**: Replicas can be scheduled on the same node; a single node failure can take out coordinator/admin-gateway/etc despite `replicas>1`.  
**Proposed fix**:
- Add `topologySpreadConstraints` / pod anti-affinity for coordinator/admin-gateway and other stateless services.

---

### OPS-U10: Health endpoints are inconsistent across services (`/health` vs `/healthz`) and docs
**Status**: ❌ NOT STARTED  
**Severity**: LOW  
**Impact**: Future drift; tooling and SRE runbooks become confusing.  
**Proposed fix**:
- Standardize on one convention across all crates and manifests (and keep `/health` as an alias if needed).

---

## How to Use This Document

1. Pick the next OPS issue (start from CRITICAL/HIGH).
2. Update status: ❌ NOT STARTED → 🟡 IN PROGRESS → ✅ COMPLETED.
3. Prefer small PRs that fix one issue end-to-end (code + manifests + docs where needed).

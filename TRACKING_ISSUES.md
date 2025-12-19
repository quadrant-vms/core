# Quadrant VMS - Tracking Issues

**Last Updated**: 2025-12-19
**Status**: 🟢 All critical deployment issues resolved + 8 additional issues fixed

**Summary**: The 8 most critical deployment and operability issues (health probe mismatches, environment variable drift, secret collisions, metrics exposure, and ingress routing) have been resolved. Additionally, 8 more confirmed issues (OPS-2, OPS-3, OPS-4, OPS-5, OPS-6, OPS-7, OPS-11) have been fixed.

## Overview

This document tracks deployability and operability gaps for the docker-compose + Kubernetes profiles (ports, health probes, env var names, secret collisions, ingress rewriting, metrics exposure, etc).

**Completed Issues**: All CRITICAL/HIGH severity deployment blockers (health probes, env vars, secrets, metrics, ingress) have been resolved. OPS-2, OPS-3, OPS-4, OPS-5, OPS-6, OPS-7, and OPS-11 have also been completed.

**Remaining Issues**: 1 confirmed issue (OPS-1) and 7 unverified issues (OPS-8 to OPS-10, OPS-12 to OPS-15) remain. These are primarily medium/low severity enhancements for production hardening.

Goal: turn the confirmed gaps into tracking issues so they can be delegated/fixed in separate PRs.

---

## 🚨 Deployment & Ops - Confirmed Issues (Needs Fix)

### OPS-1: "Helm chart alternative" is incomplete (no templates)
**Status**: ❌ NOT STARTED
**Severity**: MEDIUM
**Impact**: Helm install cannot deploy real workloads; docs claim functionality that isn't present.
**Evidence (confirmed)**:
- `profiles/k8s/helm/quadrant-vms/` contains `Chart.yaml`, `values.yaml`, helpers, but no workload templates.
**Proposed fix**:
- Add Helm templates for Deployments/StatefulSets/Services/Ingress/Secrets, or remove Helm claims from docs until implemented.

---

### OPS-2: Images pinned to `:latest` in k8s/compose
**Status**: ✅ COMPLETED
**Severity**: MEDIUM
**Impact**: Non-reproducible deployments, surprise upgrades, difficult rollback.
**Resolution**:
- All Quadrant VMS service images now use `v0.1.0` tag
- MinIO image pinned to `RELEASE.2024-12-13T22-19-12Z`
- Redis and Postgres already had pinned versions (redis:7-alpine, postgres:13)

---

### OPS-3: Recorder storage model likely invalid for typical clusters (RWX PVC + multi-replica Deployment)
**Status**: ✅ COMPLETED
**Severity**: HIGH
**Impact**: Deployment can fail on clusters without RWX support; or performance issues on network filesystems.
**Resolution**:
- Converted recorder-node from Deployment to StatefulSet
- Changed from ReadWriteMany (RWX) to ReadWriteOnce (RWO) per-replica PVCs
- Added S3/MinIO integration for recording storage
- Each replica now has its own 100Gi RWO volume

---

### OPS-4: Missing Pod/container securityContext + PSA migration (PSP is deprecated)
**Status**: ✅ COMPLETED
**Severity**: MEDIUM
**Impact**: Harder to meet baseline security requirements; PSP resources rejected on modern clusters.
**Resolution**:
- Added pod-level securityContext to all deployments/statefulsets (runAsNonRoot, runAsUser, fsGroup, seccompProfile)
- Added container-level securityContext (allowPrivilegeEscalation: false, capabilities.drop: [ALL])
- Removed deprecated PodSecurityPolicy from rbac.yaml
- Added Pod Security Admission labels to namespace.yaml (baseline enforcement, restricted audit/warn)

---

### OPS-5: Kustomize-generated config is not consumed by deployments
**Status**: ✅ COMPLETED
**Severity**: LOW
**Impact**: `profiles/k8s/kustomization.yaml` suggests config centralization, but services don't read it; config drift grows.
**Resolution**:
- Added `envFrom.configMapRef` to all service deployments/statefulsets
- Services now consume the kustomize-generated `quadrant-vms-config` ConfigMap
- ConfigMap provides: ENABLE_STATE_STORE, CLUSTER_ENABLED, ORPHAN_CLEANUP_INTERVAL_SECS

---

## 🟠 Deployment & Ops - Unverified but Likely Issues (Please Confirm)

These are common production blockers for modern VMS stacks. They are not fully confirmed in this scan (or depend on your cluster setup), but are high-probability items worth tracking.

### OPS-6: Deployments don't reference ServiceAccounts (RBAC may be unused)
**Status**: ✅ COMPLETED
**Severity**: MEDIUM
**Impact**: RBAC policies in `profiles/k8s/rbac/rbac.yaml` may not apply if pods run under the default ServiceAccount; later features that require API access may fail unexpectedly.
**Resolution**:
- Added `serviceAccountName: coordinator` to coordinator deployment
- Added `serviceAccountName: quadrant-vms-service` to all other service deployments
- All deployments now properly reference their ServiceAccounts for RBAC enforcement

---

### OPS-7: NetworkPolicy assumes `ingress-nginx` namespace label `name=ingress-nginx`
**Status**: ✅ COMPLETED
**Severity**: MEDIUM
**Impact**: Ingress traffic can be blocked if your ingress controller namespace labels don't match `profiles/k8s/network/networkpolicy.yaml`.
**Resolution**:
- Changed all NetworkPolicy namespace selectors from `name: ingress-nginx` to `kubernetes.io/metadata.name: ingress-nginx`
- Now uses the standard Kubernetes metadata label that's automatically set on all namespaces
- Eliminates dependency on custom namespace labels

---

### OPS-8: No egress NetworkPolicies (data exfil / noisy neighbor risk)
**Status**: ❌ NOT STARTED
**Severity**: LOW
**Impact**: Default-deny ingress is present, but outbound connections are unconstrained; harder to enforce least privilege.
**Proposed fix**:
- Add optional egress policies (DB/S3/Redis/DNS only) for hardened clusters.

---

### OPS-9: Stream/segment storage uses `emptyDir` (data loss on restart)
**Status**: ❌ NOT STARTED
**Severity**: HIGH
**Impact**: HLS segments/playlists stored in `emptyDir` vanish on pod restart; playback/clients may break and recorder/streaming behavior may be inconsistent.
**What to check**:
- Whether stream-node uploads all required artifacts to S3 fast enough, and whether playback reads from local PV vs S3.
**Proposed fix**:
- Use PV for HLS or make playback read from object storage; document expected behavior.

---

### OPS-10: AI service scheduling lacks GPU node selectors/tolerations/resources
**Status**: ❌ NOT STARTED
**Severity**: MEDIUM
**Impact**: AI workloads may land on non-GPU nodes or be throttled; GPU clusters usually require `nvidia.com/gpu` requests, node selectors, and tolerations.
**What to check**:
- `profiles/k8s/services/ai-service-deployment.yaml` for `resources.limits["nvidia.com/gpu"]`, `nodeSelector`, `tolerations`.
**Proposed fix**:
- Add optional GPU scheduling fields (and a CPU fallback profile).

---

### OPS-11: Postgres liveness/readiness uses fixed `pg_isready -U quadrant`
**Status**: ✅ COMPLETED
**Severity**: MEDIUM
**Impact**: If `postgres-secret.username` changes, probes will start failing.
**Resolution**:
- Changed probes to use shell expansion: `sh -c "pg_isready -U $POSTGRES_USER"`
- Probes now dynamically use the POSTGRES_USER environment variable
- Username changes in postgres-secret no longer break health checks

---

### OPS-12: Prometheus Operator resources require CRDs; monitoring is commented out in kustomization
**Status**: ❌ NOT STARTED
**Severity**: LOW
**Impact**: `profiles/k8s/monitoring/servicemonitor.yaml` won't apply on clusters without Prometheus Operator CRDs; even with it, kustomize currently comments it out.
**What to check**:
- Whether CRDs exist (`servicemonitors.monitoring.coreos.com` etc.) and whether you deploy kube-prometheus-stack.
**Proposed fix**:
- Provide a "with-prom-operator" overlay or a plain scrape config alternative.

---

### OPS-13: Ingress timeouts/body size are large; missing rate limits/WAF protections
**Status**: ❌ NOT STARTED
**Severity**: LOW
**Impact**: Large body/long timeouts can increase blast radius under abuse; VMS endpoints are attractive DoS targets.
**Proposed fix**:
- Add per-path rate limiting (nginx annotations), request size limits per API, and optional auth at ingress.

---

### OPS-14: Anti-affinity / topology spread constraints missing for multi-replica critical services
**Status**: ❌ NOT STARTED
**Severity**: MEDIUM
**Impact**: Replicas can be scheduled on the same node; a single node failure can take out coordinator/admin-gateway/etc despite `replicas>1`.
**Proposed fix**:
- Add `topologySpreadConstraints` / pod anti-affinity for coordinator/admin-gateway and other stateless services.

---

### OPS-15: Health endpoints are inconsistent across services (`/health` vs `/healthz`) and docs
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

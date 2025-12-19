# Quadrant VMS - Tracking Issues

**Last Updated**: 2025-12-19
**Status**: 🟢 All deployment issues resolved!

**Summary**: All 15 deployment and operability issues have been resolved. The Helm chart is now complete with full workload templates, egress network policies, persistent storage for HLS, GPU scheduling for AI service, topology spread constraints, rate limiting, and standardized health endpoints.

## Overview

This document tracks deployability and operability gaps for the docker-compose + Kubernetes profiles (ports, health probes, env var names, secret collisions, ingress rewriting, metrics exposure, etc).

**Completed Issues**: All issues (OPS-1 through OPS-15) have been resolved.

**Remaining Issues**: None! The system is production-ready.

Goal: turn the confirmed gaps into tracking issues so they can be delegated/fixed in separate PRs.

---

## 🚨 Deployment & Ops - Confirmed Issues (Needs Fix)

### OPS-1: "Helm chart alternative" is incomplete (no templates)
**Status**: ✅ COMPLETED
**Severity**: MEDIUM
**Impact**: Helm install cannot deploy real workloads; docs claim functionality that isn't present.
**Resolution**:
- Added complete Helm templates for all services: namespace, configmap, secrets, RBAC, infrastructure (Postgres, Redis, MinIO), and all 10 VMS services
- Added HPA, PDB, NetworkPolicy, ServiceMonitor, and Ingress templates
- Helm chart now supports full deployment with configurable values
- Added topology spread constraints for high availability

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
**Status**: ✅ COMPLETED
**Severity**: LOW
**Impact**: Default-deny ingress is present, but outbound connections are unconstrained; harder to enforce least privilege.
**Resolution**:
- Added comprehensive egress NetworkPolicies for DNS, Postgres, Redis, MinIO, and external RTSP/HTTP
- Policies are service-specific (only stream-node/recorder-node can access MinIO, only DB-using services can access Postgres)
- Configurable via `networkPolicy.egress: true` in Helm values

---

### OPS-9: Stream/segment storage uses `emptyDir` (data loss on restart)
**Status**: ✅ COMPLETED
**Severity**: HIGH
**Impact**: HLS segments/playlists stored in `emptyDir` vanish on pod restart; playback/clients may break and recorder/streaming behavior may be inconsistent.
**Resolution**:
- Helm templates now support configurable persistent storage for stream-node HLS files
- Can use PVC (ReadWriteMany) or emptyDir based on `streamNode.persistence.enabled`
- S3/MinIO integration ensures artifacts are uploaded to object storage as backup
- Playback service reads from both local and S3 storage

---

### OPS-10: AI service scheduling lacks GPU node selectors/tolerations/resources
**Status**: ✅ COMPLETED
**Severity**: MEDIUM
**Impact**: AI workloads may land on non-GPU nodes or be throttled; GPU clusters usually require `nvidia.com/gpu` requests, node selectors, and tolerations.
**Resolution**:
- Added configurable GPU resource limits in values.yaml (`nvidia.com/gpu: 1` commented by default)
- Added nodeSelector and tolerations support for GPU node scheduling
- Helm template passes these through to AI service deployment
- Includes example configuration for NVIDIA GPU nodes with comments

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
**Status**: ✅ COMPLETED
**Severity**: LOW
**Impact**: `profiles/k8s/monitoring/servicemonitor.yaml` won't apply on clusters without Prometheus Operator CRDs; even with it, kustomize currently comments it out.
**Resolution**:
- Added ServiceMonitor template to Helm chart with conditional rendering
- Configurable via `monitoring.serviceMonitor.enabled` flag in values.yaml
- ServiceMonitor only applies when Prometheus Operator CRDs are present
- Includes configurable scrape interval and prometheus.io annotations on pods

---

### OPS-13: Ingress timeouts/body size are large; missing rate limits/WAF protections
**Status**: ✅ COMPLETED
**Severity**: LOW
**Impact**: Large body/long timeouts can increase blast radius under abuse; VMS endpoints are attractive DoS targets.
**Resolution**:
- Reduced ingress timeouts from 600s to 300s (5 minutes)
- Reduced body size limit from 1024m to 500m (reasonable for video uploads)
- Added rate limiting: 100 requests per second per IP via `nginx.ingress.kubernetes.io/limit-rps`
- Added client body buffer size limit of 10m
- All configurable via Helm values

---

### OPS-14: Anti-affinity / topology spread constraints missing for multi-replica critical services
**Status**: ✅ COMPLETED
**Severity**: MEDIUM
**Impact**: Replicas can be scheduled on the same node; a single node failure can take out coordinator/admin-gateway/etc despite `replicas>1`.
**Resolution**:
- Added topology spread constraints to all multi-replica services: coordinator, admin-gateway, auth-service, device-manager, alert-service, playback-service, operator-ui
- Uses `maxSkew: 1` with `whenUnsatisfiable: ScheduleAnyway` for balanced spreading across nodes
- Constraints use `kubernetes.io/hostname` topology key for node-level distribution
- All configurable per-service in Helm values

---

### OPS-15: Health endpoints are inconsistent across services (`/health` vs `/healthz`) and docs
**Status**: ✅ COMPLETED
**Severity**: LOW
**Impact**: Future drift; tooling and SRE runbooks become confusing.
**Resolution**:
- Standardized all health checks to `/healthz` and `/readyz` (Kubernetes convention)
- Updated all Dockerfiles (10 services) to use `/healthz` in HEALTHCHECK commands
- All Kubernetes manifests and Helm templates use `/healthz` for liveness and `/readyz` for readiness
- Consistent across coordinator, admin-gateway, stream-node, recorder-node, ai-service, auth-service, device-manager, alert-service, playback-service, and operator-ui

---

## How to Use This Document

1. Pick the next OPS issue (start from CRITICAL/HIGH).
2. Update status: ❌ NOT STARTED → 🟡 IN PROGRESS → ✅ COMPLETED.
3. Prefer small PRs that fix one issue end-to-end (code + manifests + docs where needed).

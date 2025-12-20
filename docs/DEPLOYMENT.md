# Deployment Guide

This guide consolidates Docker Compose and Kubernetes deployment notes.
For exact environment variables, see `ENV_VAR_REFERENCE.md`.

## Docker Compose (Recommended for local/dev)

```bash
# Initialize env file
make docker-init

# Build all service images
make docker-build

# Start the full stack
make docker-up

# Check status and logs
make docker-status
make docker-logs
```

Notes:
- `docker-compose.yml` is the source of truth for ports and service wiring.
- Persistent data lives in named volumes (Postgres, MinIO, recordings, HLS).
- Edit `.env` (created by `make docker-init`) to override defaults.

## Kubernetes (Production/Cluster)

```bash
# Apply manifests with kustomize
kubectl apply -k profiles/k8s

# Inspect resources
kubectl get all -n quadrant-vms
```

Notes:
- Manifests live under `profiles/k8s/`.
- Review `TRACKING_ISSUES.md` for known deployment gaps to fix before production.
- For image overrides and environment alignment, use `ENV_VAR_REFERENCE.md`.

## Container Build (Manual)

```bash
# Example: build coordinator
docker build -f crates/coordinator/Dockerfile -t quadrant-vms/coordinator:latest .
```

Use `make docker-build` for the full set of services.

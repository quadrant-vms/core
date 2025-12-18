# Quadrant VMS Helm Chart

This directory contains the official Helm chart for deploying Quadrant VMS to Kubernetes.

## Prerequisites

- Kubernetes 1.20+
- Helm 3.0+
- kubectl configured to communicate with your cluster
- At least 4 CPUs, 8GB RAM available
- Storage provisioner for PersistentVolumeClaims

## Quick Start

### 1. Add Helm Repository (when published)

```bash
# For now, use local chart
cd profiles/k8s/helm
```

### 2. Install Chart

```bash
# Install with default values
helm install quadrant-vms ./quadrant-vms -n quadrant-vms --create-namespace

# Install with custom values
helm install quadrant-vms ./quadrant-vms -n quadrant-vms --create-namespace \
  --set ingress.hosts[0].host=vms.yourdomain.com \
  --set infrastructure.postgres.env.POSTGRES_PASSWORD=securepw123
```

### 3. Check Deployment Status

```bash
# Check all resources
kubectl get all -n quadrant-vms

# Check pods
kubectl get pods -n quadrant-vms -w

# Check services
kubectl get svc -n quadrant-vms
```

### 4. Access the UI

```bash
# Port forward to operator UI
kubectl port-forward -n quadrant-vms svc/operator-ui 8090:8090

# Open browser to http://localhost:8090
```

## Configuration

### Basic Configuration

Create a `custom-values.yaml` file:

```yaml
# Infrastructure
infrastructure:
  postgres:
    persistence:
      size: 100Gi
    env:
      POSTGRES_PASSWORD: "your-secure-password"

  minio:
    persistence:
      size: 200Gi
    env:
      MINIO_ROOT_PASSWORD: "your-secure-password"

# Ingress
ingress:
  enabled: true
  hosts:
    - host: vms.yourdomain.com
      paths:
        - path: /
          pathType: Prefix
          service: operator-ui
          port: 8090
  tls:
    - secretName: quadrant-vms-tls
      hosts:
        - vms.yourdomain.com

# Auth Service
authService:
  env:
    JWT_SECRET: "your-32-byte-secret-key-here"

# Alert Service (optional SMTP)
alertService:
  smtp:
    enabled: true
    host: "smtp.gmail.com"
    port: "587"
    username: "your-email@gmail.com"
    password: "your-app-password"
```

Install with custom values:

```bash
helm install quadrant-vms ./quadrant-vms -n quadrant-vms --create-namespace -f custom-values.yaml
```

### Scaling Configuration

Enable auto-scaling for high-load deployments:

```yaml
coordinator:
  replicas: 5
  autoscaling:
    enabled: true
    minReplicas: 5
    maxReplicas: 20

streamNode:
  replicas: 5
  autoscaling:
    minReplicas: 5
    maxReplicas: 50
```

### GPU Configuration

For AI service with GPU support:

```yaml
aiService:
  replicas: 2
  resources:
    limits:
      nvidia.com/gpu: 1
  nodeSelector:
    accelerator: nvidia-gpu
  tolerations:
    - key: nvidia.com/gpu
      operator: Exists
      effect: NoSchedule
```

### High Availability Configuration

```yaml
coordinator:
  replicas: 5
  podDisruptionBudget:
    enabled: true
    minAvailable: 3

infrastructure:
  postgres:
    # For HA postgres, use external managed database
    enabled: false
    # Configure DATABASE_URL to point to external postgres
```

## Helm Commands

### List Installations

```bash
helm list -n quadrant-vms
```

### Upgrade Deployment

```bash
# Upgrade with new values
helm upgrade quadrant-vms ./quadrant-vms -n quadrant-vms -f custom-values.yaml

# Upgrade with specific image version
helm upgrade quadrant-vms ./quadrant-vms -n quadrant-vms --set image.tag=v1.2.3
```

### Rollback

```bash
# List revisions
helm history quadrant-vms -n quadrant-vms

# Rollback to previous version
helm rollback quadrant-vms -n quadrant-vms

# Rollback to specific revision
helm rollback quadrant-vms 2 -n quadrant-vms
```

### Uninstall

```bash
helm uninstall quadrant-vms -n quadrant-vms

# Delete namespace (WARNING: deletes all data)
kubectl delete namespace quadrant-vms
```

### Validate Chart

```bash
# Lint chart
helm lint ./quadrant-vms

# Dry-run installation
helm install quadrant-vms ./quadrant-vms -n quadrant-vms --dry-run --debug

# Template rendering
helm template quadrant-vms ./quadrant-vms -n quadrant-vms > rendered.yaml
```

## Values Reference

See [values.yaml](quadrant-vms/values.yaml) for complete configuration options.

### Key Configuration Options

| Parameter | Description | Default |
|-----------|-------------|---------|
| `global.namespace` | Kubernetes namespace | `quadrant-vms` |
| `global.storageClass` | Storage class for PVCs | `standard` |
| `image.tag` | Default image tag for all services | `latest` |
| `coordinator.replicas` | Number of coordinator replicas | `3` |
| `infrastructure.postgres.enabled` | Enable embedded PostgreSQL | `true` |
| `infrastructure.postgres.persistence.size` | PostgreSQL storage size | `50Gi` |
| `infrastructure.minio.enabled` | Enable embedded MinIO | `true` |
| `ingress.enabled` | Enable ingress | `true` |
| `ingress.hosts` | Ingress hosts configuration | `vms.example.com` |
| `monitoring.enabled` | Enable Prometheus monitoring | `true` |
| `networkPolicy.enabled` | Enable network policies | `true` |
| `rbac.create` | Create RBAC resources | `true` |

## Monitoring

### Prometheus Integration

The chart includes ServiceMonitor CRDs for Prometheus Operator:

```yaml
monitoring:
  enabled: true
  serviceMonitor:
    enabled: true
    interval: 30s
  prometheusRule:
    enabled: true
```

Access metrics endpoints:

```bash
# Port forward to any service metrics endpoint
kubectl port-forward -n quadrant-vms svc/coordinator 9091:9091

# Metrics available at http://localhost:9091/metrics
```

### Grafana Dashboards

Pre-built dashboards available in `docs/grafana/`.

## Troubleshooting

### Check Pod Logs

```bash
# View logs for specific service
kubectl logs -n quadrant-vms -l app=coordinator --tail=100 -f

# View all logs
kubectl logs -n quadrant-vms --all-containers=true --tail=100
```

### Describe Resources

```bash
# Check pod status
kubectl describe pod -n quadrant-vms <pod-name>

# Check events
kubectl get events -n quadrant-vms --sort-by=.metadata.creationTimestamp
```

### Port Forward for Debugging

```bash
# Coordinator
kubectl port-forward -n quadrant-vms svc/coordinator 8082:8082

# PostgreSQL
kubectl port-forward -n quadrant-vms svc/postgres 5432:5432

# MinIO Console
kubectl port-forward -n quadrant-vms svc/minio 9001:9001
```

### Common Issues

**Issue**: Pods stuck in `Pending` state
- Check storage provisioner is available
- Check node resources (CPU/RAM)
- Check PVC binding: `kubectl get pvc -n quadrant-vms`

**Issue**: Image pull errors
- Verify image exists: `docker pull quadrant-vms/coordinator:latest`
- Check imagePullSecrets configuration
- Use local registry or configure proper image paths

**Issue**: Database connection errors
- Check postgres pod is running
- Verify DATABASE_URL secret is correct
- Check network policies allow communication

## Production Recommendations

1. **Use external managed databases** (AWS RDS, Google Cloud SQL, etc.)
2. **Configure persistent storage** with appropriate storage class
3. **Enable TLS/SSL** for ingress with cert-manager
4. **Set resource limits** appropriate for your workload
5. **Enable monitoring** with Prometheus and Grafana
6. **Configure backups** for database and recordings
7. **Use secrets management** (Sealed Secrets, External Secrets Operator, Vault)
8. **Enable network policies** for pod-to-pod security
9. **Configure pod disruption budgets** for high availability
10. **Test disaster recovery** procedures regularly

## Additional Resources

- [Kubernetes Documentation](https://kubernetes.io/docs/)
- [Helm Documentation](https://helm.sh/docs/)
- [Quadrant VMS Documentation](../../README.md)
- [Kustomize Deployment](../README.md)

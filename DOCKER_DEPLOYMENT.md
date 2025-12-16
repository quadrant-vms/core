# Docker Compose Deployment Guide

This guide explains how to deploy a complete Quadrant VMS cluster using Docker Compose.

---

## 🏗️ Architecture Overview

The Docker Compose deployment includes:

### Infrastructure Services
- **PostgreSQL 16** - Persistent data storage for all services
- **MinIO** - S3-compatible object storage for video files
- **Redis** - Caching layer (optional, for future features)

### Core Services
- **Coordinator** (Port 8082) - Distributed job scheduler with lease management
- **Admin Gateway** (Port 8081) - REST API facade for orchestration
- **Stream Node** (Port 8080) - RTSP to HLS transcoding
- **Recorder Node** (Port 8085) - Recording pipeline

### Management Services
- **Auth Service** (Port 8086) - Authentication, authorization, RBAC
- **Device Manager** (Port 8087) - Camera/device management
- **Alert Service** (Port 8089) - Event-driven alerts and notifications

### Intelligence & Frontend
- **AI Service** (Port 8088) - AI plugin system with YOLOv8, pose estimation, etc.
- **Playback Service** (Port 8084) - Multi-protocol playback delivery
- **Operator UI** (Port 8090) - Web-based dashboard

---

## 🚀 Quick Start

### Prerequisites

- Docker 20.10+ and Docker Compose v2.0+
- 8GB+ RAM recommended (4GB minimum)
- 20GB+ disk space for storage volumes

### Step 1: Copy Environment File

```bash
cp .env.docker .env
```

### Step 2: Customize Configuration

Edit the `.env` file to customize your deployment:

```bash
# Important: Change the JWT secret in production!
JWT_SECRET=your-super-secret-jwt-key-change-this-in-production

# Configure email notifications (optional)
SMTP_HOST=smtp.gmail.com
SMTP_PORT=587
SMTP_USERNAME=your-email@gmail.com
SMTP_PASSWORD=your-app-password
SMTP_FROM=vms@your-domain.com

# Enable GPU acceleration (if NVIDIA GPU available)
ENABLE_GPU=true
```

### Step 3: Build and Start Services

```bash
# Build all service images
docker compose build

# Start the entire stack
docker compose up -d

# View logs
docker compose logs -f

# Check service status
docker compose ps
```

### Step 4: Initialize Database

The database schema will be automatically created on first run. You can verify by checking the coordinator logs:

```bash
docker compose logs coordinator | grep migration
```

### Step 5: Access Services

Once all services are healthy, you can access:

- **Operator UI**: http://localhost:8090 (Main web dashboard)
- **Admin Gateway**: http://localhost:8081 (REST API)
- **MinIO Console**: http://localhost:9001 (S3 storage UI)
  - Username: `minio` (or value from `MINIO_ROOT_USER`)
  - Password: `minio123` (or value from `MINIO_ROOT_PASSWORD`)

---

## 📊 Service Health Checks

All services include health checks. Monitor them with:

```bash
# Check all service health
docker compose ps

# Check specific service logs
docker compose logs -f coordinator
docker compose logs -f operator-ui

# Restart a specific service
docker compose restart stream-node
```

---

## 🎯 Common Use Cases

### Starting a Live Stream

```bash
# Using the Operator UI (recommended)
# 1. Open http://localhost:8090
# 2. Navigate to "Devices" tab
# 3. Add a camera with RTSP URL
# 4. Navigate to "Live Streams" tab
# 5. Click "Start Stream"

# Or using the REST API
curl -X POST http://localhost:8081/streams \
  -H "Content-Type: application/json" \
  -d '{
    "stream_id": "camera-001",
    "source_uri": "rtsp://username:password@camera-ip:554/stream",
    "lease_ttl_secs": 300
  }'
```

### Starting a Recording

```bash
# Using the REST API
curl -X POST http://localhost:8085/recordings \
  -H "Content-Type: application/json" \
  -d '{
    "recording_id": "rec-001",
    "source_uri": "rtsp://username:password@camera-ip:554/stream",
    "output_format": "mp4",
    "duration_secs": 3600
  }'
```

### Running AI Analysis

```bash
# Create an AI task
curl -X POST http://localhost:8088/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "task_id": "ai-001",
    "plugin_id": "yolov8_detector",
    "source": {"type": "stream", "stream_id": "camera-001"},
    "config": {"confidence_threshold": 0.5}
  }'
```

---

## 🔧 Configuration Details

### Environment Variables

See [.env.docker](.env.docker) for all available configuration options.

Key configuration categories:

- **Infrastructure**: PostgreSQL, MinIO, Redis settings
- **Coordinator**: Lease TTL, clustering options
- **Authentication**: JWT secrets, token expiry
- **Storage**: S3 endpoints, bucket names
- **AI Service**: Model paths, GPU settings
- **Alerts**: SMTP configuration

### Storage Volumes

The deployment uses named volumes for persistent data:

- `postgres-data` - Database data
- `minio-data` - S3 object storage
- `redis-data` - Redis cache
- `hls-data` - HLS stream segments
- `recordings-data` - Video recordings
- `ai-models` - AI model files

To backup data:

```bash
# Backup PostgreSQL
docker compose exec postgres pg_dump -U vms quadrant_vms > backup.sql

# Backup MinIO data
docker compose exec minio mc mirror /data /backup
```

### Network Configuration

All services communicate over a dedicated `vms-network` bridge network. Services are accessible by their container names within this network.

---

## 🚦 Scaling Services

### Scaling Worker Nodes

You can scale stream and recorder nodes horizontally:

```bash
# Scale stream nodes to 3 instances
docker compose up -d --scale stream-node=3

# Scale recorder nodes to 2 instances
docker compose up -d --scale recorder-node=2
```

**Note**: When scaling, ensure each instance has a unique `NODE_ID`. For production deployments, use Kubernetes or Docker Swarm for advanced orchestration.

### Enabling Coordinator Clustering

For high availability, enable multi-coordinator clustering:

1. Edit `.env`:
```bash
CLUSTER_ENABLED=true
CLUSTER_PEERS=coordinator-2:8082,coordinator-3:8082
```

2. Deploy multiple coordinators with unique node IDs:
```bash
docker compose up -d coordinator
docker compose --scale coordinator=3 up -d
```

---

## 🐛 Troubleshooting

### Services Not Starting

1. **Check logs**:
```bash
docker compose logs [service-name]
```

2. **Verify database connection**:
```bash
docker compose exec postgres psql -U vms -d quadrant_vms -c '\dt'
```

3. **Check MinIO accessibility**:
```bash
docker compose exec stream-node curl -v http://minio:9000/minio/health/live
```

### Database Connection Errors

If services can't connect to PostgreSQL:

```bash
# Restart PostgreSQL
docker compose restart postgres

# Wait for it to be healthy
docker compose ps postgres

# Check connectivity
docker compose exec coordinator curl -v http://postgres:5432
```

### MinIO Bucket Not Found

The services expect specific S3 buckets. Create them manually:

```bash
# Access MinIO console at http://localhost:9001
# Or use mc client:
docker compose exec minio mc alias set local http://localhost:9000 minio minio123
docker compose exec minio mc mb local/vms-streams
docker compose exec minio mc mb local/vms-recordings
```

### Out of Memory

If containers are being killed:

1. Increase Docker memory limit (Docker Desktop Settings)
2. Reduce concurrent services:
```bash
# Stop non-essential services
docker compose stop ai-service alert-service
```

### Permission Issues with Volumes

If you encounter permission errors:

```bash
# Fix volume permissions
docker compose down
docker volume rm vms_postgres-data vms_minio-data
docker compose up -d
```

---

## 🔒 Production Deployment

For production environments, follow these best practices:

### Security Hardening

1. **Change all default passwords**:
```bash
# Generate strong passwords
openssl rand -base64 32

# Update in .env:
POSTGRES_PASSWORD=<strong-password>
MINIO_ROOT_PASSWORD=<strong-password>
JWT_SECRET=<strong-secret>
```

2. **Use TLS/SSL**:
- Configure reverse proxy (Nginx, Traefik) with Let's Encrypt
- Enable HTTPS for all external endpoints
- Use TLS for PostgreSQL connections

3. **Restrict network access**:
```yaml
# In docker-compose.yml, remove port mappings for internal services
# Only expose operator-ui and admin-gateway
```

4. **Enable audit logging**:
```bash
RUST_LOG=info,auth_service=debug
```

### High Availability

1. **Use external PostgreSQL**:
```bash
DATABASE_URL=postgresql://user:pass@external-postgres.example.com:5432/vms
```

2. **Use external S3** (AWS S3, MinIO cluster):
```bash
S3_ENDPOINT=https://s3.amazonaws.com
S3_BUCKET=my-production-vms-bucket
```

3. **Deploy coordinator cluster**:
```bash
CLUSTER_ENABLED=true
CLUSTER_PEERS=coordinator-1:8082,coordinator-2:8082,coordinator-3:8082
```

4. **Use Kubernetes** for advanced orchestration (see HA_DEPLOYMENT.md)

### Monitoring

1. **Prometheus metrics** are exposed on each service's `/metrics` endpoint

2. **Grafana dashboards** can be imported from `docs/grafana/`

3. **Health checks** are available at each service's `/health` endpoint

### Backup Strategy

```bash
# Daily database backups
0 2 * * * docker compose exec -T postgres pg_dump -U vms quadrant_vms | gzip > /backups/vms-$(date +\%Y\%m\%d).sql.gz

# S3 replication (if using external S3)
# Configure bucket versioning and cross-region replication
```

---

## 📚 Additional Resources

- [README.md](README.md) - Project overview and features
- [SERVICES.md](SERVICES.md) - Detailed service documentation
- [CLAUDE.md](CLAUDE.md) - Development guide
- [HA_DEPLOYMENT.md](docs/HA_DEPLOYMENT.md) - High-availability deployment guide
- [AUTHENTICATION.md](docs/AUTHENTICATION.md) - Authentication and RBAC guide

---

## 🆘 Getting Help

- **GitHub Issues**: https://github.com/yourorg/quadrant-vms/issues
- **Documentation**: Check SERVICES.md for API details
- **Logs**: Always check service logs first: `docker compose logs -f [service]`

---

## 📝 License

See [LICENSE](LICENSE) for details.

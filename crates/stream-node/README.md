# stream-node

> RTSP → HLS streaming microservice (Rust + GStreamer)

`stream-node` is part of the **Quadrant VMS** system.  
It pulls RTSP streams, transcodes them into HLS segments,  
and optionally uploads them to **MinIO (S3 compatible)** storage.

---

## Features

- RTSP ingest via GStreamer
- H.264 / H.265 codec support
- HLS (TS or fMP4 container) output
- MinIO / S3 auto-upload
- Auto-restart supervisor
- Prometheus metrics endpoint
- Simple HTTP API (Axum 0.7)

---

## Environment Variables

| Key | Description | Default |
|-----|--------------|----------|
| `HLS_ROOT` | Local HLS output path | `/data/hls` |
| `S3_ENDPOINT` | MinIO/S3 endpoint | `http://localhost:9000` |
| `S3_ACCESS_KEY` | MinIO access key | `minio` |
| `S3_SECRET_KEY` | MinIO secret key | `minio123` |
| `S3_BUCKET` | Bucket name | `vms` |
| `S3_REGION` | AWS region (for SDK) | `us-east-1` |
| `RESTART_MAX_RETRIES` | Max restart attempts per stream | `5` |
| `RESTART_BACKOFF_MS_START` | Backoff base (ms) | `500` |
| `RESTART_BACKOFF_MS_MAX` | Backoff cap (ms) | `10000` |

---

## REST API

| Method | Path | Description |
|---------|------|-------------|
| `GET` | `/healthz` | Service health check |
| `GET` | `/streams` | List running streams |
| `GET` | `/start?id=<id>&uri=<rtsp>&codec=h264|h265&container=ts|fmp4` | Start a new stream |
| `GET` | `/stop?id=<id>` | Stop a running stream |
| `GET` | `/metrics` | Prometheus metrics |

## Example:
- Check health
```bash
curl http://localhost:8080/healthz
```
- Start a demo stream
```bash
curl "http://localhost:8080/start?id=cam1&uri=rtsp://wowzaec2demo.streamlock.net/vod/mp4:BigBuckBunny_115k.mov&codec=h264&container=ts"
```
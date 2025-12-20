# Operations Guide

This document covers high-level operational concerns (HA, monitoring, GPU).

## High Availability (HA) Basics

Quadrant VMS supports multi-coordinator clustering and stateless workers.
Key concepts:
- Coordinator clustering with leader election.
- StateStore-enabled workers that can resume state after restart.
- Periodic orphan cleanup for failed workers.

See `ENV_VAR_REFERENCE.md` for the exact env vars used by each service.

## Monitoring

Most services expose metrics on `/metrics` via their HTTP servers.
Kubernetes annotations and ServiceMonitor configs should match the actual port
and path used by each service. See `TRACKING_ISSUES.md` for known gaps.

## GPU Acceleration (AI Service)

The AI service supports GPU execution providers for YOLOv8.
Common env vars:
- `YOLOV8_EXECUTION_PROVIDER` (CUDA, TensorRT, CPU)
- `YOLOV8_DEVICE_ID`
- `YOLOV8_MODEL_PATH`

When CUDA/TensorRT are unavailable, the service falls back to CPU.

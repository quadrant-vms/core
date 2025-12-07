# GPU Acceleration for AI Service

## Overview

The Quadrant VMS AI service supports GPU acceleration for YOLOv8 object detection through ONNX Runtime execution providers. This guide explains how to configure and use GPU acceleration.

## Supported Execution Providers

1. **CUDA** - NVIDIA GPU acceleration
2. **TensorRT** - NVIDIA optimized inference engine (faster than CUDA)
3. **CPU** - Fallback CPU execution

## Prerequisites

### For CUDA Support

1. **NVIDIA GPU** with CUDA Compute Capability 6.0 or higher
2. **CUDA Toolkit** (11.8 or later recommended)
3. **cuDNN** (8.6 or later recommended)

```bash
# Check CUDA version
nvidia-smi

# Verify CUDA installation
nvcc --version
```

### For TensorRT Support

1. All CUDA prerequisites
2. **NVIDIA TensorRT** (8.6 or later recommended)

```bash
# Check TensorRT installation
dpkg -l | grep tensorrt
```

## Configuration

### Environment Variables

Configure GPU acceleration using these environment variables:

```bash
# Execution provider (CUDA, TensorRT, or CPU)
export YOLOV8_EXECUTION_PROVIDER=CUDA

# GPU device ID (0, 1, 2, etc. for multi-GPU systems)
export YOLOV8_DEVICE_ID=0

# GPU memory limit in bytes (0 = unlimited)
export YOLOV8_GPU_MEM_LIMIT=0

# Model path
export YOLOV8_MODEL_PATH=models/yolov8n.onnx

# Confidence threshold
export YOLOV8_CONFIDENCE=0.5
```

### JSON Configuration

You can also configure via the plugin configuration JSON:

```json
{
  "model_path": "models/yolov8n.onnx",
  "confidence_threshold": 0.5,
  "iou_threshold": 0.45,
  "max_detections": 100,
  "input_size": 640,
  "execution_provider": "CUDA",
  "device_id": 0,
  "intra_threads": 4,
  "inter_threads": 1,
  "gpu_mem_limit": 0
}
```

## Execution Provider Fallback

The AI service automatically falls back to slower execution providers if the preferred one fails:

1. **TensorRT** → Falls back to CUDA → Falls back to CPU
2. **CUDA** → Falls back to CPU
3. **CPU** → No fallback (always works)

Example log output:
```
INFO Attempting to use CUDA execution provider (device: 0)
INFO Successfully configured CUDA execution provider
INFO Initialized YOLOv8 detector - model: models/yolov8n.onnx, provider: CUDA, device: 0
```

If CUDA is unavailable:
```
INFO Attempting to use CUDA execution provider (device: 0)
WARN Failed with CUDA, using CPU: CUDA not available
INFO Using CPU execution provider
INFO Initialized YOLOv8 detector - model: models/yolov8n.onnx, provider: CPU, device: 0
```

## Performance Metrics

GPU acceleration is tracked via Prometheus metrics:

### Inference Metrics

```
# Total inference operations by provider
ai_service_gpu_inference_total{plugin_type="yolov8_detector", execution_provider="CUDA"}

# Inference time (excluding pre/post processing)
ai_service_inference_time_seconds{plugin_type="yolov8_detector", execution_provider="CUDA"}

# Overall detection latency (including all processing)
ai_service_detection_latency_seconds{plugin_type="yolov8_detector"}
```

### Comparing Performance

Query Prometheus to compare GPU vs CPU performance:

```promql
# Average inference time by provider
rate(ai_service_inference_time_seconds_sum[5m]) / rate(ai_service_inference_time_seconds_count[5m])

# Inference throughput (ops/sec)
rate(ai_service_gpu_inference_total[5m])
```

## Multi-GPU Setup

For systems with multiple GPUs:

```bash
# Use GPU 0
export YOLOV8_DEVICE_ID=0

# Or use GPU 1
export YOLOV8_DEVICE_ID=1

# Check available GPUs
nvidia-smi -L
```

## Performance Tuning

### Thread Configuration

```bash
# Intra-operation threads (parallelism within a single op)
# Higher values can improve CPU preprocessing
export YOLOV8_INTRA_THREADS=4

# Inter-operation threads (parallelism across ops)
# Usually 1 is optimal for GPU workloads
export YOLOV8_INTER_THREADS=1
```

### Model Selection

Different YOLOv8 models have different speed/accuracy tradeoffs:

| Model | Size | Speed | Accuracy |
|-------|------|-------|----------|
| yolov8n.onnx | 6MB | Fastest | Good |
| yolov8s.onnx | 22MB | Fast | Better |
| yolov8m.onnx | 52MB | Medium | Great |
| yolov8l.onnx | 87MB | Slow | Excellent |
| yolov8x.onnx | 136MB | Slowest | Best |

```bash
export YOLOV8_MODEL_PATH=models/yolov8s.onnx  # Use small model
```

## Troubleshooting

### CUDA Not Found

```
WARN Failed with CUDA, using CPU: CUDA not available
```

**Solutions:**
1. Verify CUDA installation: `nvidia-smi`
2. Check CUDA version compatibility with ORT
3. Ensure libcudart.so is in LD_LIBRARY_PATH:
   ```bash
   export LD_LIBRARY_PATH=/usr/local/cuda/lib64:$LD_LIBRARY_PATH
   ```

### Out of Memory

```
ERROR Failed to run inference: Insufficient GPU memory
```

**Solutions:**
1. Use a smaller model (yolov8n instead of yolov8x)
2. Set GPU memory limit:
   ```bash
   export YOLOV8_GPU_MEM_LIMIT=2147483648  # 2GB limit
   ```
3. Use a smaller input size:
   ```json
   {"input_size": 416}  // Instead of 640
   ```

### TensorRT Build Issues

```
WARN Failed with TensorRT, trying CUDA: TensorRT engine build failed
```

**Solutions:**
1. Ensure TensorRT is installed correctly
2. Model may need to be converted to TensorRT format first
3. Fall back to CUDA (still GPU accelerated):
   ```bash
   export YOLOV8_EXECUTION_PROVIDER=CUDA
   ```

## Expected Performance

Typical inference times on NVIDIA RTX 3080 (640x640 input):

| Provider | Avg Inference Time | FPS |
|----------|-------------------|-----|
| CPU (8 cores) | 45-60ms | 16-22 |
| CUDA | 8-12ms | 83-125 |
| TensorRT | 5-8ms | 125-200 |

*Note: Actual performance varies based on GPU, model size, and input resolution*

## Docker Setup

Example docker-compose.yml with GPU support:

```yaml
services:
  ai-service:
    image: quadrant-vms/ai-service:latest
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: 1
              capabilities: [gpu]
    environment:
      - YOLOV8_EXECUTION_PROVIDER=CUDA
      - YOLOV8_DEVICE_ID=0
      - YOLOV8_MODEL_PATH=/models/yolov8n.onnx
    volumes:
      - ./models:/models
```

Run with NVIDIA Container Toolkit:
```bash
docker run --gpus all -e YOLOV8_EXECUTION_PROVIDER=CUDA ai-service
```

## Monitoring GPU Usage

### With nvidia-smi

```bash
# Watch GPU utilization
watch -n 1 nvidia-smi

# Log GPU usage
nvidia-smi --query-gpu=timestamp,name,utilization.gpu,utilization.memory,memory.used,memory.total --format=csv -l 1
```

### With Prometheus

The AI service exposes GPU metrics at `/metrics`:

```
ai_service_gpu_utilization_percent{plugin_type="yolov8_detector", device_id="0"}
```

## References

- [ONNX Runtime Execution Providers](https://onnxruntime.ai/docs/execution-providers/)
- [NVIDIA CUDA Downloads](https://developer.nvidia.com/cuda-downloads)
- [NVIDIA TensorRT](https://developer.nvidia.com/tensorrt)
- [YOLOv8 Documentation](https://docs.ultralytics.com/)

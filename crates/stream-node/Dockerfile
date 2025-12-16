# =============================================================================
# Stream Node Dockerfile
# Multi-stage build with Rust builder and Alpine runtime
# Requires: GStreamer and FFmpeg for video processing
# =============================================================================

# -----------------------------------------------------------------------------
# Stage 1: Builder
# -----------------------------------------------------------------------------
FROM rust:1.84-alpine AS builder

# Install build dependencies
RUN apk add --no-cache \
    musl-dev \
    pkgconfig \
    gstreamer-dev \
    gst-plugins-base-dev \
    gst-plugins-good \
    gst-plugins-bad \
    gst-plugins-ugly \
    gst-libav \
    openssl-dev \
    ca-certificates

# Create app directory
WORKDIR /build

# Copy workspace Cargo files first for better layer caching
COPY Cargo.toml Cargo.lock ./

# Copy all crates (needed for workspace dependencies)
COPY src ./src

COPY crates ./crates

# Build the stream-node binary in release mode
RUN cargo build --release -p stream-node --bin stream-node

# -----------------------------------------------------------------------------
# Stage 2: Runtime
# -----------------------------------------------------------------------------
FROM alpine:3.20

# Install runtime dependencies
RUN apk add --no-cache \
    gstreamer \
    gst-plugins-base \
    gst-plugins-good \
    gst-plugins-bad \
    gst-plugins-ugly \
    gst-libav \
    ffmpeg \
    ca-certificates \
    libgcc \
    libstdc++

# Create non-root user
RUN addgroup -g 1000 vms && \
    adduser -D -u 1000 -G vms vms

# Create data directories
RUN mkdir -p /data/hls && \
    chown -R vms:vms /data

# Copy binary from builder
COPY --from=builder /build/target/release/stream-node /usr/local/bin/stream-node

# Set ownership
RUN chown vms:vms /usr/local/bin/stream-node

# Switch to non-root user
USER vms

# Set working directory
WORKDIR /app

# Expose HTTP API port
EXPOSE 8080

# Environment variables (can be overridden)
ENV HLS_ROOT=/data/hls \
    RUST_LOG=info \
    STREAM_NODE_ADDR=0.0.0.0:8080

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:8080/health || exit 1

# Run the binary
CMD ["/usr/local/bin/stream-node"]

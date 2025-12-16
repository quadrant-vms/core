# =============================================================================
# Recorder Node Dockerfile
# Multi-stage build with Rust builder and Alpine runtime
# Requires: FFmpeg for video recording
# =============================================================================

# -----------------------------------------------------------------------------
# Stage 1: Builder
# -----------------------------------------------------------------------------
FROM rust:1.84-alpine AS builder

# Install build dependencies
RUN apk add --no-cache \
    musl-dev \
    pkgconfig \
    openssl-dev \
    ca-certificates

# Create app directory
WORKDIR /build

# Copy workspace Cargo files first for better layer caching
COPY Cargo.toml Cargo.lock ./

# Copy all crates (needed for workspace dependencies)
COPY src ./src

COPY crates ./crates

# Build the recorder-node binary in release mode
RUN cargo build --release -p recorder-node --bin recorder-node

# -----------------------------------------------------------------------------
# Stage 2: Runtime
# -----------------------------------------------------------------------------
FROM alpine:3.20

# Install runtime dependencies
RUN apk add --no-cache \
    ffmpeg \
    ca-certificates \
    libgcc

# Create non-root user
RUN addgroup -g 1000 vms && \
    adduser -D -u 1000 -G vms vms

# Create data directories
RUN mkdir -p /data/recordings && \
    chown -R vms:vms /data

# Copy binary from builder
COPY --from=builder /build/target/release/recorder-node /usr/local/bin/recorder-node

# Set ownership
RUN chown vms:vms /usr/local/bin/recorder-node

# Switch to non-root user
USER vms

# Set working directory
WORKDIR /app

# Expose HTTP API port
EXPOSE 8085

# Environment variables (can be overridden)
ENV RUST_LOG=info \
    RECORDER_NODE_ADDR=0.0.0.0:8085 \
    RECORDINGS_DIR=/data/recordings

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:8085/health || exit 1

# Run the binary
CMD ["/usr/local/bin/recorder-node"]

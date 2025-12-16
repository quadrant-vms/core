# =============================================================================
# Playback Service Dockerfile
# Multi-stage build with Rust builder and Alpine runtime
# Minimal runtime - no special dependencies
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

# Build the playback-service binary in release mode
RUN cargo build --release -p playback-service --bin playback-service

# -----------------------------------------------------------------------------
# Stage 2: Runtime
# -----------------------------------------------------------------------------
FROM alpine:3.20

# Install minimal runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    libgcc

# Create non-root user
RUN addgroup -g 1000 vms && \
    adduser -D -u 1000 -G vms vms

# Create data directories
RUN mkdir -p /data/cache && \
    chown -R vms:vms /data

# Copy binary from builder
COPY --from=builder /build/target/release/playback-service /usr/local/bin/playback-service

# Set ownership
RUN chown vms:vms /usr/local/bin/playback-service

# Switch to non-root user
USER vms

# Set working directory
WORKDIR /app

# Expose HTTP API port
EXPOSE 8090

# Environment variables (can be overridden)
ENV RUST_LOG=info \
    PLAYBACK_SERVICE_ADDR=0.0.0.0:8090 \
    CACHE_DIR=/data/cache \
    CACHE_MAX_SIZE_MB=1024 \
    CACHE_TTL_SECS=3600

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:8090/health || exit 1

# Run the binary
CMD ["/usr/local/bin/playback-service"]

# =============================================================================
# Alert Service Dockerfile
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

# Build the alert-service binary in release mode
RUN cargo build --release -p alert-service --bin alert-service

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

# Copy binary from builder
COPY --from=builder /build/target/release/alert-service /usr/local/bin/alert-service

# Set ownership
RUN chown vms:vms /usr/local/bin/alert-service

# Switch to non-root user
USER vms

# Set working directory
WORKDIR /app

# Expose HTTP API port
EXPOSE 8089

# Environment variables (can be overridden)
ENV RUST_LOG=info \
    ALERT_SERVICE_ADDR=0.0.0.0:8089 \
    SMTP_HOST=localhost \
    SMTP_PORT=587 \
    MQTT_BROKER_URL=mqtt://localhost:1883

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:8089/health || exit 1

# Run the binary
CMD ["/usr/local/bin/alert-service"]

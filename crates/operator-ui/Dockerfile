# =============================================================================
# Operator UI Dockerfile
# Multi-stage build with Node.js frontend builder, Rust backend builder, and Alpine runtime
# =============================================================================

# -----------------------------------------------------------------------------
# Stage 1: Frontend Builder
# -----------------------------------------------------------------------------
FROM node:20-alpine AS frontend-builder

WORKDIR /build/frontend

# Copy frontend package files
COPY crates/operator-ui/frontend/package.json crates/operator-ui/frontend/package-lock.json* ./

# Install frontend dependencies
RUN npm ci --only=production

# Copy frontend source
COPY crates/operator-ui/frontend ./

# Build frontend static assets
RUN npm run build

# -----------------------------------------------------------------------------
# Stage 2: Backend Builder
# -----------------------------------------------------------------------------
FROM rust:1.83-alpine AS backend-builder

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

# Build the operator-ui binary in release mode
RUN cargo build --release -p operator-ui --bin operator-ui

# -----------------------------------------------------------------------------
# Stage 3: Runtime
# -----------------------------------------------------------------------------
FROM alpine:3.20

# Install minimal runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    libgcc

# Create non-root user
RUN addgroup -g 1000 vms && \
    adduser -D -u 1000 -G vms vms

# Create static files directory
RUN mkdir -p /app/static && \
    chown -R vms:vms /app

# Copy binary from backend builder
COPY --from=backend-builder /build/target/release/operator-ui /usr/local/bin/operator-ui

# Copy frontend static assets from frontend builder
COPY --from=frontend-builder /build/frontend/dist /app/static

# Set ownership
RUN chown -R vms:vms /usr/local/bin/operator-ui /app/static

# Switch to non-root user
USER vms

# Set working directory
WORKDIR /app

# Expose HTTP API port
EXPOSE 8091

# Environment variables (can be overridden)
ENV RUST_LOG=info \
    OPERATOR_UI_ADDR=0.0.0.0:8091 \
    STATIC_DIR=/app/static \
    COORDINATOR_URL=http://coordinator:8082 \
    STREAM_NODE_URL=http://stream-node:8080 \
    RECORDER_NODE_URL=http://recorder-node:8085 \
    AI_SERVICE_URL=http://ai-service:8084 \
    AUTH_SERVICE_URL=http://auth-service:8087 \
    DEVICE_MANAGER_URL=http://device-manager:8088 \
    ALERT_SERVICE_URL=http://alert-service:8089 \
    PLAYBACK_SERVICE_URL=http://playback-service:8090

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:8091/healthz || exit 1

# Run the binary
CMD ["/usr/local/bin/operator-ui"]

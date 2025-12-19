# =============================================================================
# Coordinator Dockerfile
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

# Build the coordinator binary in release mode
RUN cargo build --release -p coordinator --bin coordinator

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
COPY --from=builder /build/target/release/coordinator /usr/local/bin/coordinator

# Set ownership
RUN chown vms:vms /usr/local/bin/coordinator

# Switch to non-root user
USER vms

# Set working directory
WORKDIR /app

# Expose HTTP API port
EXPOSE 8082

# Environment variables (can be overridden)
ENV RUST_LOG=info \
    COORDINATOR_ADDR=0.0.0.0:8082 \
    LEASE_STORE_TYPE=postgres \
    LEASE_DEFAULT_TTL_SECS=30 \
    LEASE_MAX_TTL_SECS=300

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:8082/healthz || exit 1

# Run the binary
CMD ["/usr/local/bin/coordinator"]

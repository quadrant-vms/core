FROM rust:1.81 as builder
WORKDIR /app

# system deps for glib/gio/gstreamer
RUN apt-get update && apt-get install -y \
    pkg-config build-essential \
    libglib2.0-dev \
    libgstreamer1.0-dev \
    libgstreamer-plugins-base1.0-dev \
 && rm -rf /var/lib/apt/lists/*

# cache deps
COPY Cargo.toml ./
COPY crates/common/Cargo.toml crates/common/Cargo.toml
COPY crates/telemetry/Cargo.toml crates/telemetry/Cargo.toml
COPY crates/stream-node/Cargo.toml crates/stream-node/Cargo.toml
RUN mkdir -p crates/common/src crates/telemetry/src crates/stream-node/src && \
    echo "fn main(){}" > crates/stream-node/src/main.rs && \
    echo "pub fn x(){}" > crates/common/src/lib.rs && \
    echo "pub fn x(){}" > crates/telemetry/src/lib.rs && \
    cargo build -p stream-node --release

# build real
COPY . .
RUN cargo build -p stream-node --release
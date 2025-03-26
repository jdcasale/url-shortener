# ----------------------------------------
# STAGE 1: base image with all build deps
# ----------------------------------------
FROM rustlang/rust:nightly-slim AS chef
WORKDIR /usr/src/app

# Install dependencies for RocksDB and bindgen
RUN apt-get update && apt-get install -y \
    curl \
    pkg-config libssl-dev \
    libsnappy-dev liblz4-dev libzstd-dev \
    clang llvm-14-dev libclang-14-dev \
    curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Symlink libclang.so so bindgen finds it
RUN if [ ! -f /usr/lib/llvm-14/lib/libclang.so ]; then \
      ln -s /usr/lib/llvm-14/lib/libclang.so.1 /usr/lib/llvm-14/lib/libclang.so; \
    fi

ENV LIBCLANG_PATH=/usr/lib/llvm-14/lib

# Install cargo-chef
RUN cargo install cargo-chef

# ----------------------------------------
# STAGE 2: plan dependencies
# ----------------------------------------
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ----------------------------------------
# STAGE 3: build dependencies (cached if no Cargo.toml changes)
# ----------------------------------------
FROM chef AS cacher
COPY --from=planner /usr/src/app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# ----------------------------------------
# STAGE 4: build actual application
# ----------------------------------------
FROM chef AS builder
COPY . .
COPY --from=cacher /usr/src/app/target /usr/src/app/target
RUN cargo build --release

# ----------------------------------------
# STAGE 5: runtime container
# ----------------------------------------
FROM debian:bookworm-slim

WORKDIR /usr/local/bin

# Install RocksDB runtime deps (no build tools)
RUN apt-get update && apt-get install -y \
    curl \
    libssl3 \
    libsnappy-dev liblz4-dev libzstd-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary
COPY --from=builder /usr/src/app/target/release/url-shortener-server .

# RocksDB data directory
RUN mkdir -p /data

ENTRYPOINT ["url-shortener-server"]

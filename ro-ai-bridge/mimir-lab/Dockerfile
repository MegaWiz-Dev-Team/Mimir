# analytics-api — Asgard Analytics data engine HTTP server (ADR-024).
# Multi-stage Rust build. Bundled DuckDB compiles libduckdb from source, so the
# builder needs a C++ toolchain (build-essential). The build CONTEXT must be the
# `ro-ai-bridge` directory so the `skuggi-core` path dependency is available:
#
#   docker build -f mimir-lab/Dockerfile -t asgard-analytics-api:latest .
#   (run from ro-ai-bridge/)

# bookworm-based builder so glibc/libstdc++ match the bookworm runtime below
# (rust:1.93-slim tracks trixie → GLIBC_2.38/CXXABI mismatch on bookworm).
FROM rust:1.93-slim-bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential cmake ca-certificates pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
# path dependency of mimir-lab
COPY skuggi-core skuggi-core
COPY mimir-lab mimir-lab
WORKDIR /build/mimir-lab
RUN cargo build --release --bin analytics-api

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates libstdc++6 libssl3 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/mimir-lab/target/release/analytics-api /usr/local/bin/analytics-api
ENV BIND=0.0.0.0:8091 \
    ANALYTICS_DATA_DIR=/data/analytics
EXPOSE 8091
CMD ["analytics-api"]

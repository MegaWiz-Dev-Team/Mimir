# ══════════════════════════════════════
# 🧠 Mimir API — Rust Multi-Stage Build
# ══════════════════════════════════════
# Context: Mimir repo root (docker-compose.yml sets this)
# Build: docker compose build mimir-api

# ── Stage 1: Build ──
FROM rust:1.88-slim-bookworm AS builder

RUN apt-get update && apt-get install -y \
    pkg-config libssl-dev libmariadb-dev \
    && rm -rf /var/lib/apt/lists/*

# Use /build/ro-ai-bridge as WORKDIR to match host structure
# so include_str!("../../../docs/...") resolves to /build/docs/
WORKDIR /build/ro-ai-bridge

# Copy workspace source (context = Mimir root)
ARG CACHEBUST=1
COPY ro-ai-bridge/Cargo.toml ro-ai-bridge/Cargo.lock ./
COPY ro-ai-bridge/.sqlx .sqlx/
COPY ro-ai-bridge/src src/
COPY ro-ai-bridge/mimir-core-ai mimir-core-ai/
COPY ro-ai-bridge/ro-ai-domain-game ro-ai-domain-game/
COPY ro-ai-bridge/ro-ai-domain-medical ro-ai-domain-medical/
# Sprint 50b additions to the Cargo workspace — must be copied or
# cargo errors "failed to load manifest for workspace member".
COPY ro-ai-bridge/skuggi-core skuggi-core/
COPY ro-ai-bridge/mimir-text-metrics mimir-text-metrics/
COPY ro-ai-bridge/config config/
COPY ro-ai-bridge/migrations migrations/

# Copy docs to /build/docs/ (openapi.yaml referenced via ../../../docs/api/)
COPY docs /build/docs/

# Build release (offline — no DB needed at compile time)
ENV SQLX_OFFLINE=true
RUN cargo build --release

# ── Stage 2: Runtime ──
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    ca-certificates libssl3 libmariadb3 curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /build/ro-ai-bridge/target/release/ro-ai-bridge /app/ro-ai-bridge
COPY --from=builder /build/ro-ai-bridge/config /app/config

ENV PORT=8080
EXPOSE 8080

HEALTHCHECK --interval=10s --timeout=3s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

CMD ["/app/ro-ai-bridge"]

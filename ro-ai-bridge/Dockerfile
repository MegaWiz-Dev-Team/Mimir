# ══════════════════════════════════════
# 🧠 Mimir API — Rust Multi-Stage Build
# ══════════════════════════════════════
# Build: docker build -t mimir-api .
# Run:   docker run -p 3000:8080 mimir-api

# ── Stage 1: Build ──
FROM rust:1.88-slim-bookworm AS builder

RUN apt-get update && apt-get install -y \
    pkg-config libssl-dev libmariadb-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cache deps first (rarely change)
COPY Cargo.toml Cargo.lock ./
COPY mimir-core-ai/Cargo.toml mimir-core-ai/Cargo.toml
COPY ro-ai-domain-game/Cargo.toml ro-ai-domain-game/Cargo.toml
COPY ro-ai-domain-medical/Cargo.toml ro-ai-domain-medical/Cargo.toml

# Create dummy src files for dependency caching
RUN mkdir -p src mimir-core-ai/src ro-ai-domain-game/src ro-ai-domain-medical/src \
    && echo "fn main() {}" > src/main.rs \
    && echo "// dummy" > mimir-core-ai/src/lib.rs \
    && echo "// dummy" > ro-ai-domain-game/src/lib.rs \
    && echo "// dummy" > ro-ai-domain-medical/src/lib.rs \
    && cargo build --release 2>/dev/null || true

# Copy real source
COPY src src/
COPY mimir-core-ai mimir-core-ai/
COPY ro-ai-domain-game ro-ai-domain-game/
COPY ro-ai-domain-medical ro-ai-domain-medical/
COPY config config/

# Build
RUN cargo build --release

# ── Stage 2: Runtime ──
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    ca-certificates libssl3 libmariadb3 curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/ro-ai-bridge /app/ro-ai-bridge
COPY --from=builder /app/config /app/config

ENV PORT=8080
EXPOSE 8080

HEALTHCHECK --interval=10s --timeout=3s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

CMD ["/app/ro-ai-bridge"]

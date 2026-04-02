#!/bin/bash
# ═══════════════════════════════════════════════════════════════
# Project Mimir — Quick Deploy Script
# Usage: ./scripts/deploy.sh [--dev|--prod]
# ═══════════════════════════════════════════════════════════════
set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
MODE="${1:---dev}"

info()  { echo -e "${BLUE}ℹ️  $1${NC}"; }
ok()    { echo -e "${GREEN}✅ $1${NC}"; }
warn()  { echo -e "${YELLOW}⚠️  $1${NC}"; }
fail()  { echo -e "${RED}❌ $1${NC}"; exit 1; }

echo ""
echo "╔══════════════════════════════════════════╗"
echo "║   🧠 Project Mimir — Deploy Script      ║"
echo "║   Mode: $(printf '%-32s' "$MODE")║"
echo "╚══════════════════════════════════════════╝"
echo ""

# ─── Step 1: Check prerequisites ────────────────────────────────
info "Checking prerequisites..."

command -v docker >/dev/null 2>&1 || fail "Docker is not installed. Run: brew install docker colima && colima start"
command -v cargo  >/dev/null 2>&1 || fail "Rust is not installed. Run: brew install rust"
command -v node   >/dev/null 2>&1 || fail "Node.js is not installed. Run: brew install node"
command -v npm    >/dev/null 2>&1 || fail "npm is not installed."

ok "All prerequisites found"

# ─── Step 2: Check .env ─────────────────────────────────────────
if [ ! -f "$ROOT_DIR/.env" ]; then
    warn ".env not found — creating from template..."
    cat > "$ROOT_DIR/.env" <<'ENVEOF'
PORT=3001
DATABASE_URL=mysql://mimir:mimir_password@localhost:3306/mimir
MARIADB_URL=mysql://mimir:mimir_password@localhost:3306/mimir
QDRANT_URL=http://localhost:6333
REDIS_URL=redis://localhost:6379
S3_ENDPOINT=http://localhost:9000
S3_BUCKET=mimir-uploads
S3_ACCESS_KEY=minioadmin
S3_SECRET_KEY=minioadmin
S3_REGION=us-east-1
OLLAMA_URL=http://localhost:11434
LOCAL_MODEL=llama3.2
EMBED_MODEL=nomic-embed-text
GEMINI_BASE_URL=https://generativelanguage.googleapis.com
GEMINI_API_KEY=
GEMINI_MODEL=gemini-2.0-flash
HEIMDALL_API_URL=
HEIMDALL_API_KEY=
HEIMDALL_MODEL=llama3
JWT_SECRET=mimir-dev-jwt-secret-change-in-production
VAULT_ADDR=http://localhost:8200
VAULT_TOKEN=
VAULT_MOUNT=secret
VAULT_PATH=mimir/secrets
CRON_TICK_SECONDS=60
ENVEOF
    ok "Created .env with defaults"
else
    ok ".env exists"
fi

# Export env vars
set -a; source "$ROOT_DIR/.env"; set +a

# ─── Step 3: Start Docker services ──────────────────────────────
info "Starting Docker services..."
cd "$ROOT_DIR"
docker compose up -d --remove-orphans 2>&1 | tail -5

# Wait for MariaDB
info "Waiting for MariaDB to be healthy..."
for i in $(seq 1 30); do
    if docker exec mimir_mariadb healthcheck.sh --connect --innodb_initialized 2>/dev/null; then
        break
    fi
    sleep 2
done
ok "Docker services running"

# ─── Step 4: Get Vault token ────────────────────────────────────
if [ -z "$VAULT_TOKEN" ]; then
    info "Fetching Vault root token..."
    sleep 3
    VAULT_TOKEN=$(docker logs mimir_vault 2>&1 | grep "Root Token" | tail -1 | awk '{print $NF}')
    if [ -n "$VAULT_TOKEN" ]; then
        # Update .env with the token
        if grep -q "^VAULT_TOKEN=" "$ROOT_DIR/.env"; then
            sed -i.bak "s|^VAULT_TOKEN=.*|VAULT_TOKEN=$VAULT_TOKEN|" "$ROOT_DIR/.env"
            rm -f "$ROOT_DIR/.env.bak"
        fi
        export VAULT_TOKEN
        ok "Vault token saved: ${VAULT_TOKEN:0:10}..."
    else
        warn "Could not get Vault token — check: docker logs mimir_vault"
    fi
fi

# ─── Step 5: Create S3 bucket ───────────────────────────────────
info "Ensuring S3 bucket exists..."
if command -v mc >/dev/null 2>&1; then
    mc alias set mimir "$S3_ENDPOINT" "$S3_ACCESS_KEY" "$S3_SECRET_KEY" --api S3v4 2>/dev/null || true
    mc mb mimir/"$S3_BUCKET" 2>/dev/null || true
    ok "S3 bucket '$S3_BUCKET' ready"
else
    warn "mc (MinIO client) not installed — run: brew install minio/stable/mc"
fi

# ─── Step 6: Run DB migrations ──────────────────────────────────
info "Running database migrations..."
cd "$ROOT_DIR/ro-ai-bridge"
if command -v sqlx >/dev/null 2>&1; then
    sqlx migrate run --source mimir-core-ai/migrations 2>&1 | tail -5
    ok "Migrations complete"
else
    warn "sqlx-cli not installed — run: cargo install sqlx-cli --no-default-features --features mysql"
fi

# ─── Step 7: Build backend ──────────────────────────────────────
export SQLX_OFFLINE=true   # Use cached query metadata — no live DB needed at compile time
if [ "$MODE" = "--prod" ]; then
    info "Building backend (release)..."
    cargo build --release 2>&1 | tail -3
    BACKEND_BIN="$ROOT_DIR/target/release/ro-ai-bridge"
else
    info "Building backend (debug)..."
    cargo build 2>&1 | tail -3
    BACKEND_BIN="$ROOT_DIR/target/debug/ro-ai-bridge"
fi
ok "Backend built: $BACKEND_BIN"

# ─── Step 8: Build frontend ─────────────────────────────────────
info "Building frontend..."
cd "$ROOT_DIR/ro-ai-dashboard"
npm install --silent 2>&1 | tail -3

if [ "$MODE" = "--prod" ]; then
    npm run build 2>&1 | tail -5
    ok "Frontend built (production)"
else
    ok "Frontend deps installed (dev mode — use 'npm run dev')"
fi

# ─── Done ────────────────────────────────────────────────────────
echo ""
echo "╔══════════════════════════════════════════╗"
echo "║   ✅ Deployment Complete!                ║"
echo "╚══════════════════════════════════════════╝"
echo ""
echo "  To start the backend:"
if [ "$MODE" = "--prod" ]; then
    echo "    cd ro-ai-bridge && $BACKEND_BIN"
else
    echo "    cd ro-ai-bridge && cargo run"
fi
echo ""
echo "  To start the frontend:"
if [ "$MODE" = "--prod" ]; then
    echo "    cd ro-ai-dashboard && npm start"
else
    echo "    cd ro-ai-dashboard && npm run dev"
fi
echo ""
echo "  Dashboard: http://localhost:3000"
echo "  API:       http://localhost:3001"
echo "  Vault UI:  http://localhost:8200"
echo "  Neo4j UI:  http://localhost:7474"
echo ""

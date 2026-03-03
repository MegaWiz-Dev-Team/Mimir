#!/usr/bin/env bash
# ============================================================================
# Project Mimir — First-Time Setup Script (Issue #160)
# Interactive setup: check deps, create .env, build, start, health check.
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# ── Colors ─────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info()  { echo -e "${BLUE}[INFO]${NC}  $*"; }
log_ok()    { echo -e "${GREEN}[OK]${NC}    $*"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }

# ── Check Dependencies ────────────────────────────────────────────────────
check_dependencies() {
    log_info "Checking dependencies..."
    local missing=0

    # Docker
    if command -v docker &>/dev/null; then
        local docker_version
        docker_version=$(docker --version 2>/dev/null | head -1)
        log_ok "Docker: ${docker_version}"
    else
        log_error "Docker not installed"
        echo "  → Install: https://docs.docker.com/get-docker/"
        missing=$((missing + 1))
    fi

    # Docker Compose
    if docker compose version &>/dev/null 2>&1; then
        log_ok "Docker Compose: $(docker compose version --short 2>/dev/null)"
    elif command -v docker-compose &>/dev/null; then
        log_ok "docker-compose: $(docker-compose --version 2>/dev/null)"
    else
        log_error "Docker Compose not found"
        missing=$((missing + 1))
    fi

    # OrbStack (optional, macOS)
    if command -v orbctl &>/dev/null; then
        log_ok "OrbStack detected"
    elif [ "$(uname)" = "Darwin" ]; then
        log_warn "OrbStack not found (recommended for macOS)"
        echo "  → Install: https://orbstack.dev"
    fi

    # Rust/Cargo (optional)
    if command -v cargo &>/dev/null; then
        log_ok "Rust: $(rustc --version 2>/dev/null | head -1)"
    else
        log_warn "Rust not installed (needed for local development)"
    fi

    # Node.js (optional)
    if command -v node &>/dev/null; then
        log_ok "Node.js: $(node --version 2>/dev/null)"
    else
        log_warn "Node.js not installed (needed for dashboard development)"
    fi

    if [ $missing -gt 0 ]; then
        log_error "Missing ${missing} required dependency(ies) — please install and retry"
        exit 1
    fi

    log_ok "All required dependencies present"
}

# ── Validate Environment ──────────────────────────────────────────────────
validate_env() {
    local env_file="${PROJECT_DIR}/.env"

    if [ ! -f "$env_file" ]; then
        return 1
    fi

    local required_vars=(
        "DATABASE_URL"
        "MARIADB_ROOT_PASSWORD"
        "JWT_SECRET"
    )

    local missing=0
    for var in "${required_vars[@]}"; do
        if ! grep -q "^${var}=" "$env_file" 2>/dev/null; then
            log_warn "Missing required variable: ${var}"
            missing=$((missing + 1))
        fi
    done

    return $missing
}

# ── Create .env ───────────────────────────────────────────────────────────
setup_env() {
    local env_file="${PROJECT_DIR}/.env"
    local template="${PROJECT_DIR}/.env.example"

    if [ -f "$env_file" ]; then
        log_info ".env already exists"
        if validate_env; then
            log_ok "Environment variables valid"
            return
        else
            log_warn "Some variables missing — please edit .env"
        fi
        return
    fi

    if [ ! -f "$template" ]; then
        log_error ".env.example template not found"
        exit 1
    fi

    log_info "Creating .env from template..."
    cp "$template" "$env_file"

    # Interactive customization
    echo ""
    echo -e "${CYAN}Let's configure your environment:${NC}"
    echo ""

    read -rp "MariaDB root password [root]: " db_root_pass
    db_root_pass="${db_root_pass:-root}"
    sed -i '' "s/MARIADB_ROOT_PASSWORD=.*/MARIADB_ROOT_PASSWORD=${db_root_pass}/" "$env_file" 2>/dev/null || \
        sed -i "s/MARIADB_ROOT_PASSWORD=.*/MARIADB_ROOT_PASSWORD=${db_root_pass}/" "$env_file"

    read -rp "JWT Secret (leave empty to auto-generate): " jwt_secret
    if [ -z "$jwt_secret" ]; then
        jwt_secret=$(openssl rand -hex 32 2>/dev/null || head -c 64 /dev/urandom | base64 | tr -dc 'a-zA-Z0-9' | head -c 64)
    fi
    sed -i '' "s/JWT_SECRET=.*/JWT_SECRET=${jwt_secret}/" "$env_file" 2>/dev/null || \
        sed -i "s/JWT_SECRET=.*/JWT_SECRET=${jwt_secret}/" "$env_file"

    log_ok ".env created — review at ${env_file}"
}

# ── Start Services ────────────────────────────────────────────────────────
start_services() {
    log_info "Starting infrastructure services..."
    cd "${PROJECT_DIR}"

    local compose_file="docker-compose.yml"
    if [ -f "docker-compose.prod.yml" ]; then
        read -rp "Use production compose file? (y/N): " use_prod
        if [ "$use_prod" = "y" ] || [ "$use_prod" = "Y" ]; then
            compose_file="docker-compose.prod.yml"
        fi
    fi

    docker compose -f "$compose_file" up -d 2>/dev/null || \
        docker-compose -f "$compose_file" up -d 2>/dev/null

    log_ok "Services started"
}

# ── Health Check ──────────────────────────────────────────────────────────
run_health_checks() {
    log_info "Waiting for services to be ready..."
    local max_retries=30
    local retry=0

    # Wait for MariaDB
    echo -n "  MariaDB: "
    while [ $retry -lt $max_retries ]; do
        if docker exec mimir_mariadb healthcheck.sh --connect --innodb_initialized 2>/dev/null; then
            echo -e "${GREEN}ready${NC}"
            break
        fi
        echo -n "."
        retry=$((retry + 1))
        sleep 2
    done
    if [ $retry -eq $max_retries ]; then echo -e "${RED}timeout${NC}"; fi

    # Check Qdrant
    echo -n "  Qdrant: "
    if curl -sf http://localhost:6333/healthz >/dev/null 2>&1; then
        echo -e "${GREEN}ready${NC}"
    else
        echo -e "${YELLOW}not responding${NC}"
    fi

    # Check Redis
    echo -n "  Redis: "
    if docker exec mimir_redis redis-cli ping 2>/dev/null | grep -q PONG; then
        echo -e "${GREEN}ready${NC}"
    else
        echo -e "${YELLOW}not responding${NC}"
    fi

    log_ok "Health checks complete"
}

# ── Main ──────────────────────────────────────────────────────────────────
main() {
    echo ""
    echo -e "${CYAN}═══════════════════════════════════════════════════════${NC}"
    echo -e "${CYAN}  Project Mimir — First-Time Setup${NC}"
    echo -e "${CYAN}═══════════════════════════════════════════════════════${NC}"
    echo ""

    check_dependencies
    echo ""
    setup_env
    echo ""
    start_services
    echo ""
    run_health_checks

    echo ""
    echo -e "${GREEN}═══════════════════════════════════════════════════════${NC}"
    echo -e "${GREEN}  Setup Complete! 🚀${NC}"
    echo -e "${GREEN}═══════════════════════════════════════════════════════${NC}"
    echo ""
    echo "  Next steps:"
    echo "    1. Start backend:   cd ro-ai-bridge && cargo run --bin ro-ai-bridge"
    echo "    2. Start dashboard: cd ro-ai-dashboard && npm run dev"
    echo "    3. Open browser:    http://localhost:3001"
    echo ""
}

main "$@"

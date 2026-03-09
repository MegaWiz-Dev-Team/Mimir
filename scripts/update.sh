#!/usr/bin/env bash
# ============================================================================
# Project Mimir — Update Script (Issue #159)
# Auto-backup before update, pull images, run migrations, health check.
# Rollback on failure.
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
COMPOSE_FILE="${COMPOSE_FILE:-docker-compose.prod.yml}"

# ── Colors ─────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info()  { echo -e "${BLUE}[INFO]${NC}  $*"; }
log_ok()    { echo -e "${GREEN}[OK]${NC}    $*"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }

# ── Pre-flight Check ──────────────────────────────────────────────────────
preflight_check() {
    log_info "Pre-flight checks..."

    if [ ! -f "${PROJECT_DIR}/${COMPOSE_FILE}" ]; then
        log_error "Compose file not found: ${COMPOSE_FILE}"
        exit 1
    fi

    if ! command -v docker &>/dev/null; then
        log_error "Docker not installed"
        exit 1
    fi

    if [ ! -f "${PROJECT_DIR}/.env" ]; then
        log_error ".env file not found — run setup.sh first"
        exit 1
    fi

    log_ok "Pre-flight checks passed"
}

# ── Auto Backup ───────────────────────────────────────────────────────────
auto_backup() {
    log_info "Creating pre-update backup..."
    if [ -x "${SCRIPT_DIR}/backup.sh" ]; then
        bash "${SCRIPT_DIR}/backup.sh"
        log_ok "Pre-update backup completed"
    else
        log_warn "backup.sh not found — skipping backup"
    fi
}

# ── Pull Latest ───────────────────────────────────────────────────────────
pull_images() {
    log_info "Pulling latest images..."
    cd "${PROJECT_DIR}"
    docker compose -f "${COMPOSE_FILE}" pull 2>/dev/null || \
        docker-compose -f "${COMPOSE_FILE}" pull 2>/dev/null
    log_ok "Images updated"
}

# ── Restart Services ──────────────────────────────────────────────────────
restart_services() {
    log_info "Restarting services..."
    cd "${PROJECT_DIR}"
    docker compose -f "${COMPOSE_FILE}" up -d 2>/dev/null || \
        docker-compose -f "${COMPOSE_FILE}" up -d 2>/dev/null
    log_ok "Services restarted"
}

# ── Health Check ──────────────────────────────────────────────────────────
health_check() {
    log_info "Running health checks..."
    local max_retries=30
    local retry=0

    # Wait for Bridge API
    while [ $retry -lt $max_retries ]; do
        if curl -sf http://localhost:3000/health >/dev/null 2>&1; then
            log_ok "Bridge API healthy"
            return 0
        fi
        retry=$((retry + 1))
        sleep 2
    done

    log_error "Health check failed after ${max_retries} retries"
    return 1
}

# ── Rollback ──────────────────────────────────────────────────────────────
do_rollback() {
    log_error "Update failed — initiating rollback..."
    if [ -x "${SCRIPT_DIR}/rollback.sh" ]; then
        bash "${SCRIPT_DIR}/rollback.sh" --auto
    else
        log_error "rollback.sh not found — manual recovery required"
    fi
}

# ── Main ──────────────────────────────────────────────────────────────────
main() {
    echo "═══════════════════════════════════════════════════════"
    echo "  Project Mimir — Update"
    echo "═══════════════════════════════════════════════════════"

    preflight_check
    auto_backup
    pull_images
    restart_services

    if health_check; then
        echo ""
        log_ok "Update completed successfully!"
    else
        do_rollback
        exit 1
    fi
}

main "$@"

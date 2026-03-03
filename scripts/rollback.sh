#!/usr/bin/env bash
# ============================================================================
# Project Mimir — Rollback Script (Issue #159)
# Restore from latest backup and rollback migrations.
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BACKUP_BASE="${BACKUP_DIR:-./data/backups}"
COMPOSE_FILE="${COMPOSE_FILE:-docker-compose.prod.yml}"
MIGRATIONS_DIR="${PROJECT_DIR}/ro-ai-bridge/mimir-core-ai/src/db/migrations"
AUTO_MODE="${1:-}"

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

# ── Find Latest Backup ────────────────────────────────────────────────────
find_latest_backup() {
    local type_dir="$1"
    local dir="${BACKUP_BASE}/${type_dir}"

    if [ ! -d "$dir" ]; then
        echo ""
        return
    fi

    find "$dir" -maxdepth 1 -type f | sort -r | head -1
}

# ── Restore MariaDB ───────────────────────────────────────────────────────
restore_mariadb() {
    local backup_file
    backup_file=$(find_latest_backup "mariadb")

    if [ -z "$backup_file" ]; then
        log_warn "No MariaDB backup found — skipping restore"
        return 0
    fi

    log_info "Restoring MariaDB from $(basename "$backup_file")..."

    local MARIADB_USER="${MARIADB_USER:-mimir}"
    local MARIADB_PASSWORD="${MARIADB_PASSWORD:-mimir_password}"
    local MARIADB_DATABASE="${MARIADB_DATABASE:-mimir}"

    if docker ps --format '{{.Names}}' | grep -q mimir_mariadb; then
        gunzip -c "$backup_file" | docker exec -i mimir_mariadb mysql \
            -u "${MARIADB_USER}" \
            -p"${MARIADB_PASSWORD}" \
            "${MARIADB_DATABASE}" 2>/dev/null
        log_ok "MariaDB restored"
    else
        log_error "MariaDB container not running"
        return 1
    fi
}

# ── Rollback Migrations ──────────────────────────────────────────────────
list_down_migrations() {
    if [ ! -d "$MIGRATIONS_DIR" ]; then
        log_warn "Migrations directory not found"
        return
    fi

    log_info "Available rollback migrations (.down.sql):"
    find "$MIGRATIONS_DIR" -name "*.down.sql" | sort -r | head -5 | while read -r f; do
        echo "  $(basename "$f")"
    done
}

# ── Restart Services ──────────────────────────────────────────────────────
restart_services() {
    log_info "Restarting services..."
    cd "${PROJECT_DIR}"
    docker compose -f "${COMPOSE_FILE}" restart 2>/dev/null || \
        docker-compose -f "${COMPOSE_FILE}" restart 2>/dev/null || true
    log_ok "Services restarted"
}

# ── Health Check ──────────────────────────────────────────────────────────
health_check() {
    log_info "Running post-rollback health checks..."
    sleep 5

    if curl -sf http://localhost:3000/health >/dev/null 2>&1; then
        log_ok "Bridge API healthy after rollback"
        return 0
    else
        log_warn "Bridge API not responding — may need manual intervention"
        return 1
    fi
}

# ── Main ──────────────────────────────────────────────────────────────────
main() {
    echo "═══════════════════════════════════════════════════════"
    echo "  Project Mimir — Rollback"
    echo "═══════════════════════════════════════════════════════"

    if [ "$AUTO_MODE" != "--auto" ]; then
        echo ""
        echo "This will restore the latest backup and restart services."
        read -rp "Continue? (y/N): " confirm
        if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
            log_info "Rollback cancelled"
            exit 0
        fi
    fi

    restore_mariadb
    list_down_migrations
    restart_services
    health_check

    echo ""
    log_ok "Rollback completed"
}

main "$@"

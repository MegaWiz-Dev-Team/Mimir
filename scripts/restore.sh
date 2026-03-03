#!/usr/bin/env bash
# ============================================================================
# Project Mimir — Restore Script (Issue #158)
# Interactive restore from backup.
# ============================================================================
set -euo pipefail

BACKUP_BASE="${BACKUP_DIR:-./data/backups}"

# MariaDB settings
MARIADB_HOST="${MARIADB_HOST:-localhost}"
MARIADB_PORT="${MARIADB_PORT:-3306}"
MARIADB_USER="${MARIADB_USER:-mimir}"
MARIADB_PASSWORD="${MARIADB_PASSWORD:-REDACTED-PW}"
MARIADB_DATABASE="${MARIADB_DATABASE:-mimir}"

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

# ── List Available Backups ────────────────────────────────────────────────
list_backups() {
    local type_dir="$1"
    local dir="${BACKUP_BASE}/${type_dir}"

    if [ ! -d "$dir" ] || [ -z "$(ls -A "$dir" 2>/dev/null)" ]; then
        echo "  (no backups found)"
        return
    fi

    local i=1
    while IFS= read -r file; do
        local size
        size=$(du -sh "$file" 2>/dev/null | cut -f1)
        local basename
        basename=$(basename "$file")
        echo "  ${i}) ${basename} (${size})"
        i=$((i + 1))
    done < <(find "$dir" -maxdepth 1 -type f | sort -r)
}

# ── Restore MariaDB ───────────────────────────────────────────────────────
restore_mariadb() {
    local backup_file="$1"
    log_info "Restoring MariaDB from ${backup_file}..."

    if [[ "$backup_file" == *.gz ]]; then
        if command -v mysql &>/dev/null; then
            gunzip -c "$backup_file" | mysql \
                -h "${MARIADB_HOST}" \
                -P "${MARIADB_PORT}" \
                -u "${MARIADB_USER}" \
                -p"${MARIADB_PASSWORD}" \
                "${MARIADB_DATABASE}"
        elif docker ps --format '{{.Names}}' | grep -q mimir_mariadb; then
            gunzip -c "$backup_file" | docker exec -i mimir_mariadb mysql \
                -u "${MARIADB_USER}" \
                -p"${MARIADB_PASSWORD}" \
                "${MARIADB_DATABASE}"
        else
            log_error "mysql client not found and container not running"
            return 1
        fi
        log_ok "MariaDB restored successfully"
    else
        log_error "Unsupported backup format: ${backup_file}"
        return 1
    fi
}

# ── Restore Config ────────────────────────────────────────────────────────
restore_config() {
    local backup_file="$1"
    log_info "Restoring configuration from ${backup_file}..."

    # Create backup of current config first
    if [ -f .env ]; then
        cp .env ".env.pre-restore.$(date +%Y%m%d_%H%M%S)"
    fi

    tar -xzf "$backup_file" 2>/dev/null
    log_ok "Configuration restored"
}

# ── Health Check ──────────────────────────────────────────────────────────
health_check() {
    log_info "Running post-restore health checks..."

    local checks_passed=0
    local checks_total=0

    # MariaDB
    checks_total=$((checks_total + 1))
    if docker exec mimir_mariadb healthcheck.sh --connect --innodb_initialized 2>/dev/null; then
        log_ok "MariaDB healthy"
        checks_passed=$((checks_passed + 1))
    else
        log_warn "MariaDB health check failed"
    fi

    echo ""
    log_info "Health check: ${checks_passed}/${checks_total} passed"
}

# ── Main ──────────────────────────────────────────────────────────────────
main() {
    echo "═══════════════════════════════════════════════════════"
    echo "  Project Mimir — Restore"
    echo "═══════════════════════════════════════════════════════"

    if [ ! -d "$BACKUP_BASE" ]; then
        log_error "No backup directory found at ${BACKUP_BASE}"
        exit 1
    fi

    echo ""
    echo "Available backup types:"
    echo "  1) MariaDB"
    echo "  2) Config"
    echo "  3) Cancel"
    echo ""

    read -rp "Select type to restore [1-3]: " choice

    case "$choice" in
        1)
            echo ""
            echo "MariaDB backups:"
            list_backups "mariadb"
            echo ""

            local files
            files=($(find "${BACKUP_BASE}/mariadb" -maxdepth 1 -type f | sort -r))
            if [ ${#files[@]} -eq 0 ]; then
                log_error "No MariaDB backups found"
                exit 1
            fi

            read -rp "Select backup number [1-${#files[@]}]: " num
            if [ "$num" -ge 1 ] && [ "$num" -le ${#files[@]} ] 2>/dev/null; then
                local selected="${files[$((num - 1))]}"
                log_info "Selected: $(basename "$selected")"
                read -rp "Confirm restore? (y/N): " confirm
                if [ "$confirm" = "y" ] || [ "$confirm" = "Y" ]; then
                    restore_mariadb "$selected"
                    health_check
                else
                    log_info "Restore cancelled"
                fi
            else
                log_error "Invalid selection"
            fi
            ;;
        2)
            echo ""
            echo "Config backups:"
            list_backups "config"
            echo ""

            local files
            files=($(find "${BACKUP_BASE}/config" -maxdepth 1 -type f | sort -r))
            if [ ${#files[@]} -eq 0 ]; then
                log_error "No config backups found"
                exit 1
            fi

            read -rp "Select backup number [1-${#files[@]}]: " num
            if [ "$num" -ge 1 ] && [ "$num" -le ${#files[@]} ] 2>/dev/null; then
                local selected="${files[$((num - 1))]}"
                read -rp "Confirm restore? (y/N): " confirm
                if [ "$confirm" = "y" ] || [ "$confirm" = "Y" ]; then
                    restore_config "$selected"
                else
                    log_info "Restore cancelled"
                fi
            else
                log_error "Invalid selection"
            fi
            ;;
        3|*)
            log_info "Restore cancelled"
            ;;
    esac
}

main "$@"

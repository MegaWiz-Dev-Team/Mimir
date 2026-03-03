#!/usr/bin/env bash
# ============================================================================
# Project Mimir — Backup Script (Issue #158)
# Automated backup for MariaDB, Qdrant, and configuration files.
# ============================================================================
set -euo pipefail

# ── Configuration ──────────────────────────────────────────────────────────
BACKUP_BASE="${BACKUP_DIR:-./data/backups}"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
DAILY_RETENTION="${BACKUP_DAILY_RETENTION:-7}"
WEEKLY_RETENTION="${BACKUP_WEEKLY_RETENTION:-4}"

# MariaDB settings
MARIADB_HOST="${MARIADB_HOST:-localhost}"
MARIADB_PORT="${MARIADB_PORT:-3306}"
MARIADB_USER="${MARIADB_USER:-mimir}"
MARIADB_PASSWORD="${MARIADB_PASSWORD:-REDACTED-PW}"
MARIADB_DATABASE="${MARIADB_DATABASE:-mimir}"

# Qdrant settings
QDRANT_HOST="${QDRANT_HOST:-localhost}"
QDRANT_PORT="${QDRANT_PORT:-6333}"

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

# ── Directory Setup ────────────────────────────────────────────────────────
setup_dirs() {
    mkdir -p "${BACKUP_BASE}/mariadb"
    mkdir -p "${BACKUP_BASE}/qdrant"
    mkdir -p "${BACKUP_BASE}/config"
    log_info "Backup directories ready at ${BACKUP_BASE}"
}

# ── MariaDB Backup ────────────────────────────────────────────────────────
backup_mariadb() {
    local backup_file="${BACKUP_BASE}/mariadb/mimir_mariadb_${TIMESTAMP}.sql.gz"
    log_info "Backing up MariaDB → ${backup_file}"

    if command -v mysqldump &>/dev/null; then
        mysqldump \
            -h "${MARIADB_HOST}" \
            -P "${MARIADB_PORT}" \
            -u "${MARIADB_USER}" \
            -p"${MARIADB_PASSWORD}" \
            --single-transaction \
            --routines \
            --triggers \
            "${MARIADB_DATABASE}" | gzip > "${backup_file}"
        log_ok "MariaDB backup complete ($(du -sh "${backup_file}" | cut -f1))"
    elif docker ps --format '{{.Names}}' | grep -q mimir_mariadb; then
        docker exec mimir_mariadb mysqldump \
            -u "${MARIADB_USER}" \
            -p"${MARIADB_PASSWORD}" \
            --single-transaction \
            --routines \
            --triggers \
            "${MARIADB_DATABASE}" | gzip > "${backup_file}"
        log_ok "MariaDB backup via Docker complete"
    else
        log_error "mysqldump not found and MariaDB container not running"
        return 1
    fi
}

# ── Qdrant Backup ─────────────────────────────────────────────────────────
backup_qdrant() {
    local backup_file="${BACKUP_BASE}/qdrant/mimir_qdrant_${TIMESTAMP}.snapshot"
    log_info "Creating Qdrant snapshot..."

    local response
    response=$(curl -s -X POST "http://${QDRANT_HOST}:${QDRANT_PORT}/snapshots" \
        -H "Content-Type: application/json" 2>/dev/null || echo "FAIL")

    if [ "$response" = "FAIL" ]; then
        log_warn "Qdrant snapshot API unavailable — skipping"
        return 0
    fi

    echo "$response" > "${backup_file}"
    log_ok "Qdrant snapshot saved"
}

# ── Config Backup ─────────────────────────────────────────────────────────
backup_config() {
    local backup_file="${BACKUP_BASE}/config/mimir_config_${TIMESTAMP}.tar.gz"
    log_info "Backing up configuration files..."

    local files_to_backup=()
    [ -f .env ] && files_to_backup+=(".env")
    [ -f docker-compose.yml ] && files_to_backup+=("docker-compose.yml")
    [ -f docker-compose.prod.yml ] && files_to_backup+=("docker-compose.prod.yml")

    if [ ${#files_to_backup[@]} -gt 0 ]; then
        tar -czf "${backup_file}" "${files_to_backup[@]}" 2>/dev/null
        log_ok "Config backup complete (${#files_to_backup[@]} files)"
    else
        log_warn "No configuration files found to backup"
    fi
}

# ── Retention Cleanup ─────────────────────────────────────────────────────
cleanup_retention() {
    local dir="$1"
    local keep="$2"
    local type_name="$3"

    if [ ! -d "$dir" ]; then return; fi

    local count
    count=$(find "$dir" -maxdepth 1 -type f | wc -l | tr -d ' ')

    if [ "$count" -gt "$keep" ]; then
        local to_delete=$((count - keep))
        log_info "Cleaning ${type_name}: removing ${to_delete} old backup(s) (keeping ${keep})"
        find "$dir" -maxdepth 1 -type f -print0 | sort -z | head -z -n "$to_delete" | xargs -0 rm -f
    fi
}

apply_retention() {
    log_info "Applying retention policy (daily: ${DAILY_RETENTION}, weekly: ${WEEKLY_RETENTION})"
    cleanup_retention "${BACKUP_BASE}/mariadb" "${DAILY_RETENTION}" "MariaDB"
    cleanup_retention "${BACKUP_BASE}/qdrant" "${DAILY_RETENTION}" "Qdrant"
    cleanup_retention "${BACKUP_BASE}/config" "${DAILY_RETENTION}" "Config"
}

# ── Main ──────────────────────────────────────────────────────────────────
main() {
    echo "═══════════════════════════════════════════════════════"
    echo "  Project Mimir — Backup (${TIMESTAMP})"
    echo "═══════════════════════════════════════════════════════"

    setup_dirs

    local failed=0
    backup_mariadb || failed=$((failed + 1))
    backup_qdrant  || failed=$((failed + 1))
    backup_config  || failed=$((failed + 1))

    apply_retention

    echo ""
    if [ "$failed" -eq 0 ]; then
        log_ok "All backups completed successfully"
    else
        log_warn "${failed} backup(s) had issues — check logs above"
    fi

    # Summary
    echo ""
    log_info "Backup summary:"
    for type_dir in mariadb qdrant config; do
        if [ -d "${BACKUP_BASE}/${type_dir}" ]; then
            local count
            count=$(find "${BACKUP_BASE}/${type_dir}" -maxdepth 1 -type f | wc -l | tr -d ' ')
            echo "  ${type_dir}: ${count} backup(s)"
        fi
    done
}

main "$@"

#!/usr/bin/env bash
# ============================================================================
# Project Mimir — Deployment Verification Script (Issue #161)
# Health checks, API smoke tests, frontend verify, resource usage.
# ============================================================================
set -euo pipefail

# ── Colors ─────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info()  { echo -e "${BLUE}[INFO]${NC}  $*"; }
log_ok()    { echo -e "${GREEN}[PASS]${NC}  $*"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
log_fail()  { echo -e "${RED}[FAIL]${NC}  $*"; }

BRIDGE_URL="${BRIDGE_URL:-http://localhost:3000}"
DASHBOARD_URL="${DASHBOARD_URL:-http://localhost:3001}"

PASS=0
FAIL=0
WARN=0

check() {
    local name="$1"
    local result="$2"

    if [ "$result" = "0" ]; then
        log_ok "$name"
        PASS=$((PASS + 1))
    else
        log_fail "$name"
        FAIL=$((FAIL + 1))
    fi
}

check_warn() {
    local name="$1"
    local result="$2"

    if [ "$result" = "0" ]; then
        log_ok "$name"
        PASS=$((PASS + 1))
    else
        log_warn "$name (non-critical)"
        WARN=$((WARN + 1))
    fi
}

# ── Service Health Checks ─────────────────────────────────────────────────
service_health_checks() {
    echo ""
    echo "── Service Health Checks ──────────────────────────────"

    # Bridge API
    local bridge_ok=1
    if curl -sf "${BRIDGE_URL}/health" >/dev/null 2>&1; then bridge_ok=0; fi
    check "Bridge API (${BRIDGE_URL}/health)" "$bridge_ok"

    # MariaDB
    local mariadb_ok=1
    if docker exec mimir_mariadb healthcheck.sh --connect --innodb_initialized 2>/dev/null; then mariadb_ok=0; fi
    check "MariaDB (container health)" "$mariadb_ok"

    # Qdrant
    local qdrant_ok=1
    if curl -sf http://localhost:6333/healthz >/dev/null 2>&1; then qdrant_ok=0; fi
    check "Qdrant (http://localhost:6333/healthz)" "$qdrant_ok"

    # Redis
    local redis_ok=1
    if docker exec mimir_redis redis-cli ping 2>/dev/null | grep -q PONG; then redis_ok=0; fi
    check "Redis (PING/PONG)" "$redis_ok"

    # RustFS
    local rustfs_ok=1
    if curl -sf http://localhost:9000/minio/health/live >/dev/null 2>&1; then rustfs_ok=0; fi
    check_warn "RustFS (health endpoint)" "$rustfs_ok"

    # Vault
    local vault_ok=1
    if curl -sf http://localhost:8201/v1/sys/health >/dev/null 2>&1; then vault_ok=0; fi
    check_warn "Vault (sys/health)" "$vault_ok"
}

# ── API Endpoint Smoke Tests ──────────────────────────────────────────────
api_smoke_tests() {
    echo ""
    echo "── API Endpoint Smoke Tests ───────────────────────────"

    # Health endpoint
    local health_ok=1
    local health_resp
    health_resp=$(curl -sf "${BRIDGE_URL}/health" 2>/dev/null || echo "FAIL")
    if echo "$health_resp" | grep -q '"status":"ok"'; then health_ok=0; fi
    check "GET /health → status:ok" "$health_ok"

    # Auth endpoint (expect 401 without token)
    local auth_ok=1
    local auth_status
    auth_status=$(curl -sf -o /dev/null -w '%{http_code}' "${BRIDGE_URL}/api/v1/auth/me" 2>/dev/null || echo "000")
    if [ "$auth_status" = "401" ] || [ "$auth_status" = "200" ]; then auth_ok=0; fi
    check "GET /api/v1/auth/me → responds (${auth_status})" "$auth_ok"

    # Sources endpoint
    local sources_ok=1
    local sources_status
    sources_status=$(curl -sf -o /dev/null -w '%{http_code}' "${BRIDGE_URL}/api/v1/sources" 2>/dev/null || echo "000")
    if [ "$sources_status" != "000" ]; then sources_ok=0; fi
    check "GET /api/v1/sources → responds (${sources_status})" "$sources_ok"

    # Backup status
    local backup_ok=1
    local backup_status
    backup_status=$(curl -sf -o /dev/null -w '%{http_code}' "${BRIDGE_URL}/api/v1/backup/status" 2>/dev/null || echo "000")
    if [ "$backup_status" != "000" ]; then backup_ok=0; fi
    check "GET /api/v1/backup/status → responds (${backup_status})" "$backup_ok"
}

# ── Frontend Verification ─────────────────────────────────────────────────
frontend_checks() {
    echo ""
    echo "── Frontend Verification ──────────────────────────────"

    # Dashboard accessible
    local dash_ok=1
    local dash_status
    dash_status=$(curl -sf -o /dev/null -w '%{http_code}' "${DASHBOARD_URL}" 2>/dev/null || echo "000")
    if [ "$dash_status" = "200" ]; then dash_ok=0; fi
    check_warn "Dashboard accessible (${DASHBOARD_URL}) → ${dash_status}" "$dash_ok"
}

# ── System Resources ──────────────────────────────────────────────────────
system_resources() {
    echo ""
    echo "── System Resources ─────────────────────────────────────"

    # Docker container stats
    log_info "Container resource usage:"
    docker stats --no-stream --format "  {{.Name}}: CPU={{.CPUPerc}}, MEM={{.MemUsage}}" \
        mimir_mariadb mimir_qdrant mimir_redis 2>/dev/null || log_warn "Could not fetch container stats"

    # Disk usage
    echo ""
    log_info "Data directory sizes:"
    for dir in data/mariadb data/redis data/backups; do
        if [ -d "$dir" ]; then
            echo "  ${dir}: $(du -sh "$dir" 2>/dev/null | cut -f1)"
        fi
    done
}

# ── Summary ───────────────────────────────────────────────────────────────
summary() {
    echo ""
    echo "═══════════════════════════════════════════════════════"
    echo "  Deployment Verification Summary"
    echo "═══════════════════════════════════════════════════════"
    echo ""
    echo -e "  ${GREEN}PASS:${NC} ${PASS}"
    echo -e "  ${RED}FAIL:${NC} ${FAIL}"
    echo -e "  ${YELLOW}WARN:${NC} ${WARN}"
    echo ""

    if [ "$FAIL" -eq 0 ]; then
        echo -e "  ${GREEN}✅ All critical checks passed!${NC}"
    else
        echo -e "  ${RED}❌ ${FAIL} critical check(s) failed — review above${NC}"
        exit 1
    fi
}

# ── Main ──────────────────────────────────────────────────────────────────
main() {
    echo "═══════════════════════════════════════════════════════"
    echo "  Project Mimir — Deployment Verification"
    echo "  $(date '+%Y-%m-%d %H:%M:%S')"
    echo "═══════════════════════════════════════════════════════"

    service_health_checks
    api_smoke_tests
    frontend_checks
    system_resources
    summary
}

main "$@"

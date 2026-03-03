#!/bin/bash
# ============================================================================
# migrate_rollback.sh — Run a rollback (.down.sql) for a specific migration
#
# Usage:
#   ./scripts/migrate_rollback.sh <migration_timestamp>
#
# Example:
#   ./scripts/migrate_rollback.sh 20260301300000
#
# This will find and execute the corresponding .down.sql file from
# mimir-core-ai/migrations/down/
# ============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
MIGRATIONS_DIR="$PROJECT_ROOT/mimir-core-ai/migrations"
DOWN_DIR="$MIGRATIONS_DIR/down"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Load .env for DB credentials
if [ -f "$PROJECT_ROOT/.env" ]; then
    set -a
    source "$PROJECT_ROOT/.env"
    set +a
fi

DB_HOST="${DB_HOST:-127.0.0.1}"
DB_PORT="${DB_PORT:-3306}"
DB_USER="${DB_USER:-mimir}"
DB_PASS="${DB_PASSWORD:-mimir_password}"
DB_NAME="${DB_NAME:-mimir}"

# ─── Validate arguments ─────────────────────────────────────────────────────
if [ $# -lt 1 ]; then
    echo -e "${RED}Error: Missing migration timestamp argument${NC}"
    echo ""
    echo "Usage: $0 <migration_timestamp>"
    echo ""
    echo "Available down migrations:"
    ls -1 "$DOWN_DIR"/*.down.sql 2>/dev/null | while read f; do
        basename "$f" | sed 's/\.down\.sql//'
    done
    exit 1
fi

TARGET="$1"

# Find matching .down.sql file
DOWN_FILE=$(find "$DOWN_DIR" -name "${TARGET}*.down.sql" | head -1)

if [ -z "$DOWN_FILE" ]; then
    echo -e "${RED}Error: No .down.sql found for migration '$TARGET'${NC}"
    echo ""
    echo "Available down migrations:"
    ls -1 "$DOWN_DIR"/*.down.sql 2>/dev/null | while read f; do
        basename "$f" | sed 's/\.down\.sql//'
    done
    exit 1
fi

echo -e "${YELLOW}═══════════════════════════════════════════════════════════════${NC}"
echo -e "${YELLOW}Migration Rollback — Pre-flight Check${NC}"
echo -e "${YELLOW}═══════════════════════════════════════════════════════════════${NC}"
echo ""
echo -e "Target:   ${GREEN}$(basename "$DOWN_FILE")${NC}"
echo -e "Database: ${GREEN}$DB_NAME${NC} @ ${GREEN}$DB_HOST:$DB_PORT${NC}"
echo ""
echo -e "${YELLOW}SQL to execute:${NC}"
echo "───────────────────────────────────────────────────────────────"
cat "$DOWN_FILE"
echo "───────────────────────────────────────────────────────────────"
echo ""

# ─── Confirmation ────────────────────────────────────────────────────────────
read -p "$(echo -e "${RED}⚠  This will modify the database. Continue? [y/N]: ${NC}")" confirm
if [[ "$confirm" != "y" && "$confirm" != "Y" ]]; then
    echo "Aborted."
    exit 0
fi

# ─── Backup before rollback ─────────────────────────────────────────────────
BACKUP_DIR="$PROJECT_ROOT/backups"
mkdir -p "$BACKUP_DIR"
BACKUP_FILE="$BACKUP_DIR/pre_rollback_${TARGET}_$(date +%Y%m%d_%H%M%S).sql"

echo ""
echo -e "${YELLOW}Creating backup...${NC}"
mysqldump -h "$DB_HOST" -P "$DB_PORT" -u "$DB_USER" -p"$DB_PASS" "$DB_NAME" > "$BACKUP_FILE" 2>/dev/null
echo -e "${GREEN}✅ Backup saved: $BACKUP_FILE${NC}"

# ─── Execute rollback ───────────────────────────────────────────────────────
echo ""
echo -e "${YELLOW}Executing rollback...${NC}"
mysql -h "$DB_HOST" -P "$DB_PORT" -u "$DB_USER" -p"$DB_PASS" "$DB_NAME" < "$DOWN_FILE" 2>&1

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✅ Rollback completed successfully!${NC}"
    echo ""
    echo -e "${YELLOW}Note: Remember to also remove the migration entry from _sqlx_migrations:${NC}"
    echo -e "  DELETE FROM _sqlx_migrations WHERE version = ${TARGET};"
else
    echo -e "${RED}❌ Rollback failed! Check the error above.${NC}"
    echo -e "Backup available at: $BACKUP_FILE"
    exit 1
fi

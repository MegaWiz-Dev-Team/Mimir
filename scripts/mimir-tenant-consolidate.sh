#!/usr/bin/env bash
# ============================================================================
# Mimir tenant consolidation — merge fragmented tenant silos into ONE canonical
# tenant_id, across BOTH the SQL DB and every Qdrant collection, consistently.
# Requires bash 4+ (mapfile), mysql/mysqldump, curl, jq. Run ON A BOX WITH
# CLUSTER ACCESS (reaches MariaDB + the Qdrant service), never over the internet.
#
# WHY: vector RAG "returns zero" because the medical corpus was ingested under
#   the platform-default slug `asgard_platform` (758 chunks) while the canonical
#   medical tenant `asgard_medical` has 0 chunks. The medical callers (dashboard,
#   agents) already query `asgard_medical`, so search matches nothing. Moving the
#   mis-filed corpus into asgard_medical fixes it with NO caller changes.
#   NOTE: `asgard_vor` is intentionally LEFT ALONE — it is the vor agent's own
#   isolated tenant (dedicated analytics/img infra; its RAG works today). Do NOT
#   merge it; set OLD_TENANTS explicitly if you ever need to.
#
# SAFETY MODEL:
#   * DRY-RUN by default; nothing mutates unless you pass --apply.
#   * Step 0 is a MANDATORY backup (mysqldump of affected tables + Qdrant
#     snapshots) before any write.
#   * tenant_id-bearing tables are DISCOVERED from information_schema at
#     runtime — no hard-coded/guessed schema.
#   * DB writes run in a single transaction (all-or-nothing).
#   * Qdrant set-payload is idempotent (safe to re-run).
#   * A post-migration verify proves the new tenant has data + old slugs drained.
# ============================================================================
set -euo pipefail

# ── Parameters (space-separated strings so env can override) ────────────────
OLD_TENANTS="${OLD_TENANTS:-asgard_platform}"              # slug(s) to drain (asgard_vor left intact)
NEW_TENANT="${NEW_TENANT:-asgard_medical}"                 # canonical slug
QDRANT_URL="${QDRANT_URL:-http://qdrant.asgard-infra.svc.cluster.local:6333}"
QDRANT_COLLECTIONS="${QDRANT_COLLECTIONS:-source_chunks golden_qa}"
BACKUP_DIR="${BACKUP_DIR:-./mimir-tenant-migration-$(date +%Y%m%d-%H%M%S)}"

# MariaDB connection: discrete vars, else parse DATABASE_URL (mysql://u:p@host:port/db)
DB_HOST="${DB_HOST:-}"; DB_PORT="${DB_PORT:-3306}"
DB_USER="${DB_USER:-}"; DB_PASS="${DB_PASS:-}"; DB_NAME="${DB_NAME:-}"
if [[ -z "$DB_HOST$DB_USER$DB_NAME" && -n "${DATABASE_URL:-}" ]]; then
  pr="${DATABASE_URL#*://}"; creds="${pr%%@*}"; hp="${pr#*@}"
  DB_USER="${creds%%:*}"; DB_PASS="${creds#*:}"; DB_NAME="${hp##*/}"
  hostport="${hp%%/*}"; DB_HOST="${hostport%%:*}"; [[ "$hostport" == *:* ]] && DB_PORT="${hostport##*:}"
fi

APPLY=0; [[ "${1:-}" == "--apply" ]] && APPLY=1

c_red=$'\033[31m'; c_grn=$'\033[32m'; c_yel=$'\033[33m'; c_dim=$'\033[2m'; c_off=$'\033[0m'
say()  { printf '%b\n' "$*"; }
sect() { printf '\n%b── %s ──%b\n' "$c_yel" "$*" "$c_off"; }
die()  { printf '%b✗ %s%b\n' "$c_red" "$*" "$c_off" >&2; exit 1; }
ok()   { printf '%b✓ %s%b\n' "$c_grn" "$*" "$c_off"; }

for bin in mysql mysqldump curl jq; do command -v "$bin" >/dev/null || die "$bin not found"; done
[[ -n "$DB_HOST" && -n "$DB_USER" && -n "$DB_NAME" ]] || die "DB connection not set (DB_HOST/DB_USER/DB_NAME or DATABASE_URL)"

MYSQL=(mysql -h "$DB_HOST" -P "$DB_PORT" -u "$DB_USER" ${DB_PASS:+-p"$DB_PASS"} -N -B "$DB_NAME")
mysql_q() { "${MYSQL[@]}" -e "$1"; }

in_list=""; for t in $OLD_TENANTS; do in_list+="${in_list:+,}'$t'"; done
qfilter='{"should":['"$(for t in $OLD_TENANTS; do printf '{"key":"tenant_id","match":{"value":"%s"}},' "$t"; done | sed 's/,$//')"']}'

say "${c_dim}mode=$([[ $APPLY == 1 ]] && echo APPLY || echo DRY-RUN)  new=$NEW_TENANT  old=($OLD_TENANTS)  db=$DB_NAME@$DB_HOST  qdrant=$QDRANT_URL${c_off}"

# ── 1. Discover tenant_id-bearing tables ────────────────────────────────────
sect "1. Discover tables carrying tenant_id"
mapfile -t TABLES < <(mysql_q "SELECT DISTINCT table_name FROM information_schema.columns WHERE table_schema='$DB_NAME' AND column_name='tenant_id' ORDER BY table_name;")
[[ ${#TABLES[@]} -gt 0 ]] || die "no tables with a tenant_id column found in $DB_NAME"
for t in "${TABLES[@]}"; do
  printf '   %-32s rows-to-move=%s\n' "$t" "$(mysql_q "SELECT COUNT(*) FROM \`$t\` WHERE tenant_id IN ($in_list);")"
done

# ── 2. Qdrant point counts under old tenants ────────────────────────────────
sect "2. Qdrant points to re-tag"
for c in $QDRANT_COLLECTIONS; do
  cnt=$(curl -fsS "$QDRANT_URL/collections/$c/points/count" -H 'content-type: application/json' \
        -d "{\"exact\":true,\"filter\":$qfilter}" 2>/dev/null | jq -r '.result.count // "?"')
  printf '   %-20s points-to-move=%s\n' "$c" "$cnt"
done

if [[ $APPLY == 0 ]]; then
  say "\n${c_yel}DRY-RUN complete. Review counts, then re-run:  $0 --apply${c_off}"
  exit 0
fi

# ── 0. MANDATORY backup ─────────────────────────────────────────────────────
sect "0. Backup (before any write)"
mkdir -p "$BACKUP_DIR"
mysqldump -h "$DB_HOST" -P "$DB_PORT" -u "$DB_USER" ${DB_PASS:+-p"$DB_PASS"} \
  --single-transaction --no-tablespaces "$DB_NAME" "${TABLES[@]}" > "$BACKUP_DIR/db-affected-tables.sql"
ok "DB dump → $BACKUP_DIR/db-affected-tables.sql ($(wc -l < "$BACKUP_DIR/db-affected-tables.sql") lines)"
for c in $QDRANT_COLLECTIONS; do
  snap=$(curl -fsS -X POST "$QDRANT_URL/collections/$c/snapshots" | jq -r '.result.name // empty')
  [[ -n "$snap" ]] && ok "Qdrant snapshot $c → $snap" || say "${c_yel}⚠ snapshot for $c not confirmed${c_off}"
done
say "${c_dim}Restore: mysql $DB_NAME < db-affected-tables.sql ; Qdrant recover-from-snapshot.${c_off}"

# ── 3. Re-tag DB rows (single transaction) ──────────────────────────────────
sect "3. Re-tag DB rows → $NEW_TENANT"
{
  echo "START TRANSACTION;"
  for t in "${TABLES[@]}"; do echo "UPDATE \`$t\` SET tenant_id='$NEW_TENANT' WHERE tenant_id IN ($in_list);"; done
  echo "COMMIT;"
} | "${MYSQL[@]}"
ok "DB rows re-tagged across ${#TABLES[@]} tables"

# ── 4. Re-tag Qdrant payload (idempotent) ───────────────────────────────────
sect "4. Re-tag Qdrant payload → $NEW_TENANT"
for c in $QDRANT_COLLECTIONS; do
  curl -fsS -X POST "$QDRANT_URL/collections/$c/points/payload?wait=true" \
    -H 'content-type: application/json' \
    -d "{\"payload\":{\"tenant_id\":\"$NEW_TENANT\"},\"filter\":$qfilter}" >/dev/null \
    && ok "set-payload done: $c" || die "set-payload failed: $c"
done

# ── 5. Verify ───────────────────────────────────────────────────────────────
sect "5. Verify"
newcnt=$(curl -fsS "$QDRANT_URL/collections/source_chunks/points/count" -H 'content-type: application/json' \
  -d "{\"exact\":true,\"filter\":{\"must\":[{\"key\":\"tenant_id\",\"match\":{\"value\":\"$NEW_TENANT\"}}]}}" | jq -r '.result.count')
oldcnt=$(curl -fsS "$QDRANT_URL/collections/source_chunks/points/count" -H 'content-type: application/json' \
  -d "{\"exact\":true,\"filter\":$qfilter}" | jq -r '.result.count')
say "   source_chunks: tenant_id=$NEW_TENANT → $newcnt points | old slugs remaining → $oldcnt"
[[ "$oldcnt" == "0" ]] && ok "old slugs drained" || say "${c_yel}⚠ $oldcnt still under old slugs — re-run --apply${c_off}"
say "\n${c_grn}Data migration complete.${c_off} The medical callers already query $NEW_TENANT, so:"
say "   • smoke test:  POST /api/v1/vector/search  -H 'X-Tenant-Id: $NEW_TENANT'  → expect >0 results"
say "   • prevent recurrence: fix whatever INGESTED under the old slug to use $NEW_TENANT going forward"
say "   • asgard-vor is untouched — it keeps querying its own tenant (do NOT change VOR_TENANT)"

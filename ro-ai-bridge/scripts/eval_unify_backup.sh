#!/usr/bin/env bash
# ============================================================================
# Eval-unification BACKUP — Step 0. MUST run + verify before applying the core
# migration or the backfill. Dumps every legacy eval table (and evx_* if they
# already exist) to T7, gzips, verifies the gzip, and writes a MANIFEST with
# per-table row counts. Refuses to proceed if any step fails.
#
# Per feedback_backup_before_changes: no implicit backups; verify MANIFEST+gzip.
#
#   DB_HOST=127.0.0.1 DB_PORT=30006 DB_USER=root DB_PASS=*** DB_NAME=mimir \
#     ./ro-ai-bridge/scripts/eval_unify_backup.sh
# ============================================================================
set -euo pipefail

DB_HOST="${DB_HOST:-127.0.0.1}"
DB_PORT="${DB_PORT:-30006}"
DB_USER="${DB_USER:-root}"
DB_NAME="${DB_NAME:-mimir}"
: "${DB_PASS:?set DB_PASS}"

STAMP="$(date +%Y-%m-%d-%H%M%S)"
DEST_ROOT="${DEST_ROOT:-/Volumes/T7 Shield/asgard-backup-eval-unify-${STAMP}}"
DUMP="${DEST_ROOT}/eval_tables.sql"
GZ="${DUMP}.gz"
MANIFEST="${DEST_ROOT}/MANIFEST.txt"

# mariadb-dump, NOT mysqldump (asgard_full_backup_procedure)
DUMP_BIN="$(command -v mariadb-dump || command -v mysqldump)"
CLI_BIN="$(command -v mariadb || command -v mysql)"
AUTH=(-h "$DB_HOST" -P "$DB_PORT" -u "$DB_USER" "-p${DB_PASS}")

LEGACY_TABLES=(
  eval_runs eval_scores eval_summary eval_datasets eval_benchmark_datasets
  rag_eval_runs rag_eval_datasets rag_eval_queries
  ocr_eval_datasets ocr_eval_cases ocr_eval_runs ocr_eval_results
  ocr_layout_eval_runs ocr_layout_eval_items ocr_layout_region_match
)
EVX_TABLES=( evx_target evx_dataset evx_experiment evx_run evx_metric evx_item evx_item_review evx_artifact evx_span )

echo "[*] Backup dest: ${DEST_ROOT}"
[ -d "/Volumes/T7 Shield" ] || { echo "!! T7 not mounted — aborting"; exit 1; }
mkdir -p "$DEST_ROOT"

# Only dump tables that actually exist (evx_* may not yet)
EXISTING=()
for t in "${LEGACY_TABLES[@]}" "${EVX_TABLES[@]}"; do
  if "$CLI_BIN" "${AUTH[@]}" -N -e \
      "SELECT 1 FROM information_schema.tables WHERE table_schema='${DB_NAME}' AND table_name='${t}' LIMIT 1" \
      | grep -q 1; then
    EXISTING+=("$t")
  fi
done
echo "[*] Dumping ${#EXISTING[@]} tables"

"$DUMP_BIN" "${AUTH[@]}" --single-transaction --quick --routines=false \
  "$DB_NAME" "${EXISTING[@]}" > "$DUMP"

# Manifest with row counts BEFORE gzip
{
  echo "eval-unify backup ${STAMP}"
  echo "db=${DB_NAME} host=${DB_HOST}:${DB_PORT}"
  echo "---- row counts ----"
  for t in "${EXISTING[@]}"; do
    c="$("$CLI_BIN" "${AUTH[@]}" -N -e "SELECT COUNT(*) FROM \`${DB_NAME}\`.\`${t}\`")"
    printf "%-28s %s\n" "$t" "$c"
  done
} > "$MANIFEST"

gzip -f "$DUMP"
gzip -t "$GZ" || { echo "!! gzip verification FAILED — backup invalid"; exit 1; }

SIZE="$(stat -f%z "$GZ" 2>/dev/null || stat -c%s "$GZ")"
[ "$SIZE" -gt 0 ] || { echo "!! empty dump — aborting"; exit 1; }
echo "sha256  $(shasum -a 256 "$GZ" | awk '{print $1}')" >> "$MANIFEST"
echo "gz_bytes ${SIZE}" >> "$MANIFEST"

echo "[✓] Backup OK"
echo "    dump:     ${GZ} (${SIZE} bytes)"
echo "    manifest: ${MANIFEST}"
echo "    gzip -t verified. Safe to proceed with migration + backfill."

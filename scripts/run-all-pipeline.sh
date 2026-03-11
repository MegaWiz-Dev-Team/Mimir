#!/usr/bin/env bash
# ============================================================
#  Mimir Auto-Pipeline Batch Runner
#  Runs auto-pipeline on all specified sources sequentially.
#
#  Usage:
#    ./scripts/run-all-pipeline.sh                  # run remaining sources
#    ./scripts/run-all-pipeline.sh 10 11             # run specific sources
# ============================================================

set -euo pipefail

# ── Config ──────────────────────────────────────────────────
BRIDGE_URL="http://localhost:3000"
TENANT_ID="127d37ee-2de2-4094-8993-f7cff046c0ec"
USERNAME="megacare"
PASSWORD="admin123"
PROVIDER="heimdall"
MODEL="mlx-community/Qwen3.5-27B-4bit"
RUN_LABEL="production-qwen27b"
POLL_INTERVAL=30   # seconds between status checks

DB_USER="mimir"
DB_PASS="REDACTED-PW"
DB_NAME="mimir"

# ── Colors ──────────────────────────────────────────────────
G='\033[0;32m'  R='\033[0;31m'  Y='\033[0;33m'
C='\033[0;36m'  W='\033[1;37m'  N='\033[0m'
DIM='\033[2m'   BOLD='\033[1m'

# ── Sources to run ──────────────────────────────────────────
if [ $# -gt 0 ]; then
  SOURCES=("$@")
else
  # Default: all sources that haven't been run with this label
  SOURCES=(12 11 10 14)
  echo -e "${DIM}No sources specified, using default order: ${SOURCES[*]}${N}"
fi

# ── Login ───────────────────────────────────────────────────
get_token() {
  curl -s "${BRIDGE_URL}/api/v1/auth/login" \
    -H 'Content-Type: application/json' \
    -d "{\"username\":\"${USERNAME}\",\"password\":\"${PASSWORD}\"}" \
    | python3 -c 'import json,sys; print(json.load(sys.stdin).get("token",""))' 2>/dev/null
}

# ── Check run status from DB ────────────────────────────────
check_run_status() {
  local run_id="$1"
  python3 -c "
import pymysql
conn = pymysql.connect(host='localhost', user='${DB_USER}', password='${DB_PASS}', database='${DB_NAME}', port=3306)
cur = conn.cursor()
cur.execute('SELECT status FROM pipeline_runs WHERE id = %s', ('${run_id}',))
r = cur.fetchone()
print(r[0] if r else 'unknown')
conn.close()
" 2>/dev/null
}

# ── Get run progress ────────────────────────────────────────
show_progress() {
  local run_id="$1"
  python3 -c "
import pymysql
conn = pymysql.connect(host='localhost', user='${DB_USER}', password='${DB_PASS}', database='${DB_NAME}', port=3306)
cur = conn.cursor()
cur.execute('''
  SELECT step_number, step_name, status, item_count, latency_ms
  FROM pipeline_run_steps WHERE run_id = %s ORDER BY step_number
''', ('${run_id}',))
steps = cur.fetchall()
names = {1:'Chunks', 2:'Embed', 3:'KG', 4:'QA', 5:'Index'}
parts = []
for s in steps:
    n = names.get(s[0], s[1])
    if s[2] == 'completed':
        parts.append(f'✅{n}({s[3]})')
    elif s[2] == 'running':
        # Get live KG count
        cur.execute(\"SELECT COUNT(*) FROM kg_entities WHERE run_label = '${RUN_LABEL}' AND source_id = (SELECT source_id FROM pipeline_runs WHERE id = %s)\", ('${run_id}',))
        kg = cur.fetchone()[0]
        m = (s[4] or 0) // 60000
        parts.append(f'🔄{n}(KG:{kg})')
    elif s[2] == 'skipped':
        parts.append(f'⏭{n}')
    else:
        parts.append(f'⏳{n}')
print(' → '.join(parts) if parts else '⏳ Starting...')
conn.close()
" 2>/dev/null
}

# ── Main ────────────────────────────────────────────────────
echo -e "${BOLD}${C}╔══════════════════════════════════════════════════════════╗${N}"
echo -e "${BOLD}${C}║${N}  ${BOLD}${W}🚀 Mimir Batch Pipeline Runner${N}                         ${BOLD}${C}║${N}"
echo -e "${BOLD}${C}╚══════════════════════════════════════════════════════════╝${N}"
echo -e "  Model:   ${W}${MODEL}${N}"
echo -e "  Sources: ${W}${SOURCES[*]}${N}"
echo -e "  Label:   ${W}${RUN_LABEL}${N}"
echo ""

TOKEN=$(get_token)
if [ -z "$TOKEN" ]; then
  echo -e "${R}❌ Login failed — is bridge running?${N}"
  exit 1
fi
echo -e "  ${G}✅ Login OK${N}"
echo ""

TOTAL=${#SOURCES[@]}
COMPLETED=0
FAILED=0
START_TIME=$(date +%s)

for i in "${!SOURCES[@]}"; do
  SRC_ID=${SOURCES[$i]}
  SEQ=$((i + 1))

  echo -e "${BOLD}${C}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${N}"
  echo -e "  ${BOLD}[${SEQ}/${TOTAL}] Source ${SRC_ID}${N}  $(date '+%H:%M:%S')"
  echo -e "${C}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${N}"

  # Refresh token before each source (in case it expired)
  TOKEN=$(get_token)

  # Trigger pipeline
  RESP=$(curl -s -X POST "${BRIDGE_URL}/api/v1/sources/${SRC_ID}/auto-pipeline" \
    -H "Authorization: Bearer ${TOKEN}" \
    -H "X-Tenant-Id: ${TENANT_ID}" \
    -H "Content-Type: application/json" \
    -d "{
      \"provider\": \"${PROVIDER}\",
      \"model\": \"${MODEL}\",
      \"run_label\": \"${RUN_LABEL}\",
      \"skip_completed\": false
    }" 2>/dev/null)

  RUN_ID=$(echo "$RESP" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("pipeline_run_id",""))' 2>/dev/null)
  SRC_NAME=$(echo "$RESP" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("source_name","Unknown"))' 2>/dev/null)

  if [ -z "$RUN_ID" ]; then
    echo -e "  ${R}❌ Failed to start: ${RESP}${N}"
    FAILED=$((FAILED + 1))
    continue
  fi

  echo -e "  ${G}▶ Started${N} ${SRC_NAME}"
  echo -e "  ${DIM}Run ID: ${RUN_ID}${N}"
  SRC_START=$(date +%s)

  # Poll until done
  while true; do
    sleep "$POLL_INTERVAL"
    STATUS=$(check_run_status "$RUN_ID")
    PROGRESS=$(show_progress "$RUN_ID")
    ELAPSED=$(( $(date +%s) - SRC_START ))
    MINS=$((ELAPSED / 60))
    SECS=$((ELAPSED % 60))

    echo -e "  ${DIM}[${MINS}m${SECS}s]${N} ${PROGRESS}"

    if [ "$STATUS" = "completed" ]; then
      echo -e "  ${G}✅ Source ${SRC_ID} completed!${N}  (${MINS}m${SECS}s)"
      COMPLETED=$((COMPLETED + 1))
      break
    elif [ "$STATUS" = "failed" ]; then
      echo -e "  ${R}❌ Source ${SRC_ID} failed!${N}"
      FAILED=$((FAILED + 1))
      break
    fi
  done

  echo ""
done

# ── Summary ─────────────────────────────────────────────────
TOTAL_TIME=$(( $(date +%s) - START_TIME ))
TOTAL_MINS=$((TOTAL_TIME / 60))
TOTAL_HRS=$((TOTAL_MINS / 60))
REM_MINS=$((TOTAL_MINS % 60))

echo -e "${BOLD}${C}╔══════════════════════════════════════════════════════════╗${N}"
echo -e "${BOLD}${C}║${N}  ${BOLD}${W}🏁 Batch Complete${N}                                      ${BOLD}${C}║${N}"
echo -e "${BOLD}${C}╚══════════════════════════════════════════════════════════╝${N}"
echo -e "  ${G}✅ Completed: ${COMPLETED}${N}"
echo -e "  ${R}❌ Failed:    ${FAILED}${N}"
echo -e "  ⏱  Total:     ${TOTAL_HRS}h ${REM_MINS}m"
echo ""

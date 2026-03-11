#!/usr/bin/env bash
# ============================================================
#  Mimir Auto-Pipeline Monitor
#  Usage: ./scripts/monitor-pipeline.sh [interval_seconds]
# ============================================================

set -euo pipefail

INTERVAL="${1:-10}"
DB_USER="mimir"
DB_PASS="mimir_password"
DB_NAME="mimir"
DB_HOST="localhost"
DB_PORT="3306"

# ── Colors ──────────────────────────────────────────────────
R='\033[0;31m'   G='\033[0;32m'   Y='\033[0;33m'
B='\033[0;34m'   M='\033[0;35m'   C='\033[0;36m'
W='\033[1;37m'   D='\033[0;90m'   N='\033[0m'
BOLD='\033[1m'   DIM='\033[2m'

# ── Check pymysql ───────────────────────────────────────────
if ! python3 -c "import pymysql" 2>/dev/null; then
  echo -e "${R}❌ pymysql not installed. Run: pip3 install pymysql${N}"
  exit 1
fi

# ── Main Loop ───────────────────────────────────────────────
while true; do
  clear
  echo -e "${BOLD}${C}╔══════════════════════════════════════════════════════════════╗${N}"
  echo -e "${BOLD}${C}║${N}  ${BOLD}${W}⚡ Mimir Auto-Pipeline Monitor${N}            ${D}$(date '+%Y-%m-%d %H:%M:%S')${N}  ${BOLD}${C}║${N}"
  echo -e "${BOLD}${C}╚══════════════════════════════════════════════════════════════╝${N}"
  echo ""

  # ── Service Health ──────────────────────────────────────
  echo -e "${BOLD}${W}  🔌 Services${N}"
  echo -e "  ─────────────────────────────────────────────────"

  # Bridge
  if curl -s --max-time 2 http://localhost:3000/api/v1/sources > /dev/null 2>&1; then
    echo -e "  Bridge (3000)     ${G}● UP${N}"
  else
    echo -e "  Bridge (3000)     ${R}● DOWN${N}"
  fi

  # Heimdall
  HEALTH=$(curl -s --max-time 2 http://localhost:8080/health 2>/dev/null || echo '{}')
  GW_STATUS=$(echo "$HEALTH" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('status','down'))" 2>/dev/null || echo "down")
  if [ "$GW_STATUS" = "healthy" ]; then
    echo -e "  Heimdall (8080)   ${G}● HEALTHY${N}"
  elif [ "$GW_STATUS" = "degraded" ]; then
    echo -e "  Heimdall (8080)   ${Y}● DEGRADED${N}"
  else
    echo -e "  Heimdall (8080)   ${R}● DOWN${N}"
  fi

  # Embedding
  if curl -s --max-time 2 http://localhost:8001/health > /dev/null 2>&1; then
    echo -e "  Embedding (8001)  ${G}● UP${N}"
  else
    echo -e "  Embedding (8001)  ${R}● DOWN${N}"
  fi

  echo ""

  # ── Pipeline Status from DB ─────────────────────────────
  python3 -c "
import pymysql, sys, os
from datetime import datetime, timedelta

conn = pymysql.connect(host='${DB_HOST}', user='${DB_USER}', password='${DB_PASS}',
                       database='${DB_NAME}', port=${DB_PORT})
cur = conn.cursor(pymysql.cursors.DictCursor)

# Colors
G, R, Y, C, W, B, N, DIM = '\033[0;32m', '\033[0;31m', '\033[0;33m', '\033[0;36m', '\033[1;37m', '\033[0;34m', '\033[0m', '\033[2m'
BOLD = '\033[1m'

# ── Active/Recent Runs ──
print(f'  {BOLD}{W}📊 Pipeline Runs{N}')
print(f'  ─────────────────────────────────────────────────')

cur.execute('''
    SELECT pr.id, pr.source_id, pr.status, pr.provider, pr.model, 
           pr.run_label, pr.error_message, pr.started_at, pr.finished_at,
           ds.name as source_name,
           (SELECT COUNT(*) FROM chunks WHERE source_id = pr.source_id) as total_chunks
    FROM pipeline_runs pr
    LEFT JOIN data_sources ds ON ds.id = pr.source_id
    WHERE pr.run_label IS NOT NULL
    ORDER BY pr.started_at DESC
    LIMIT 8
''')
runs = cur.fetchall()

for run in runs:
    status = run['status']
    if status == 'running':
        icon, color = '🔄', Y
    elif status == 'completed':
        icon, color = '✅', G
    elif status == 'failed':
        icon, color = '❌', R
    else:
        icon, color = '⏳', DIM

    src_name = (run['source_name'] or 'Unknown')[:35]
    label = (run['run_label'] or '')[:25]
    model_short = (run['model'] or '').split('/')[-1][:20]

    # Duration
    if run['started_at']:
        start = run['started_at']
        end = run['finished_at'] or datetime.utcnow()
        dur = end - start
        mins = int(dur.total_seconds() / 60)
        secs = int(dur.total_seconds() % 60)
        dur_str = f'{mins}m{secs:02d}s'
    else:
        dur_str = '--'

    print(f'  {icon} {color}S{run[\"source_id\"]:02d}{N} {src_name:<35} {color}{status:<10}{N} {DIM}{dur_str:>8}{N}  {DIM}{label}{N}')

# ── Active Run Details ──
active_runs = [r for r in runs if r['status'] == 'running']

for run in active_runs:
    print()
    print(f'  {BOLD}{Y}⚡ Active Run: {run[\"id\"][:8]}...{N}')
    print(f'  {DIM}Source: {run[\"source_name\"]} ({run[\"total_chunks\"]} chunks){N}')
    print(f'  {DIM}Model:  {run[\"model\"]}{N}')
    print()

    cur.execute('''
        SELECT step_number, step_name, status, item_count, latency_ms, error_message
        FROM pipeline_run_steps WHERE run_id = %s ORDER BY step_number
    ''', (run['id'],))
    steps = cur.fetchall()

    step_names = {1: 'Chunk Check', 2: 'Embed Chunks', 3: 'KG Extraction', 4: 'QA Extraction', 5: 'QA Indexing'}

    for step in steps:
        sn = step['step_number']
        name = step_names.get(sn, step['step_name'])
        st = step['status']
        count = step['item_count'] or 0
        lat = step['latency_ms'] or 0

        if st == 'running':
            bar_icon, sc = '▶', Y
        elif st == 'completed':
            bar_icon, sc = '■', G
        elif st == 'skipped':
            bar_icon, sc = '○', DIM
        else:
            bar_icon, sc = '□', DIM

        lat_str = ''
        if lat > 0:
            m = lat // 60000
            s = (lat % 60000) // 1000
            lat_str = f'{m}m{s:02d}s'

        print(f'    {sc}{bar_icon}{N} Step {sn}: {name:<16} {sc}{st:<10}{N} count={count:<6} {DIM}{lat_str}{N}')

        if step['error_message']:
            print(f'      {R}↳ {step[\"error_message\"][:70]}{N}')

    # KG/QA progress
    if run['run_label']:
        cur.execute('SELECT COUNT(*) as c FROM kg_entities WHERE run_label = %s', (run['run_label'],))
        kg = cur.fetchone()['c']
        print()
        print(f'    {C}📈 KG Entities extracted: {BOLD}{kg}{N}')

# ── Summary Table ──
print()
print(f'  {BOLD}{W}📋 All Sources{N}')
print(f'  ─────────────────────────────────────────────────')
print(f'  {DIM}ID  Source                               Chunks  Last Run{N}')

cur.execute('''
    SELECT ds.id, ds.name,
           (SELECT COUNT(*) FROM chunks WHERE source_id = ds.id) as chunks,
           (SELECT status FROM pipeline_runs WHERE source_id = ds.id 
            AND run_label IS NOT NULL ORDER BY started_at DESC LIMIT 1) as last_status,
           (SELECT run_label FROM pipeline_runs WHERE source_id = ds.id
            AND run_label IS NOT NULL ORDER BY started_at DESC LIMIT 1) as last_label
    FROM data_sources ds
    WHERE ds.tenant_id = '127d37ee-2de2-4094-8993-f7cff046c0ec'
    ORDER BY ds.id
''')
for s in cur.fetchall():
    st = s['last_status'] or '—'
    if st == 'completed': sc, icon = G, '✅'
    elif st == 'running': sc, icon = Y, '🔄'
    elif st == 'failed': sc, icon = R, '❌'
    else: sc, icon = DIM, '⬜'
    label = (s['last_label'] or '—')[:20]
    print(f'  {icon} {s[\"id\"]:2d}  {s[\"name\"][:36]:<36}  {s[\"chunks\"]:4d}    {sc}{st:<10}{N} {DIM}{label}{N}')

conn.close()
" 2>/dev/null

  echo ""
  echo -e "  ${DIM}Refreshing every ${INTERVAL}s · Press Ctrl+C to exit${N}"
  sleep "$INTERVAL"
done

#!/usr/bin/env bash
# Quick status check for MLX tournament + Heimdall MLX state.
# Usage:
#   bash scripts/mlx_tournament_status.sh             # one-shot
#   bash scripts/mlx_tournament_status.sh --watch     # auto-refresh every 10s (no `watch` needed)
#   bash scripts/mlx_tournament_status.sh --watch 5   # custom interval (seconds)
set -e

API="${API:-http://localhost:30000}"
TENANT="${TENANT:-asgard_medical}"
LOG="${LOG:-/tmp/mlx_round2.log}"

# Built-in watch loop — works on macOS without brew install watch
if [ "${1:-}" = "--watch" ] || [ "${1:-}" = "-w" ]; then
    INTERVAL="${2:-10}"
    while true; do
        clear
        # Re-invoke self without --watch to render once
        bash "$0"
        printf '\n\033[2m(refreshing every %ss · Ctrl-C to stop)\033[0m\n' "$INTERVAL"
        sleep "$INTERVAL"
    done
fi

bold() { printf '\033[1m%s\033[0m\n' "$*"; }
dim()  { printf '\033[2m%s\033[0m\n' "$*"; }

bold "═══ MLX Tournament Status ($(date +%H:%M:%S)) ═══"

# 1. Active MLX server (port owner is authoritative)
PORT_PID=$(lsof -t -i :8081 -sTCP:LISTEN 2>/dev/null | head -1 || true)
if [ -n "$PORT_PID" ]; then
    ACTIVE=$(ps -p "$PORT_PID" -o command= 2>/dev/null | sed -E 's/.*--model ([^ ]+).*/\1/')
    bold "🤖 Active MLX (port 8081): $ACTIVE"
    dim "   PID $PORT_PID"
else
    echo "🤖 No MLX server bound to port 8081"
fi

# 2. Orphan / zombie mlx_lm.server processes
ORPHAN_PIDS=$(pgrep -u "$USER" -f mlx_lm.server 2>/dev/null | grep -v "^${PORT_PID}$" || true)
if [ -n "$ORPHAN_PIDS" ]; then
    printf '\033[33m⚠️  Orphan mlx_lm.server processes (will be swept on next swap):\033[0m\n'
    for p in $ORPHAN_PIDS; do
        m=$(ps -p "$p" -o command= 2>/dev/null | sed -E 's/.*--model ([^ ]+).*/\1/')
        echo "   PID $p — $m"
    done
fi

echo
bold "📊 Eval runs (latest 8)"
curl -s -m 5 "$API/api/v1/eval/runs?limit=8" -H "X-Tenant-Id: $TENANT" 2>/dev/null \
| python3 -c "
import sys, json
runs = json.load(sys.stdin)
for r in runs[:8]:
    n = r['name'][:50]
    s = r['status']
    cur = r['completed_combinations']; tot = r['total_combinations']
    cost = r.get('total_cost_usd') or 0
    icon = {'COMPLETED':'✅','RUNNING':'🟡','FAILED':'❌','CANCELLED':'⚪','PENDING':'⏸'}.get(s,'?')
    print(f'  {icon} {s:10} {cur:>3}/{tot:<3}  {n:50}  \${cost:.4f}')
"

echo
bold "📜 Tournament script (last 4 lines of $LOG)"
if [ -f "$LOG" ]; then
    tail -4 "$LOG" | sed 's/^/   /'
else
    dim "   (log file not found)"
fi

# 3. Memory snapshot
echo
PAGE_SIZE=16384
read FREE INACT SPEC < <(vm_stat | awk '
    /Pages free:/        { gsub(/\./,""); free=$3 }
    /Pages inactive:/    { gsub(/\./,""); inact=$3 }
    /Pages speculative:/ { gsub(/\./,""); spec=$3 }
    END { print free, inact, spec }')
RECLAIM_GB=$(awk -v f="$FREE" -v i="$INACT" -v s="$SPEC" -v p="$PAGE_SIZE" \
    'BEGIN { printf "%.1f", (f+i+s) * p / 1024 / 1024 / 1024 }')
bold "💾 Reclaimable RAM: ${RECLAIM_GB} GB"

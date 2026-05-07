#!/usr/bin/env bash
# monitor_hbp_bench.sh — live progress / ETA / running averages for an
# in-flight HBp benchmark run produced by bench_typhoon_si_med_hbp.py.
#
# The bench script writes lines like:
#   [12/100] Q: ...
#     gen 7.4s · ans len=300 · think len=1500
#     → acc=3 comp=2 rel=4 safe=1
#
# This monitor parses those lines and produces a one-screen summary.
#
# Usage:
#   scripts/monitor_hbp_bench.sh                    # auto-detect newest bench log + refresh every 5s
#   scripts/monitor_hbp_bench.sh --once             # one-shot snapshot, no loop
#   scripts/monitor_hbp_bench.sh --interval 2       # refresh every 2s
#   scripts/monitor_hbp_bench.sh --log <path>       # explicit log path
#   scripts/monitor_hbp_bench.sh --no-color         # plain output
#   scripts/monitor_hbp_bench.sh --help

set -u
INTERVAL=5
ONCE=0
LOG=""
COLOR=auto

while [[ $# -gt 0 ]]; do
    case "$1" in
        --interval) INTERVAL="$2"; shift 2 ;;
        --once)     ONCE=1; shift ;;
        --log)      LOG="$2"; shift 2 ;;
        --no-color) COLOR=off; shift ;;
        -h|--help)
            sed -n '2,/^$/p' "$0" | sed 's/^# \{0,1\}//'
            exit 0 ;;
        *) echo "unknown arg: $1" >&2; exit 2 ;;
    esac
done

# Color setup — auto disables outside a TTY
if [[ "$COLOR" == "auto" ]]; then
    if [[ -t 1 ]]; then COLOR=on; else COLOR=off; fi
fi
if [[ "$COLOR" == "on" ]]; then
    BOLD=$'\033[1m'; DIM=$'\033[2m'
    GREEN=$'\033[32m'; YELLOW=$'\033[33m'; CYAN=$'\033[36m'; BOLDCYAN=$'\033[1;36m'
    RESET=$'\033[0m'
else
    BOLD=''; DIM=''; GREEN=''; YELLOW=''; CYAN=''; BOLDCYAN=''; RESET=''
fi

# ─── Locate log ───────────────────────────────────────────────────────────
detect_log() {
    local candidates=(
        /tmp/gemma-day3-bench.log
        /tmp/hbp-bench-live.log
    )
    for c in "${candidates[@]}"; do
        [[ -s "$c" ]] && { echo "$c"; return; }
    done
    # Fallback: newest task-output that contains an HBp progress line
    local task_dir="/private/tmp/claude-501/-Users-mimir-Developer/404ecbbc-8c9a-48ad-abc5-4d20255fe6c7/tasks"
    if [[ -d "$task_dir" ]]; then
        local found
        found=$(grep -lE '^\[[0-9]+/[0-9]+\]' "$task_dir"/*.output 2>/dev/null \
                | xargs -r ls -t 2>/dev/null | head -1)
        [[ -n "$found" ]] && { echo "$found"; return; }
    fi
    return 1
}

if [[ -z "$LOG" ]]; then
    LOG=$(detect_log) || { echo "no bench log found; use --log <path>" >&2; exit 3; }
fi
[[ -f "$LOG" ]] || { echo "log not found: $LOG" >&2; exit 4; }

# ─── Snapshot ─────────────────────────────────────────────────────────────
snapshot() {
    [[ "$COLOR" == "on" ]] && clear
    printf '%s═══ HBp BENCH MONITOR %s· %s · refresh %ss ═══%s\n' \
        "$BOLDCYAN" "$CYAN" "$(date +%H:%M:%S)" "$INTERVAL" "$RESET"
    printf 'log: %s%s%s\n\n' "$DIM" "$LOG" "$RESET"

    # ─── Process state — pick the heaviest python (the actual bench worker,
    # not the bash wrapper). pgrep picks all matching pids; we sort by RSS.
    local pid_block
    pid_block=$(pgrep -f 'bench_typhoon_si_med_hbp\.py' | xargs -I{} ps -p {} -o pid=,rss=,etime=,time= 2>/dev/null | sort -k2 -nr | head -1)
    if [[ -n "$pid_block" ]]; then
        # pid rss(KB) etime cpu
        local pid rss_kb etime cpu rss
        pid=$(    echo "$pid_block" | awk '{print $1}')
        rss_kb=$( echo "$pid_block" | awk '{print $2}')
        etime=$(  echo "$pid_block" | awk '{print $3}')
        cpu=$(    echo "$pid_block" | awk '{print $4}')
        if [[ "$rss_kb" =~ ^[0-9]+$ ]] && (( rss_kb > 0 )); then
            rss=$(python3 -c "print(f'{$rss_kb/1024/1024:.1f}GB')")
        else
            rss="-"
        fi
        printf '%s●%s running   pid=%s  uptime=%s  cpu=%s  mem=%s\n' \
            "$GREEN" "$RESET" "$pid" "$etime" "$cpu" "$rss"
    else
        printf '%s○%s bench process not detected (finished or not started)\n' "$YELLOW" "$RESET"
    fi
    echo

    # ─── Progress (last [n/m] line)
    local last
    last=$(grep -E '^\[[0-9]+/[0-9]+\]' "$LOG" | tail -1)
    if [[ -n "$last" ]]; then
        # Use sed capture groups for safe parsing
        local cur total pct
        cur=$(  echo "$last" | sed -nE 's/^\[([0-9]+)\/([0-9]+)\].*/\1/p')
        total=$(echo "$last" | sed -nE 's/^\[([0-9]+)\/([0-9]+)\].*/\2/p')
        cur=${cur:-0}; total=${total:-0}
        if (( total > 0 )); then pct=$((100 * cur / total)); else pct=0; fi
        local bar_w=40 fill empty
        fill=$((bar_w * pct / 100))
        empty=$((bar_w - fill))
        printf 'progress: %s%d%s/%d (%d%%)  [' "$BOLD" "$cur" "$RESET" "$total" "$pct"
        if (( fill > 0 )); then
            printf '%s' "$GREEN"
            local i; for ((i=0; i<fill; i++)); do printf '█'; done
            printf '%s' "$RESET"
        fi
        local i; for ((i=0; i<empty; i++)); do printf '·'; done
        printf ']\n'
    else
        printf 'progress: %s(no [n/m] markers yet — model loading?)%s\n' "$DIM" "$RESET"
    fi
    echo

    # ─── Running averages + ETA via Python (more readable + portable than awk)
    python3 - "$LOG" <<'PY' 2>/dev/null || echo "(stats unavailable)"
import re, sys
log = sys.argv[1]
acc, comp, rel, safe, gen = [], [], [], [], []
last_progress = (0, 0)
phase_markers = []
with open(log, errors="replace") as f:
    for line in f:
        m = re.match(r"\[(\d+)/(\d+)\]", line)
        if m:
            last_progress = (int(m.group(1)), int(m.group(2)))
        m = re.search(r"acc=(\d+)\s+comp=(\d+)\s+rel=(\d+)\s+safe=(\d+)", line)
        if m:
            acc.append(int(m.group(1)))
            comp.append(int(m.group(2)))
            rel.append(int(m.group(3)))
            safe.append(int(m.group(4)))
        m = re.search(r"gen ([\d.]+)s", line)
        if m:
            gen.append(float(m.group(1)))
        if "=====" in line:
            phase_markers.append(line.strip())

n = len(acc)
if n:
    a = sum(acc)/n; c = sum(comp)/n; r = sum(rel)/n; s = sum(safe)/n
    hbp = ((a-1)/4 + (c-1)/4 + (r-1)/4 + s) / 4 * 100
    print(f"running averages over n={n}:")
    print(f"  acc        {a:.2f}")
    print(f"  comp       {c:.2f}")
    print(f"  rel        {r:.2f}")
    print(f"  safe       {s:.2f}  ({sum(1 for x in safe if x==0)} unsafe)")
    print(f"  HBp% est   {hbp:.2f}")
    n_acc1 = sum(1 for x in acc if x==1)
    n_acc5 = sum(1 for x in acc if x==5)
    print(f"  acc=1: {n_acc1}  ·  acc=5: {n_acc5}  ·  bimodal share: {100*(n_acc1+n_acc5)/n:.0f}%")
else:
    print("running averages: (no scored questions yet)")

cur, total = last_progress
if cur > 0 and total > cur and gen:
    recent = gen[-10:]
    avg = sum(recent) / len(recent)
    per_q = avg + 3  # add ~3s for judge call
    rem = total - cur
    eta_min = rem * per_q / 60
    print(f"\nETA: {rem} remaining · {per_q:.1f}s/Q avg · ~{eta_min:.1f} min")

if phase_markers:
    print(f"\nphase markers ({len(phase_markers)}):")
    for m in phase_markers[-3:]:
        print(f"  {m}")
PY
    echo

    # ─── Last 4 scored questions
    printf '%slast 4 scored questions:%s\n' "$BOLD" "$RESET"
    grep -B 2 '→ acc=' "$LOG" | tail -12 | sed 's/^/  /' | head -12
}

# ─── Loop or one-shot ─────────────────────────────────────────────────────
if (( ONCE )); then
    snapshot
else
    trap 'printf "\n%s[monitor stopped]%s\n" "$DIM" "$RESET"; exit 0' INT
    while :; do
        snapshot
        sleep "$INTERVAL"
    done
fi

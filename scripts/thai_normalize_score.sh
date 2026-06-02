#!/usr/bin/env bash
# Thai→EN normalize bench + end-to-end score for ONE model.
#   ./thai_normalize_score.sh <model_id>
# Runs the LLM normalize (Heimdall venv mlx_lm), feeds each English output to
# /knowledge/primekg/resolve, and scores against the gold expected keyword(s).
# Memory guardrail: run ONE model at a time; this checks headroom first.
set -uo pipefail
MODEL="${1:?usage: thai_normalize_score.sh <model_id>}"
VENV=/Users/mimir/Developer/Heimdall/.venv/bin/python
GOLD="$(dirname "$0")/thai_normalize_gold.tsv"
NS=asgard

# --- memory headroom guard ---
free_gb=$(vm_stat | awk '/Pages free/{f=$3}/Pages inactive/{i=$3}END{print (f+i)*16384/1e9}')
echo "## model: $MODEL  (reclaimable mem ~${free_gb}GB)"
awk "BEGIN{exit !($free_gb < 6)}" && { echo "  ⚠ low memory (<6GB) — aborting, free up first"; exit 2; }

POD=$(kubectl -n $NS get pods -l app=mimir-api -o jsonpath='{range .items[?(@.status.containerStatuses[0].ready==true)]}{.metadata.name}{"\n"}{end}' | head -1)

resolve_top() {  # $1 = english term -> prints top resolved node name (lowercased)
  local q; q=$(printf '%s' "$1" | sed "s/\"/'/g")
  kubectl -n $NS exec "$POD" -- curl -s -m10 -X POST http://localhost:8080/api/v1/knowledge/primekg/resolve \
    -H 'Content-Type: application/json' -d "{\"text\":\"$q\",\"limit\":3}" 2>/dev/null \
    | python3 -c "import sys,json
try:
  d=json.load(sys.stdin); n=(d.get('resolved') or [{}])[0].get('name','')
  print((n or '').lower())
except: print('')"
}

# --- run LLM normalize (one process; memory frees on exit) ---
OUT=/tmp/norm_${MODEL//\//_}.tsv
$VENV "$(dirname "$0")/thai_normalize_bench.py" --model "$MODEL" --gold "$GOLD" > "$OUT" 2>/tmp/norm_err.log || {
  echo "  ✗ LLM run failed:"; tail -3 /tmp/norm_err.log | sed 's/^/    /'; exit 1; }

# --- score end-to-end ---
hit=0; tot=0; lat_sum=0
while IFS=$'\t' read -r thai expect _grp; do
  [ -z "$thai" ] && continue
  eng=$(awk -F'\t' -v t="$thai" '$1==t{print $2}' "$OUT")
  ms=$(awk -F'\t' -v t="$thai" '$1==t{print $3}' "$OUT")
  top=$(resolve_top "$eng")
  tot=$((tot+1)); lat_sum=$((lat_sum + ${ms:-0}))
  if echo "$top" | grep -qiE "$expect"; then
    hit=$((hit+1)); mark="✓"
  else mark="✗"; fi
  printf "  %s %-26s →EN '%s' →%s\n" "$mark" "$thai" "$eng" "${top:-(none)}"
done < "$GOLD"
avg=0; [ "$tot" -gt 0 ] && avg=$((lat_sum/tot))
pct=0; [ "$tot" -gt 0 ] && pct=$((hit*100/tot))
echo "  ── $MODEL: $hit/$tot hit (${pct}%)  avg ${avg}ms/term"

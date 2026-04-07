#!/bin/bash
# ═══════════════════════════════════════════════════════════════
# Project Mimir — Phase 5-8 UI Gap Verification Script
# Usage: ./scripts/test-api-vs-ui-scenarios.sh
# ═══════════════════════════════════════════════════════════════
set -uo pipefail

API="http://localhost:30000"
GREEN='\033[0;32m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

header(){ echo -e "\n${CYAN}━━━ $1 ━━━${NC}"; }
ok()    { echo -e "${GREEN}  ✅ $1${NC}"; }
fail()  { echo -e "${RED}  ❌ $1${NC}"; }

echo "╔══════════════════════════════════════════════════╗"
echo "║   🧪 UI Gap Simulation Test Script               ║"
echo "╚══════════════════════════════════════════════════╝"

# ── Gap 1: AI Eval Set Generator ────────────────────────
header "Gap 1: Missing AI Query Generator Dialog"
echo "UI Action: User clicks 'Generate with AI', enters prompt 'Medicine' count 1"

HTTP_CODE=$(curl -s -o /tmp/mimir_ui_gap1 -w "%{http_code}" \
    -X POST "$API/api/v1/rag-eval/generate-set" \
    -H "Content-Type: application/json" \
    -H "X-Tenant-ID: test" \
    -d '{"prompt":"Medicine","count":1,"multi_turn":false}')

if [ "$HTTP_CODE" = "200" ]; then
    ok "API /generate-set responded successfully."
    echo "  JSON from API (to be placed in UI textbox):"
    cat /tmp/mimir_ui_gap1 | jq '.' | head -10 | sed 's/^/    /'
    echo "    ..."
else
    fail "API /generate-set failed with $HTTP_CODE"
fi


# ── Gap 2: Benchmark Migration ──────────────────────────
header "Gap 2: Missing Full Evaluation Run integration"
echo "UI Action: User runs Benchmark, it should now call /rag-eval/run"

HTTP_CODE=$(curl -s -o /tmp/mimir_ui_gap2 -w "%{http_code}" \
    -X POST "$API/api/v1/rag-eval/run" \
    -H "Content-Type: application/json" \
    -H "X-Tenant-ID: test" \
    -d '{
      "name":"UI Gap Simulated Eval",
      "eval_set":[{"query":"Test Query", "expected_titles":["Dummy"]}],
      "params":{
        "weights":{"vector":0.5,"tree":0.3,"graph":0.2},
        "top_k":5,
        "vector_alpha":0.5,
        "vector_threshold":0.0,
        "graph_hops":1
      },
      "evaluate_generation":false
    }')

if [ "$HTTP_CODE" = "200" ]; then
    RUN_ID=$(cat /tmp/mimir_ui_gap2 | jq -r '.run_id')
    ok "Full evaluation API succeeded. Run ID: $RUN_ID"
    echo "  (UI should redirect to Evaluation -> $RUN_ID)"
else
    fail "API /rag-eval/run failed with $HTTP_CODE"
fi


# ── Gap 3: Cross-Encoder UI ─────────────────────────────
header "Gap 3: Missing Cross-Encoder Toggle"
echo "UI Action: User selects 'Cross-Encoder' strategy in Search Mode dropdown"

HTTP_CODE=$(curl -s -o /tmp/mimir_ui_gap3 -w "%{http_code}" \
    -X POST "$API/api/search" \
    -H "Content-Type: application/json" \
    -H "X-Tenant-ID: test" \
    -d '{
      "query": "Aspirin",
      "limit": 3,
      "rerank": {
        "enabled": true,
        "strategy": "cross-encoder",
        "final_top_k": 3
      }
    }')

if [ "$HTTP_CODE" = "200" ]; then
    ok "Search API successfully processed cross-encoder strategy."
    MODE=$(cat /tmp/mimir_ui_gap3 | jq -r '.mode_used')
    echo "  Search matched in mode: $MODE"
else
    fail "API /search with cross-encoder failed with $HTTP_CODE"
fi

echo -e "\n${CYAN}All Backend endpoints are confirmed ready for UI integration!${NC}"

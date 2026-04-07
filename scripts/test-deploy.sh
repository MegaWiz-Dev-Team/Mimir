#!/bin/bash
# ═══════════════════════════════════════════════════════════════
# Project Mimir v0.30.0 — Post-Deploy Smoke Test
# Usage: ./scripts/test-deploy.sh [API_BASE_URL]
# ═══════════════════════════════════════════════════════════════
set -uo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

API="${1:-http://localhost:30000}"
PASS=0
FAIL=0
SKIP=0

info()  { echo -e "${BLUE}ℹ️  $1${NC}"; }
ok()    { echo -e "${GREEN}  ✅ $1${NC}"; ((PASS++)); }
fail()  { echo -e "${RED}  ❌ $1${NC}"; ((FAIL++)); }
skip()  { echo -e "${YELLOW}  ⏭  $1 (skipped)${NC}"; ((SKIP++)); }
header(){ echo -e "\n${CYAN}━━━ $1 ━━━${NC}"; }

echo ""
echo "╔══════════════════════════════════════════════════╗"
echo "║   🧪 Mimir v0.30.0 — Post-Deploy Smoke Test    ║"
echo "║   Target: $API"
echo "╚══════════════════════════════════════════════════╝"

# ─── Helper ──────────────────────────────────────────────────
check_status() {
    local desc="$1"
    local method="$2"
    local url="$3"
    local expected_status="${4:-200}"
    local body="${5:-}"

    if [ "$method" = "POST" ] && [ -n "$body" ]; then
        resp=$(curl -s -o /tmp/mimir_test_body -w "%{http_code}" \
            -X POST "$url" \
            -H "Content-Type: application/json" \
            -H "X-Tenant-ID: default_tenant" \
            -d "$body" 2>/dev/null || echo "000")
    elif [ "$method" = "POST" ]; then
        resp=$(curl -s -o /tmp/mimir_test_body -w "%{http_code}" \
            -X POST "$url" \
            -H "X-Tenant-ID: default_tenant" 2>/dev/null || echo "000")
    else
        resp=$(curl -s -o /tmp/mimir_test_body -w "%{http_code}" \
            -H "X-Tenant-ID: default_tenant" \
            "$url" 2>/dev/null || echo "000")
    fi

    if [ "$resp" = "$expected_status" ]; then
        ok "$desc (HTTP $resp)"
    elif [ "$resp" = "000" ]; then
        fail "$desc (connection refused)"
    else
        fail "$desc (expected $expected_status, got $resp)"
        # Show error body for debugging
        head -c 200 /tmp/mimir_test_body 2>/dev/null | sed 's/^/       /' || true
        echo ""
    fi
}

check_json_field() {
    local desc="$1"
    local field="$2"
    local file="/tmp/mimir_test_body"

    if command -v jq >/dev/null 2>&1; then
        val=$(jq -r "$field" "$file" 2>/dev/null || echo "null")
        if [ "$val" != "null" ] && [ -n "$val" ]; then
            ok "$desc = $val"
        else
            fail "$desc (field '$field' missing or null)"
        fi
    else
        skip "$desc (jq not installed)"
    fi
}

# ╔══════════════════════════════════════════════════════════════╗
# ║                      TEST SUITES                            ║
# ╚══════════════════════════════════════════════════════════════╝

# ── 1. Health & Basic ────────────────────────────────────────
header "1. Health & Connectivity"

check_status "Health check" GET "$API/health"

# ── 2. Search Engine ─────────────────────────────────────────
header "2. Ensemble Search Engine"

check_status "POST /api/search (basic)" POST "$API/api/search" "200" \
    '{"query":"test search","limit":3}'

check_json_field "Search returns results array" ".results"
check_json_field "Search returns weights_used" ".weights_used"
check_json_field "Search returns mode_used" ".mode_used"

# Cross-Encoder rerank request (may fail gracefully if Heimdall TEI not available)
check_status "POST /api/search (cross-encoder)" POST "$API/api/search" "200" \
    '{"query":"test rerank","limit":3,"rerank":{"enabled":true,"strategy":"cross-encoder","final_top_k":3}}'

# With filters
check_status "POST /api/search (with filters)" POST "$API/api/search" "200" \
    '{"query":"filtered search","limit":5,"filters":{"source_types":["file"]}}'

# ── 3. Agent CRUD ────────────────────────────────────────────
header "3. Agent CRUD"

check_status "GET /api/v1/agents" GET "$API/api/v1/agents"
check_json_field "Agents list is array" ".agents"

# Create agent with RAG params
check_status "POST /api/v1/agents (create with RAG params)" POST "$API/api/v1/agents" "201" \
    '{"name":"smoke-test-agent-'$RANDOM'","system_prompt":"You are a test agent.","model_id":"llama3","use_rag":true,"use_knowledge_graph":true,"use_pageindex":true,"rag_params":{"weights":{"vector":0.5,"tree":0.3,"graph":0.2},"advanced":{"top_k_per_source":5}},"rerank_config":{"enabled":false,"strategy":"rrf","final_top_k":5}}'

# ── 4. RAG Evaluation Framework ─────────────────────────────
header "4. RAG Evaluation Framework"

check_status "GET /api/v1/rag-eval/runs (list)" GET "$API/api/v1/rag-eval/runs"
check_json_field "Eval runs list" ".runs"

# Run a mini evaluation (Async)
check_status "POST /api/v1/rag-eval/run (mini eval)" POST "$API/api/v1/rag-eval/run" "202" \
    '{"name":"smoke-test-eval","eval_set":[{"query":"test question","expected_titles":["nonexistent"]}],"params":{"weights":{"vector":0.5,"tree":0.3,"graph":0.2},"top_k":5,"vector_alpha":0.7,"vector_threshold":0.3,"graph_hops":2},"evaluate_generation":false}'

check_json_field "Eval run_id returned" ".run_id"

if command -v jq >/dev/null 2>&1; then
    RUN_ID=$(jq -r '.run_id' /tmp/mimir_test_body 2>/dev/null)
    if [ -n "$RUN_ID" ] && [ "$RUN_ID" != "null" ]; then
        # Wait for async background process (smoke test has only 1 query so it should be fast)
        sleep 2
        
        # Check the run details
        check_status "GET /api/v1/rag-eval/runs/$RUN_ID (poll)" GET "$API/api/v1/rag-eval/runs/$RUN_ID" "200"
        
        check_json_field "Eval hit_rate returned" ".run.scores.hit_rate"
        check_json_field "Eval mrr returned" ".run.scores.mrr"
        check_json_field "Eval ndcg returned" ".run.scores.ndcg"
    fi
fi

# ── 5. Auto-Tuner ───────────────────────────────────────────
header "5. Auto-Tuner (Background Job)"

check_status "POST /api/v1/rag-eval/auto-tune (start)" POST "$API/api/v1/rag-eval/auto-tune" "200" \
    '{"eval_set":[{"query":"smoke test","expected_titles":["test"]}],"base_params":{"weights":{"vector":0.5,"tree":0.3,"graph":0.2},"top_k":5,"vector_alpha":0.7,"vector_threshold":0.3,"graph_hops":2},"iterations":1,"target_metric":"ndcg"}'

check_json_field "Auto-tune job_id returned" ".job_id"

# Poll job status
if command -v jq >/dev/null 2>&1; then
    JOB_ID=$(jq -r '.job_id' /tmp/mimir_test_body 2>/dev/null)
    if [ -n "$JOB_ID" ] && [ "$JOB_ID" != "null" ]; then
        sleep 2
        check_status "GET /api/v1/rag-eval/auto-tune/$JOB_ID (poll)" GET "$API/api/v1/rag-eval/auto-tune/$JOB_ID"
        check_json_field "Auto-tune status" ".status"
    fi
fi

# ── 6. Evaluation Extensions ────────────────────────────────
header "6. Evaluation Extensions"

check_status "GET /api/v1/evaluations/results" GET "$API/api/v1/evaluations/results"
check_status "GET /api/v1/evaluations/extraction-summary" GET "$API/api/v1/evaluations/extraction-summary"
check_status "GET /api/v1/evaluations/retrieval-summary" GET "$API/api/v1/evaluations/retrieval-summary"
check_status "GET /api/v1/evaluations/pipeline-scorecard" GET "$API/api/v1/evaluations/pipeline-scorecard"

# ── 7. Sources & Data ───────────────────────────────────────
header "7. Data Sources"

check_status "GET /api/v1/sources" GET "$API/api/v1/sources"

# ── 8. Models ────────────────────────────────────────────────
header "8. LLM Models"

check_status "GET /api/v1/models" GET "$API/api/v1/models"

# ╔══════════════════════════════════════════════════════════════╗
# ║                      RESULTS                                ║
# ╚══════════════════════════════════════════════════════════════╝

echo ""
echo "╔══════════════════════════════════════════════════╗"
echo "║   📊 Test Results                               ║"
echo "╠══════════════════════════════════════════════════╣"
printf "║   ✅ Passed:  %-35s║\n" "$PASS"
printf "║   ❌ Failed:  %-35s║\n" "$FAIL"
printf "║   ⏭  Skipped: %-35s║\n" "$SKIP"
echo "╚══════════════════════════════════════════════════╝"
echo ""

if [ "$FAIL" -gt 0 ]; then
    echo -e "${RED}⚠️  Some tests failed. Check output above for details.${NC}"
    exit 1
else
    echo -e "${GREEN}🎉 All tests passed! Mimir v0.30.0 is healthy.${NC}"
    exit 0
fi

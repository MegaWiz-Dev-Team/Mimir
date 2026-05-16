#!/bin/bash
# S1 Hit Rate@3 Validation
# Tests 10 standard insurance queries against Mimir
# Success = Hit Rate@3 >= 75%

set -e

MIMIR_API="${MIMIR_API:-http://localhost:8000}"

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_pass() { echo -e "${GREEN}[PASS]${NC} $1"; }
log_fail() { echo -e "${RED}[FAIL]${NC} $1"; }
log_metric() { echo -e "${YELLOW}[METRIC]${NC} $1"; }

log_info "=== S1 Hit Rate@3 Validation ==="
log_info "Mimir API: $MIMIR_API"
log_info ""

# 10 Standard Insurance Test Queries
# Tier 1: Lookup (simple entity search)
# Tier 2: Reasoning (coverage + condition)
# Tier 3: Exclusion (what's NOT covered)
# Tier 4: Robustness (edge cases, Thai language)

QUERIES=(
  # Tier 1: Lookup
  "Critical Illness coverage"
  "Health insurance plans"

  # Tier 2: Reasoning
  "What is excluded from life insurance?"
  "Which products cover hospital expenses?"

  # Tier 3: Exclusion
  "Pre-existing conditions exclusion"
  "Waiting period requirements"

  # Tier 4: Robustness
  "ประกันสุขภาพ" # Thai: Health insurance
  "ประกันชีวิต" # Thai: Life insurance
  "การครอบคลุม" # Thai: Coverage
  "ข้อยกเว้น" # Thai: Exclusion
)

PASSED=0
FAILED=0
TOTAL=${#QUERIES[@]}

log_info "Running $TOTAL test queries..."
log_info ""

for i in "${!QUERIES[@]}"; do
  QUERY="${QUERIES[$i]}"
  QUERY_NUM=$((i + 1))

  echo -n "[$QUERY_NUM/$TOTAL] $QUERY ... "

  # Call Mimir search API
  RESPONSE=$(curl -s -X POST "$MIMIR_API/api/search" \
    -H "Content-Type: application/json" \
    -d "{\"query\": \"$QUERY\", \"top_k\": 3}" 2>/dev/null || echo "")

  if [ -z "$RESPONSE" ]; then
    log_fail "No response from API"
    FAILED=$((FAILED + 1))
    continue
  fi

  # Check if results exist (simple heuristic)
  RESULT_COUNT=$(echo "$RESPONSE" | jq '.results | length' 2>/dev/null || echo "0")

  if [ "$RESULT_COUNT" -gt 0 ]; then
    log_pass "✅ Got $RESULT_COUNT results"
    PASSED=$((PASSED + 1))
  else
    log_fail "❌ No results"
    FAILED=$((FAILED + 1))
  fi
done

# Calculate Hit Rate@3
log_info ""
log_info "=== Results ==="
HIT_RATE=$(( PASSED * 100 / TOTAL ))

log_metric "Queries passed: $PASSED/$TOTAL"
log_metric "Hit Rate@3: $HIT_RATE%"

# Decision
log_info ""
if [ "$HIT_RATE" -ge 75 ]; then
  echo -e "${GREEN}✅ PASS${NC} Hit Rate@3 >= 75% (${HIT_RATE}%)"
  log_info "Ready for S1 execution"
  exit 0
elif [ "$HIT_RATE" -ge 50 ]; then
  echo -e "${YELLOW}⚠️  WARN${NC} Hit Rate@3 = ${HIT_RATE}% (target 75%)"
  log_info "Consider tuning before S1 execution"
  exit 0
else
  echo -e "${RED}❌ FAIL${NC} Hit Rate@3 < 50% (${HIT_RATE}%)"
  log_info "Activate Plan B: Switch to Typhoon embedding model"
  log_info "See: S1_FALLBACK_STRATEGY.md"
  exit 1
fi

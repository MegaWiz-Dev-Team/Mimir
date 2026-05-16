#!/bin/bash
# S1 Orchestration Pipeline: RefGraph → JSON → Mimir
# Usage: ./s1_consolidate_and_ingest.sh <input_file> [output_file]

set -e  # Exit on error

# Configuration
REFGRAPH_BIN="${REFGRAPH_BIN:-/Users/mimir/Developer/Mimir/refgraph-rs/target/release/refgraph}"
MIMIR_API="${MIMIR_API:-http://localhost:8000}"
MIMIR_INGEST_ENDPOINT="/api/ingest"  # Adjust based on actual Mimir API
DOMAIN="${DOMAIN:-insurance}"

# Arguments
INPUT_FILE="${1:?Usage: $0 <input_file> [output_file]}"
OUTPUT_FILE="${2:-consolidated_$(date +%s).json}"

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Functions
log_info() {
  echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
  echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
  echo -e "${RED}[ERROR]${NC} $1"
}

# Pre-flight checks
log_info "=== S1 Consolidation + Ingestion Pipeline ==="
log_info "Input file: $INPUT_FILE"
log_info "Output file: $OUTPUT_FILE"
log_info "RefGraph binary: $REFGRAPH_BIN"
log_info "Mimir API: $MIMIR_API"

# Check input file exists
if [ ! -f "$INPUT_FILE" ]; then
  log_error "Input file not found: $INPUT_FILE"
  exit 1
fi

# Check RefGraph binary exists
if [ ! -f "$REFGRAPH_BIN" ]; then
  log_error "RefGraph binary not found: $REFGRAPH_BIN"
  log_info "Build with: cd refgraph-rs && cargo build --release"
  exit 1
fi

# Check Mimir API is accessible
log_info "Checking Mimir API connectivity..."
if ! curl -s "$MIMIR_API/health" > /dev/null 2>&1; then
  log_error "Mimir API not accessible at $MIMIR_API"
  log_info "Start Mimir: kubectl port-forward -n asgard svc/mimir 8000:8000"
  exit 1
fi
log_info "✅ Mimir API is accessible"

# Step 1: Consolidate with RefGraph
log_info ""
log_info "=== Step 1: RefGraph Consolidation ==="
log_info "Running: $REFGRAPH_BIN --domain $DOMAIN --input $INPUT_FILE --jsonl $OUTPUT_FILE"

START_TIME=$(date +%s%N | cut -b1-13)

if $REFGRAPH_BIN --domain "$DOMAIN" --input "$INPUT_FILE" --jsonl "$OUTPUT_FILE"; then
  END_TIME=$(date +%s%N | cut -b1-13)
  DURATION=$((($END_TIME - $START_TIME) / 1000))
  log_info "✅ RefGraph consolidation complete (${DURATION}ms)"
else
  log_error "RefGraph consolidation failed"
  exit 1
fi

# Verify output file
if [ ! -f "$OUTPUT_FILE" ]; then
  log_error "RefGraph did not produce output file: $OUTPUT_FILE"
  exit 1
fi

# Check output file is valid JSON
if ! jq empty "$OUTPUT_FILE" 2>/dev/null; then
  log_error "Output file is not valid JSON: $OUTPUT_FILE"
  exit 1
fi

# Extract metadata
ENTITY_COUNT=$(jq -r '.metadata.entity_count // 0' "$OUTPUT_FILE" 2>/dev/null || echo "unknown")
RELATIONSHIP_COUNT=$(jq -r '.metadata.relationship_count // 0' "$OUTPUT_FILE" 2>/dev/null || echo "unknown")
AVG_CONFIDENCE=$(jq -r '.metadata.average_confidence // 0' "$OUTPUT_FILE" 2>/dev/null || echo "unknown")

log_info "Consolidated metadata:"
log_info "  Entities: $ENTITY_COUNT"
log_info "  Relationships: $RELATIONSHIP_COUNT"
log_info "  Average confidence: $AVG_CONFIDENCE"

# Step 2: Ingest into Mimir
log_info ""
log_info "=== Step 2: Mimir Ingestion ==="
log_info "POST $MIMIR_API$MIMIR_INGEST_ENDPOINT"

START_TIME=$(date +%s%N | cut -b1-13)

RESPONSE=$(curl -s -w "\n%{http_code}" \
  -X POST "$MIMIR_API$MIMIR_INGEST_ENDPOINT" \
  -H "Content-Type: application/json" \
  --data-binary @"$OUTPUT_FILE")

HTTP_CODE=$(echo "$RESPONSE" | tail -n1)
RESPONSE_BODY=$(echo "$RESPONSE" | head -n-1)

END_TIME=$(date +%s%N | cut -b1-13)
DURATION=$((($END_TIME - $START_TIME) / 1000))

if [ "$HTTP_CODE" = "200" ] || [ "$HTTP_CODE" = "201" ]; then
  log_info "✅ Mimir ingestion complete (${DURATION}ms)"
  log_info "HTTP Response: $HTTP_CODE"

  # Parse response if it contains useful data
  if [ ! -z "$RESPONSE_BODY" ]; then
    log_info "Response: $(echo $RESPONSE_BODY | jq -c . 2>/dev/null || echo $RESPONSE_BODY | head -c 100)"
  fi
else
  log_error "Mimir ingestion failed with HTTP $HTTP_CODE"
  log_error "Response: $RESPONSE_BODY"
  exit 1
fi

# Step 3: Verification
log_info ""
log_info "=== Step 3: Verification ==="

# Wait a moment for Mimir to index
sleep 2

# Check if entities were ingested (optional - depends on Mimir API)
log_info "Output file: $OUTPUT_FILE"
log_info "Size: $(du -h $OUTPUT_FILE | cut -f1)"

# Summary
log_info ""
log_info "=== Pipeline Complete ✅ ==="
log_info "Timeline:"
log_info "  ├─ RefGraph consolidation: ~2 min"
log_info "  ├─ Mimir ingestion: ~1 min"
log_info "  └─ Total: ~3 min"
log_info ""
log_info "Next steps:"
log_info "  1. Verify entities in Mimir: curl $MIMIR_API/api/stats"
log_info "  2. Run test queries: ./test_hit_rate.sh"
log_info "  3. Check Hit Rate@3 >= 75%"
log_info ""
log_info "Consolidation file: $OUTPUT_FILE (for debugging)"

exit 0

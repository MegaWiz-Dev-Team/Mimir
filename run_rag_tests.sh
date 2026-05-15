#!/bin/bash
##
# RAG Studio Playground Test Runner
# Runs tests for asgard-medical tenant in Node.js or Python
##

set -e

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MIMIR_URL="${MIMIR_URL:-http://localhost:3002}"
TEST_TYPE="${1:-auto}"
TEST_LANG="${2:-auto}"

# Extract language if test type is specified
if [ "$TEST_TYPE" != "auto" ] && [ "$TEST_TYPE" != "rag" ] && [ "$TEST_TYPE" != "agents" ]; then
  TEST_LANG="$TEST_TYPE"
  TEST_TYPE="auto"
fi

echo -e "${BLUE}╭─────────────────────────────────────────╮${NC}"
echo -e "${BLUE}│  Mimir Test Suite Runner                │${NC}"
echo -e "${BLUE}│  Testing: asgard-medical Tenant        │${NC}"
echo -e "${BLUE}╰─────────────────────────────────────────╯${NC}"
echo ""
echo -e "Base URL: ${YELLOW}${MIMIR_URL}${NC}"
echo -e "Script Dir: ${YELLOW}${SCRIPT_DIR}${NC}"
echo ""

# Auto-detect test type if not specified
if [ "$TEST_TYPE" = "auto" ]; then
  TEST_TYPE="rag"
  echo -e "${YELLOW}ℹ No test type specified - using: rag${NC}"
  echo -e "${YELLOW}  Use 'rag' or 'agents' as first argument${NC}"
  echo ""
fi

# Auto-detect language if not specified
if [ "$TEST_LANG" = "auto" ]; then
  if command -v node &> /dev/null; then
    TEST_LANG="node"
    echo -e "${GREEN}✓ Node.js found - using JavaScript tests${NC}"
  elif command -v python3 &> /dev/null; then
    TEST_LANG="python"
    echo -e "${GREEN}✓ Python found - using Python tests${NC}"
  elif command -v python &> /dev/null; then
    TEST_LANG="python"
    echo -e "${GREEN}✓ Python found - using Python tests${NC}"
  else
    echo -e "${RED}✗ No suitable runtime found. Please install Node.js (18+) or Python (3.8+)${NC}"
    exit 1
  fi
  echo ""
fi

# Determine test script
case "$TEST_TYPE" in
  rag)
    TEST_PREFIX="rag_playground"
    ;;
  agents)
    TEST_PREFIX="agents_api"
    ;;
  e2e)
    TEST_PREFIX="e2e_medical_workflow"
    ;;
  *)
    echo -e "${RED}✗ Unknown test type: $TEST_TYPE${NC}"
    echo -e "${YELLOW}Usage: $0 [rag|agents|e2e] [node|python|auto]${NC}"
    exit 1
    ;;
esac

# Run tests
case "$TEST_LANG" in
  node|js|javascript)
    if ! command -v node &> /dev/null; then
      echo -e "${RED}✗ Node.js is not installed${NC}"
      exit 1
    fi
    TEST_FILE="$SCRIPT_DIR/test_${TEST_PREFIX}_medical.js"
    if [ ! -f "$TEST_FILE" ]; then
      echo -e "${RED}✗ Test file not found: $TEST_FILE${NC}"
      exit 1
    fi
    echo -e "${BLUE}Running JavaScript tests ($TEST_TYPE)...${NC}"
    echo ""
    MIMIR_URL="$MIMIR_URL" node "$TEST_FILE"
    ;;

  python|py)
    if ! command -v python3 &> /dev/null && ! command -v python &> /dev/null; then
      echo -e "${RED}✗ Python is not installed${NC}"
      exit 1
    fi

    PYTHON_CMD="python3"
    if ! command -v python3 &> /dev/null; then
      PYTHON_CMD="python"
    fi

    # Check for required requests library
    if ! $PYTHON_CMD -c "import requests" 2>/dev/null; then
      echo -e "${YELLOW}⚠ Python 'requests' library not found${NC}"
      echo -e "${YELLOW}  Installing: pip install requests${NC}"
      pip install requests || pip3 install requests
      echo ""
    fi

    TEST_FILE="$SCRIPT_DIR/test_${TEST_PREFIX}_medical.py"
    if [ ! -f "$TEST_FILE" ]; then
      echo -e "${RED}✗ Test file not found: $TEST_FILE${NC}"
      exit 1
    fi

    echo -e "${BLUE}Running Python tests ($TEST_TYPE)...${NC}"
    echo ""
    MIMIR_URL="$MIMIR_URL" $PYTHON_CMD "$TEST_FILE"
    ;;

  *)
    echo -e "${RED}✗ Unknown test language: $TEST_LANG${NC}"
    echo -e "${YELLOW}Usage: $0 [rag|agents|e2e] [node|python|auto]${NC}"
    echo ""
    echo -e "${BLUE}Examples:${NC}"
    echo "  $0                    # Auto-detect and run RAG tests"
    echo "  $0 rag                # Run RAG Playground tests"
    echo "  $0 agents             # Run Agent Studio API tests"
    echo "  $0 e2e                # Run End-to-End integration tests"
    echo "  $0 rag node           # Run RAG tests with Node.js"
    echo "  $0 agents python      # Run agents tests with Python"
    echo "  $0 e2e                # Run E2E tests (auto-detect runtime)"
    echo ""
    exit 1
    ;;
esac

# Check exit code
if [ $? -eq 0 ]; then
  echo ""
  echo -e "${GREEN}✓ Tests completed successfully${NC}"
  exit 0
else
  echo ""
  echo -e "${RED}✗ Tests failed${NC}"
  exit 1
fi

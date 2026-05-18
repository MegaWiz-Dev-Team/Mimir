#!/usr/bin/env bash
# Bootstrap shared knowledge bases on a fresh Mac mini.
#
# Phase 2 of the customer deployment runbook
# (Asgard/docs/technical/customer-deployment-runbook.md).
#
# Runs the canonical ingest path for every universal reference KB:
#   - ICD-10-TM master table (MariaDB) + Qdrant icd10-th collection
#   - PrimeKG Neo4j graph (129K nodes / 8.1M edges) + Qdrant primekg-entities
#   - LOINC schema (data download is manual; see W2.3a runbook)
#
# Idempotent — safe to re-run after partial failure. Detects already-loaded
# state and skips. Uses checkpoint files in /tmp.
#
# Per `feedback_no_ollama`: every embedding call routes through Heimdall.
# Per ADR-009: single-tenant per Mac mini — these KBs are tenant_id=NULL.
#
# Pre-req (Phase 1 of deployment runbook):
#   - ./scripts/deploy-all.sh has run (K3s + Heimdall up)
#   - kubectl port-forward to mariadb:33306, qdrant:6333, neo4j:7687 (this
#     script will start them if missing)
#
# Usage:
#   ./scripts/bootstrap-shared-kbs.sh                     # full run, all KBs
#   ./scripts/bootstrap-shared-kbs.sh --skip-icd10        # skip ICD-10 phase
#   ./scripts/bootstrap-shared-kbs.sh --skip-primekg      # skip PrimeKG phase
#   ./scripts/bootstrap-shared-kbs.sh --dry-run           # show plan, no work
#
# Env (sane defaults — override only if non-standard):
#   MARIADB_HOST / MARIADB_PORT / MARIADB_USER / MARIADB_PASS / MARIADB_DB
#   HEIMDALL_API_URL / HEIMDALL_API_KEY
#   QDRANT_URL
#   NEO4J_NAMESPACE / NEO4J_USER / NEO4J_PASS
#   MIMIR_API_URL / MIMIR_JWT

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ── Flags ──────────────────────────────────────────────────────────────────
SKIP_ICD10=false
SKIP_PRIMEKG=false
SKIP_LOINC_SCHEMA=false
DRY_RUN=false
PRIMEKG_CSV=""
ICD10_PDF=""

for arg in "$@"; do
  case "$arg" in
    --skip-icd10)         SKIP_ICD10=true ;;
    --skip-primekg)       SKIP_PRIMEKG=true ;;
    --skip-loinc-schema)  SKIP_LOINC_SCHEMA=true ;;
    --dry-run)            DRY_RUN=true ;;
    --primekg-csv=*)      PRIMEKG_CSV="${arg#*=}" ;;
    --icd10-pdf=*)        ICD10_PDF="${arg#*=}" ;;
    -h|--help)
      sed -n '2,30p' "$0"; exit 0 ;;
    *) echo "Unknown arg: $arg" >&2; exit 1 ;;
  esac
done

# ── Defaults ───────────────────────────────────────────────────────────────
MARIADB_HOST="${MARIADB_HOST:-127.0.0.1}"
MARIADB_PORT="${MARIADB_PORT:-33306}"
MARIADB_USER="${MARIADB_USER:-root}"
MARIADB_PASS="${MARIADB_PASS:-root}"
MARIADB_DB="${MARIADB_DB:-mimir}"
HEIMDALL_API_URL="${HEIMDALL_API_URL:-http://localhost:8080/v1}"
HEIMDALL_API_KEY="${HEIMDALL_API_KEY:-}"
QDRANT_URL="${QDRANT_URL:-http://localhost:6333}"
NEO4J_NAMESPACE="${NEO4J_NAMESPACE:-asgard-infra}"
NEO4J_USER="${NEO4J_USER:-neo4j}"
NEO4J_PASS="${NEO4J_PASS:-}"
MIMIR_API_URL="${MIMIR_API_URL:-http://localhost:8090}"
MIMIR_JWT="${MIMIR_JWT:-}"

# ── Helpers ────────────────────────────────────────────────────────────────
ts() { date +%H:%M:%S; }
log()  { printf '[%s] %s\n' "$(ts)" "$*"; }
warn() { printf '[%s] WARN: %s\n' "$(ts)" "$*" >&2; }
die()  { printf '[%s] ERR: %s\n' "$(ts)" "$*" >&2; exit 1; }

run_or_dry() {
  if $DRY_RUN; then
    printf '  [dry-run] %s\n' "$*"
  else
    "$@"
  fi
}

have_mysql_cli() { command -v mysql >/dev/null 2>&1; }

mariadb_query() {
  if have_mysql_cli; then
    mysql -h "$MARIADB_HOST" -P "$MARIADB_PORT" -u "$MARIADB_USER" \
          -p"$MARIADB_PASS" "$MARIADB_DB" -B -N -e "$1"
  else
    # Fallback: exec into the mariadb pod (mariadb client always present there)
    kubectl -n "$NEO4J_NAMESPACE" exec deploy/mariadb -- \
      mariadb -u "$MARIADB_USER" -p"$MARIADB_PASS" "$MARIADB_DB" -B -N -e "$1"
  fi
}

mariadb_file() {
  if have_mysql_cli; then
    mysql -h "$MARIADB_HOST" -P "$MARIADB_PORT" -u "$MARIADB_USER" \
          -p"$MARIADB_PASS" "$MARIADB_DB" < "$1"
  else
    kubectl -n "$NEO4J_NAMESPACE" exec -i deploy/mariadb -- \
      mariadb -u "$MARIADB_USER" -p"$MARIADB_PASS" "$MARIADB_DB" < "$1"
  fi
}

ensure_port_forward() {
  local svc="$1" ns="$2" port="$3"
  if nc -z "$MARIADB_HOST" "$port" 2>/dev/null; then
    return 0
  fi
  log "Starting port-forward $svc:$port → $ns/$svc"
  if $DRY_RUN; then return 0; fi
  kubectl port-forward -n "$ns" "svc/$svc" "$port:$port" \
    > "/tmp/bootstrap-pf-$svc.log" 2>&1 &
  sleep 2
  nc -z "$MARIADB_HOST" "$port" 2>/dev/null \
    || die "port-forward $svc:$port failed; see /tmp/bootstrap-pf-$svc.log"
}

# ── Phase 0 — env + port-forwards + Heimdall + Mimir ──────────────────────
log "=== Bootstrap shared knowledge bases ==="
log "MariaDB: $MARIADB_HOST:$MARIADB_PORT $MARIADB_USER@$MARIADB_DB"
log "Qdrant:  $QDRANT_URL"
log "Heimdall:$HEIMDALL_API_URL"
log "Neo4j:   $NEO4J_NAMESPACE/$NEO4J_USER"
log "Mimir:   $MIMIR_API_URL"
$DRY_RUN && log "(dry-run mode — no work performed)"

ensure_port_forward "mariadb" "$NEO4J_NAMESPACE" "$MARIADB_PORT"
ensure_port_forward "qdrant"  "$NEO4J_NAMESPACE" "${QDRANT_URL##*:}"
ensure_port_forward "neo4j"   "$NEO4J_NAMESPACE" "7687"

if [[ -z "$HEIMDALL_API_KEY" ]]; then
  warn "HEIMDALL_API_KEY not set — embeddings will fail. Source from launchd plist."
fi

if [[ -z "$NEO4J_PASS" ]]; then
  log "Reading NEO4J_PASS from k8s secret $NEO4J_NAMESPACE/neo4j-secret..."
  NEO4J_PASS=$(kubectl -n "$NEO4J_NAMESPACE" get secret neo4j-secret \
    -o jsonpath='{.data.NEO4J_PASSWORD}' 2>/dev/null | base64 -d || true)
  [[ -n "$NEO4J_PASS" ]] || warn "NEO4J_PASS empty; PrimeKG import will fail."
fi

# ── Phase 1 — Apply migrations ─────────────────────────────────────────────
log ""
log "=== Phase 1: MariaDB schema (icd10 + loinc masters) ==="

if [[ "$(mariadb_query "SHOW TABLES LIKE 'icd10_codes'" 2>/dev/null)" == "icd10_codes" ]]; then
  log "  icd10_codes already exists — skipping migration."
else
  log "  Applying sprint48_icd10_codes.sql..."
  run_or_dry mariadb_file "$REPO_ROOT/ro-ai-bridge/migrations/sprint48_icd10_codes.sql"
fi

if $SKIP_LOINC_SCHEMA; then
  log "  loinc schema skipped (--skip-loinc-schema)."
elif [[ "$(mariadb_query "SHOW TABLES LIKE 'loinc_codes'" 2>/dev/null)" == "loinc_codes" ]]; then
  log "  loinc_codes already exists — skipping migration."
else
  log "  Applying sprint49_loinc_codes.sql..."
  run_or_dry mariadb_file "$REPO_ROOT/ro-ai-bridge/migrations/sprint49_loinc_codes.sql"
fi

# ── Phase 2 — ICD-10-TM ingest ─────────────────────────────────────────────
log ""
log "=== Phase 2: ICD-10-TM (Thai master, 15,376 codes) ==="
if $SKIP_ICD10; then
  log "  skipped (--skip-icd10)."
else
  ICD10_COUNT=$(mariadb_query "SELECT COUNT(*) FROM icd10_codes WHERE tenant_id IS NULL" 2>/dev/null || echo 0)
  if [[ "$ICD10_COUNT" -ge 15000 ]]; then
    log "  $ICD10_COUNT rows already present in icd10_codes — skipping ingest."
  else
    if [[ -z "$ICD10_PDF" ]]; then
      ICD10_PDF=/tmp/icd10tm_anamai.pdf
      if [[ ! -f "$ICD10_PDF" ]]; then
        log "  Downloading anamai ICD-10-TM PDF (free, public)..."
        run_or_dry curl -fsSL -o "$ICD10_PDF" \
          "https://backenddc.anamai.moph.go.th/coverpage/d1579eb1c80b878ab62513c060681290.pdf"
      else
        log "  Re-using cached PDF at $ICD10_PDF"
      fi
    fi
    log "  Parsing + ingesting via icd10_tm_anamai_ingest.py..."
    run_or_dry python3 "$SCRIPT_DIR/icd10_tm_anamai_ingest.py" \
      --pdf "$ICD10_PDF" \
      --source-version anamai-moph-2010
  fi

  # Qdrant icd10-th collection
  ICD10_QDRANT_COUNT=$(curl -fsS "$QDRANT_URL/collections/icd10-th" 2>/dev/null \
    | python3 -c 'import sys,json; d=json.load(sys.stdin); print(d.get("result",{}).get("points_count",0))' 2>/dev/null || echo 0)
  if [[ "$ICD10_QDRANT_COUNT" -ge 15000 ]]; then
    log "  icd10-th Qdrant collection has $ICD10_QDRANT_COUNT points — skipping embed."
  else
    log "  Embedding ICD-10 rows → Qdrant icd10-th via Heimdall BGE-M3..."
    run_or_dry env \
      MARIADB_HOST="$MARIADB_HOST" MARIADB_PORT="$MARIADB_PORT" \
      MARIADB_USER="$MARIADB_USER" MARIADB_PASS="$MARIADB_PASS" \
      MARIADB_DB="$MARIADB_DB" \
      HEIMDALL_API_URL="$HEIMDALL_API_URL" HEIMDALL_API_KEY="$HEIMDALL_API_KEY" \
      QDRANT_URL="$QDRANT_URL" \
      python3 "$SCRIPT_DIR/icd10_embed_qdrant.py" --batch 64 --workers 4
  fi
fi

# ── Phase 3 — PrimeKG ──────────────────────────────────────────────────────
log ""
log "=== Phase 3: PrimeKG (Harvard biomedical KG, 129K nodes + 8.1M edges) ==="
if $SKIP_PRIMEKG; then
  log "  skipped (--skip-primekg)."
else
  if [[ -z "$PRIMEKG_CSV" ]]; then
    PRIMEKG_CSV="$REPO_ROOT/data/PrimeKG/kg.csv"
  fi
  if [[ ! -f "$PRIMEKG_CSV" ]]; then
    die "PrimeKG kg.csv not found at $PRIMEKG_CSV. Download manually from
         https://dataverse.harvard.edu/dataset.xhtml?persistentId=doi:10.7910/DVN/IXA7BM
         and re-run with --primekg-csv=/path/to/kg.csv"
  fi

  # Neo4j node count
  NEO4J_NODES=$(kubectl -n "$NEO4J_NAMESPACE" exec deploy/neo4j -- \
    cypher-shell -u "$NEO4J_USER" -p "$NEO4J_PASS" --format plain \
    "MATCH (n:PrimeKG) RETURN count(n);" 2>/dev/null | tail -1 || echo 0)
  if [[ "$NEO4J_NODES" -ge 120000 ]]; then
    log "  Neo4j has $NEO4J_NODES :PrimeKG nodes — skipping import."
  else
    log "  Importing PrimeKG via primekg_import.sh (idempotent, ~6-30 min)..."
    run_or_dry env NEO4J_PASS="$NEO4J_PASS" \
      bash "$SCRIPT_DIR/primekg_import.sh" "$PRIMEKG_CSV"
  fi

  # Qdrant primekg-entities
  PKG_QDRANT_COUNT=$(curl -fsS "$QDRANT_URL/collections/primekg-entities" 2>/dev/null \
    | python3 -c 'import sys,json; d=json.load(sys.stdin); print(d.get("result",{}).get("points_count",0))' 2>/dev/null || echo 0)
  if [[ "$PKG_QDRANT_COUNT" -ge 120000 ]]; then
    log "  primekg-entities Qdrant collection has $PKG_QDRANT_COUNT points — skipping embed."
  else
    log "  Triggering Mimir admin embed (POST /api/v1/admin/knowledge/primekg/embed)..."
    if [[ -z "$MIMIR_JWT" ]]; then
      warn "MIMIR_JWT not set — admin endpoint requires HS256 token with iss=mimir-auth."
      warn "Skipping PrimeKG Qdrant embed. Run manually after generating JWT."
    else
      run_or_dry curl -fsS -X POST "$MIMIR_API_URL/api/v1/admin/knowledge/primekg/embed" \
        -H "Authorization: Bearer $MIMIR_JWT" \
        -H "Content-Type: application/json" \
        -d '{"batch_size":500,"dry_run":false}'
      log "  Embed running in background — poll: GET $MIMIR_API_URL/api/v1/admin/knowledge/primekg/embed/status"
    fi
  fi
fi

# ── Phase 4 — Verify via shared knowledge catalog ─────────────────────────
log ""
log "=== Phase 4: Verify via /api/v1/knowledge/shared ==="
if [[ -z "$MIMIR_JWT" ]]; then
  warn "MIMIR_JWT not set; skipping catalog verify."
else
  run_or_dry curl -fsS "$MIMIR_API_URL/api/v1/knowledge/shared" \
    -H "Authorization: Bearer $MIMIR_JWT" \
    | python3 -c 'import sys, json
d = json.load(sys.stdin)
for kb in d.get("items", []):
    cs = ", ".join("{}={}".format(k, v) for k, v in kb["counts"].items())
    print("  [{:<13}] {:<42} {}".format(kb["status"], kb["name"], cs))
'
fi

log ""
log "=== Bootstrap complete ==="
log "Next: Phase 3 of runbook — tenant + Eir agents recovery"
log "  kubectl exec -n $NEO4J_NAMESPACE deploy/mariadb -- mariadb -uroot -p\$PW < scripts/recover-asgard-tenant.sql"

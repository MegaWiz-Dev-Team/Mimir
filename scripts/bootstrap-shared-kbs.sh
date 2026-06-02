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
#   ./scripts/bootstrap-shared-kbs.sh --skip-loinc       # skip LOINC phase
#   ./scripts/bootstrap-shared-kbs.sh --skip-tmt         # skip TMT phase
#   ./scripts/bootstrap-shared-kbs.sh --skip-tmlt        # skip TMLT phase
#   ./scripts/bootstrap-shared-kbs.sh --skip-tpc         # skip TPC phase
#   ./scripts/bootstrap-shared-kbs.sh --skip-snomed-refsets  # skip IPS/GPFP/EDQM phase
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
SKIP_LOINC=false
SKIP_TMT=false
SKIP_TMLT=false
SKIP_TPC=false
SKIP_SNOMED_REFSETS=false
DRY_RUN=false
PRIMEKG_CSV=""
ICD10_PDF=""

for arg in "$@"; do
  case "$arg" in
    --skip-icd10)         SKIP_ICD10=true ;;
    --skip-primekg)       SKIP_PRIMEKG=true ;;
    --skip-loinc)         SKIP_LOINC=true ;;
    --skip-tmt)           SKIP_TMT=true ;;
    --skip-tmlt)          SKIP_TMLT=true ;;
    --skip-tpc)           SKIP_TPC=true ;;
    --skip-snomed-refsets) SKIP_SNOMED_REFSETS=true ;;
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
  # Try asgard-secrets (canonical, key NEO4J_PASSWORD) then fall back to
  # asgard-infra/neo4j-secret (key NEO4J_AUTH = "neo4j/<password>").
  NEO4J_PASS=$(kubectl -n asgard get secret asgard-secrets \
    -o jsonpath='{.data.NEO4J_PASSWORD}' 2>/dev/null | base64 -d || true)
  if [[ -z "$NEO4J_PASS" ]]; then
    AUTH=$(kubectl -n "$NEO4J_NAMESPACE" get secret neo4j-secret \
      -o jsonpath='{.data.NEO4J_AUTH}' 2>/dev/null | base64 -d || true)
    NEO4J_PASS="${AUTH#neo4j/}"
  fi
  if [[ -n "$NEO4J_PASS" ]]; then
    log "NEO4J_PASS resolved from k8s secrets."
  else
    warn "NEO4J_PASS empty; PrimeKG import will fail."
  fi
fi

# ── Phase 1 — Apply migrations ─────────────────────────────────────────────
log ""
log "=== Phase 1: MariaDB schema (icd10 + loinc masters) ==="

apply_migration_if_missing() {
  local sentinel_table="$1"   # e.g. icd10_codes
  local migration_file="$2"   # e.g. sprint48_icd10_codes.sql
  if [[ "$(mariadb_query "SHOW TABLES LIKE '$sentinel_table'" 2>/dev/null)" == "$sentinel_table" ]]; then
    log "  $sentinel_table already exists — skipping migration."
  else
    log "  Applying $(basename "$migration_file")..."
    run_or_dry mariadb_file "$migration_file"
  fi
}

apply_migration_if_missing icd10_codes "$REPO_ROOT/ro-ai-bridge/migrations/sprint48_icd10_codes.sql"
apply_migration_if_missing loinc_codes "$REPO_ROOT/ro-ai-bridge/migrations/sprint49_loinc_codes.sql"
apply_migration_if_missing tmt_codes   "$REPO_ROOT/ro-ai-bridge/migrations/sprint50_tmt_codes.sql"
apply_migration_if_missing tmlt_codes  "$REPO_ROOT/ro-ai-bridge/migrations/sprint51_tmlt_codes.sql"
apply_migration_if_missing tpc_codes   "$REPO_ROOT/ro-ai-bridge/migrations/sprint52_tpc_codes.sql"
apply_migration_if_missing snomed_refset_members "$REPO_ROOT/ro-ai-bridge/migrations/sprint58_snomed_refsets_edqm.sql"

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

# ── Phase 4 — LOINC (auto-detect data/LOINC/Loinc_*) ─────────────────────
log ""
log "=== Phase 4: LOINC (Lab/Observation Codes) ==="
if $SKIP_LOINC; then
  log "  skipped (--skip-loinc)."
else
  LOINC_CSV=$(ls "$REPO_ROOT"/data/LOINC/Loinc_*/LoincTable/Loinc.csv 2>/dev/null | head -1)
  if [[ -z "$LOINC_CSV" ]]; then
    log "  No data/LOINC/Loinc_*/LoincTable/Loinc.csv found — skipping (manual download from loinc.org required)."
  else
    LOINC_COUNT=$(mariadb_query "SELECT COUNT(*) FROM loinc_codes WHERE tenant_id IS NULL" 2>/dev/null || echo 0)
    if [[ "$LOINC_COUNT" -ge 90000 ]]; then
      log "  $LOINC_COUNT rows already present in loinc_codes — skipping ingest."
    else
      # Derive source_version from path: data/LOINC/Loinc_2.82/... → loinc-2.82
      LOINC_VER=$(echo "$LOINC_CSV" | sed -E 's|.*/Loinc_([0-9.]+)/.*|loinc-\1|')
      log "  Ingesting $LOINC_CSV (source_version=$LOINC_VER)..."
      run_or_dry env MARIADB_NAMESPACE="$NEO4J_NAMESPACE" \
        python3 "$SCRIPT_DIR/loinc_ingest.py" \
        --csv "$LOINC_CSV" --source-version "$LOINC_VER"
    fi
  fi
fi

# ── Phase 5 — TMT (auto-detect data/TMT/TMTRF*) ──────────────────────────
log ""
log "=== Phase 5: TMT (Thai Medicines Terminology) ==="
if $SKIP_TMT; then
  log "  skipped (--skip-tmt)."
else
  TMT_DIR=$(ls -d "$REPO_ROOT"/data/TMT/TMTRF[0-9]* 2>/dev/null | head -1)
  if [[ -z "$TMT_DIR" ]]; then
    log "  No data/TMT/TMTRF*/ release found — skipping (download from this.or.th)."
  else
    TMT_COUNT=$(mariadb_query "SELECT COUNT(*) FROM tmt_codes WHERE tenant_id IS NULL" 2>/dev/null || echo 0)
    if [[ "$TMT_COUNT" -ge 100000 ]]; then
      log "  $TMT_COUNT concepts already present in tmt_codes — skipping ingest."
    else
      TMT_VER=$(basename "$TMT_DIR" | sed -E 's/TMTRF([0-9]+)/tmt-\1/')
      log "  Ingesting $TMT_DIR (source_version=$TMT_VER)..."
      run_or_dry env MARIADB_NAMESPACE="$NEO4J_NAMESPACE" \
        /opt/homebrew/bin/python3 "$SCRIPT_DIR/tmt_ingest.py" \
        --release-dir "$TMT_DIR" --source-version "$TMT_VER"
    fi
  fi
fi

# ── Phase 6 — TMLT (auto-detect data/TMLT/TMLTRF*) ───────────────────────
log ""
log "=== Phase 6: TMLT (Thai Medical Laboratory Terminology) ==="
if $SKIP_TMLT; then
  log "  skipped (--skip-tmlt)."
else
  TMLT_DIR=$(ls -d "$REPO_ROOT"/data/TMLT/TMLTRF[0-9]* 2>/dev/null | head -1)
  if [[ -z "$TMLT_DIR" ]]; then
    log "  No data/TMLT/TMLTRF*/ release found — skipping (download from this.or.th)."
  else
    TMLT_COUNT=$(mariadb_query "SELECT COUNT(*) FROM tmlt_codes WHERE tenant_id IS NULL" 2>/dev/null || echo 0)
    if [[ "$TMLT_COUNT" -ge 4000 ]]; then
      log "  $TMLT_COUNT concepts already present in tmlt_codes — skipping ingest."
    else
      TMLT_VER=$(basename "$TMLT_DIR" | sed -E 's/TMLTRF([0-9]+)/tmlt-\1/')
      log "  Ingesting $TMLT_DIR (source_version=$TMLT_VER)..."
      run_or_dry env MARIADB_NAMESPACE="$NEO4J_NAMESPACE" \
        /opt/homebrew/bin/python3 "$SCRIPT_DIR/tmlt_ingest.py" \
        --release-dir "$TMLT_DIR" --source-version "$TMLT_VER"
    fi
  fi
fi

# ── Phase 7 — TPC via ICD-9-CM upstream baseline ─────────────────────────
log ""
log "=== Phase 7: TPC (Procedure codes — ICD-9-CM baseline) ==="
if $SKIP_TPC; then
  log "  skipped (--skip-tpc)."
else
  TPC_PDF="$REPO_ROOT/data/ICD-9-CM/icd9cm.pdf"
  TPC_ERRATA="$REPO_ROOT/data/ICD-9-CM/new-invalid-icd9cm-2561.pdf"
  if [[ ! -f "$TPC_PDF" ]]; then
    log "  No data/ICD-9-CM/icd9cm.pdf found — skipping."
  else
    TPC_COUNT=$(mariadb_query "SELECT COUNT(*) FROM tpc_codes WHERE tenant_id IS NULL" 2>/dev/null || echo 0)
    if [[ "$TPC_COUNT" -ge 3000 ]]; then
      log "  $TPC_COUNT codes already present in tpc_codes — skipping ingest."
    else
      log "  Ingesting $TPC_PDF (+ errata if present)..."
      ERRATA_ARG=""
      [[ -f "$TPC_ERRATA" ]] && ERRATA_ARG="--errata $TPC_ERRATA"
      run_or_dry env MARIADB_NAMESPACE="$NEO4J_NAMESPACE" \
        /opt/homebrew/bin/python3 "$SCRIPT_DIR/icd9cm_ingest.py" \
        --pdf "$TPC_PDF" $ERRATA_ARG --source-version icd9cm-cms-fy15
    fi
  fi
fi

# ── Phase 8 — SNOMED refsets (IPS / GP-FP) + EDQM dose map ───────────────
# Source: SNOMED International MLDS (https://mlds.ihtsdotools.org/), Thailand member.
# Packages auto-detected under data/SnomedCT/ (symlink to controlled storage).
log ""
log "=== Phase 8: SNOMED refsets (IPS, GP/FP, EDQM dose map) ==="
if $SKIP_SNOMED_REFSETS; then
  log "  skipped (--skip-snomed-refsets)."
else
  SCT_DIR="$REPO_ROOT/data/SnomedCT"
  RS_COUNT=$(mariadb_query "SELECT COUNT(*) FROM snomed_refset_members WHERE tenant_id IS NULL" 2>/dev/null || echo 0)
  if [[ "$RS_COUNT" -ge 16000 ]]; then
    log "  $RS_COUNT refset members already present — skipping ingest."
  else
    IPS_F=$(ls "$SCT_DIR"/SnomedCT_IPS_*/Snapshot/Refset/Content/der2_Refset_IPSSimpleSnapshot_*.txt 2>/dev/null | grep -v '/\._' | head -1)
    GPFP_F=$(ls "$SCT_DIR"/SnomedCT_GPFP_*/Snapshot/Refset/Content/der2_Refset_GPFPSimpleSnapshot_*.txt 2>/dev/null | grep -v '/\._' | head -1)
    EDQM_F=$(ls "$SCT_DIR"/SnomedCT_*EDQM*/Snapshot/Refset/Map/der2_ssRefset_EDQMSimpleMapSnapshot_*.txt 2>/dev/null | grep -v '/\._' | head -1)
    if [[ -z "$IPS_F$GPFP_F$EDQM_F" ]]; then
      log "  No IPS/GPFP/EDQM packages under data/SnomedCT/ — skipping (download via MLDS)."
    else
      [[ -n "$IPS_F"  ]] && run_or_dry env MARIADB_NAMESPACE="$NEO4J_NAMESPACE" /opt/homebrew/bin/python3 \
        "$SCRIPT_DIR/snomed_refset_ingest.py" --ips  "$IPS_F"  --source-version sct-ips-20250701  --source-url https://mlds.ihtsdotools.org/
      [[ -n "$GPFP_F" ]] && run_or_dry env MARIADB_NAMESPACE="$NEO4J_NAMESPACE" /opt/homebrew/bin/python3 \
        "$SCRIPT_DIR/snomed_refset_ingest.py" --gpfp "$GPFP_F" --source-version sct-gpfp-20260101 --source-url https://mlds.ihtsdotools.org/
      [[ -n "$EDQM_F" ]] && run_or_dry env MARIADB_NAMESPACE="$NEO4J_NAMESPACE" /opt/homebrew/bin/python3 \
        "$SCRIPT_DIR/snomed_refset_ingest.py" --edqm "$EDQM_F" --source-version sct-edqm-20250701 --source-url https://mlds.ihtsdotools.org/
      # TMT→SNOMED dose link needs tmt_codes + snomed_descriptions present.
      run_or_dry env MARIADB_NAMESPACE="$NEO4J_NAMESPACE" /opt/homebrew/bin/python3 \
        "$SCRIPT_DIR/snomed_refset_ingest.py" --tmt-dose-link --source-version sct-edqm-20250701 --source-url https://mlds.ihtsdotools.org/
    fi
  fi
fi

# ── Phase 9 — Verify via shared knowledge catalog ─────────────────────────
log ""
log "=== Phase 9: Verify via /api/v1/knowledge/shared ==="
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

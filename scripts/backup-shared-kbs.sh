#!/usr/bin/env bash
# Backup shared knowledge bases (MariaDB master tables + Qdrant collections
# + Neo4j PrimeKG snapshot state). Lets us roll back to a known-good state
# before any destructive change (schema migration, re-ingest, etc.).
#
# What gets captured:
#   - MariaDB: mysqldump of icd10/loinc/tmt/tmlt + their *_ingest_runs tables.
#   - Qdrant:  snapshot API call for icd10-th and primekg-entities collections.
#   - Neo4j:   row counts + label distribution as a verification manifest
#              (the source kg.csv is the real backup — it's idempotent).
#   - Manifest with SHA-256 + row counts so we can verify a restore.
#
# Storage layout:
#   <BACKUP_DIR>/<YYYY-MM-DD-HHMM>/
#     mariadb-shared-kbs.sql.gz
#     qdrant-icd10-th.snapshot          (if Qdrant snapshot API succeeds)
#     qdrant-primekg-entities.snapshot
#     neo4j-primekg-manifest.txt
#     MANIFEST.txt                       (sizes + sha256 + row counts)
#
# Usage:
#   ./scripts/backup-shared-kbs.sh
#   BACKUP_DIR=/Volumes/External ./scripts/backup-shared-kbs.sh   # off-box
#
# Default BACKUP_DIR keeps things on-host but outside the git repo:
#   ~/asgard-backups/shared-kbs/

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

BACKUP_DIR="${BACKUP_DIR:-$HOME/asgard-backups/shared-kbs}"
STAMP="$(date +%Y-%m-%d-%H%M)"
OUT="$BACKUP_DIR/$STAMP"
mkdir -p "$OUT"

MARIADB_USER="${MARIADB_USER:-root}"
MARIADB_PASS="${MARIADB_PASS:-root}"
MARIADB_DB="${MARIADB_DB:-mimir}"
MARIADB_NS="${MARIADB_NAMESPACE:-asgard-infra}"
QDRANT_URL="${QDRANT_URL:-http://localhost:6333}"

ts()  { date +%H:%M:%S; }
log() { printf '[%s] %s\n' "$(ts)" "$*"; }

log "Backup target: $OUT"

# ── MariaDB dump ───────────────────────────────────────────────────────────
log ""
log "=== MariaDB: dumping shared KB tables ==="

TABLES=(
  # Shared knowledge masters (the original scope)
  icd10_codes icd10_ingest_runs
  loinc_codes loinc_ingest_runs
  tmt_codes tmt_relationships tmt_ingest_runs
  tmlt_codes tmlt_relationships tmlt_ingest_runs
  tpc_codes tpc_ingest_runs
  # Tenant pipeline config (chunk_size etc. — Sprint 48 C.3)
  tenant_configs
  # Retrieval eval (M1 + future PrimeKG bench runs)
  rag_eval_runs rag_eval_queries
  # OCR layout eval (Syn Phase 1.6 result ingested 2026-05-18)
  ocr_layout_eval_runs ocr_layout_eval_items ocr_layout_region_match
)

# Single-transaction so the dump is point-in-time consistent without
# locking writers. Each table is included by name (avoids per-row guessing).
DUMP="$OUT/mariadb-shared-kbs.sql"
# Exec into the pod — no host mariadb client required.
if kubectl -n "$MARIADB_NS" exec deploy/mariadb -- \
   mariadb-dump --single-transaction --quick \
     -u "$MARIADB_USER" -p"$MARIADB_PASS" "$MARIADB_DB" "${TABLES[@]}" \
   > "$DUMP" 2>"$OUT/mariadb-dump.err"; then
  log "  ✓ dump complete: $(du -h "$DUMP" | cut -f1)"
  gzip "$DUMP"
  log "  ✓ compressed: $(du -h "$DUMP.gz" | cut -f1)"
else
  log "  ⚠ dump failed — see $OUT/mariadb-dump.err"
  cat "$OUT/mariadb-dump.err" | tail -5
fi

# ── Qdrant snapshots ───────────────────────────────────────────────────────
log ""
log "=== Qdrant: triggering collection snapshots ==="

snapshot() {
  local coll="$1"
  local resp
  resp=$(curl -fsS -X POST "$QDRANT_URL/collections/$coll/snapshots" 2>/dev/null) \
    || { log "  ⚠ $coll: snapshot trigger failed (collection missing?)"; return; }
  local snap_name
  snap_name=$(echo "$resp" | python3 -c 'import sys, json; print(json.load(sys.stdin)["result"]["name"])' 2>/dev/null)
  if [[ -z "$snap_name" ]]; then
    log "  ⚠ $coll: snapshot name not parseable"
    return
  fi
  log "  triggered $coll snapshot: $snap_name"
  # Download the snapshot file to local backup dir (Qdrant exposes it via /snapshots/<coll>/<name>)
  curl -fsS -o "$OUT/qdrant-$coll.snapshot" \
    "$QDRANT_URL/collections/$coll/snapshots/$snap_name" \
    && log "  ✓ $coll: downloaded $(du -h "$OUT/qdrant-$coll.snapshot" | cut -f1)" \
    || log "  ⚠ $coll: download failed"
}

snapshot icd10-th
snapshot primekg-entities

# ── Neo4j manifest ─────────────────────────────────────────────────────────
log ""
log "=== Neo4j: capturing PrimeKG manifest ==="
NEO4J_PASS="${NEO4J_PASS:-}"
if [[ -z "$NEO4J_PASS" ]]; then
  NEO4J_PASS=$(kubectl -n "$MARIADB_NS" get secret neo4j-secret \
    -o jsonpath='{.data.NEO4J_PASSWORD}' 2>/dev/null | base64 -d || true)
fi
if [[ -n "$NEO4J_PASS" ]]; then
  kubectl -n "$MARIADB_NS" exec deploy/neo4j -- cypher-shell \
    -u neo4j -p "$NEO4J_PASS" --format plain \
    'MATCH (n:PrimeKG) WITH labels(n) AS L, count(n) AS c
       RETURN [l IN L WHERE l <> "PrimeKG"][0] AS node_type, c
       ORDER BY c DESC;' > "$OUT/neo4j-primekg-manifest.txt" 2>/dev/null \
    && log "  ✓ manifest: $(wc -l < "$OUT/neo4j-primekg-manifest.txt") lines"
  kubectl -n "$MARIADB_NS" exec deploy/neo4j -- cypher-shell \
    -u neo4j -p "$NEO4J_PASS" --format plain \
    'MATCH (n:PrimeKG) RETURN count(n) AS nodes;
     MATCH ()-[r]->(:PrimeKG) RETURN count(r) AS edges;' \
    >> "$OUT/neo4j-primekg-manifest.txt" 2>/dev/null
  log "  Note: Neo4j source-of-truth is data/PrimeKG/kg.csv — re-importable via primekg_import.sh"
else
  log "  ⚠ NEO4J_PASS not available; skipping Neo4j manifest"
fi

# ── Manifest + checksums ───────────────────────────────────────────────────
log ""
log "=== Writing MANIFEST.txt ==="
{
  echo "Backup taken: $(date -Iseconds)"
  echo "Host: $(hostname)"
  echo "Stamp: $STAMP"
  echo ""
  echo "=== Files ==="
  cd "$OUT"
  for f in *; do
    [[ -f "$f" ]] || continue
    sha=$(shasum -a 256 "$f" | awk '{print $1}')
    size=$(du -h "$f" | awk '{print $1}')
    printf '  %-40s %8s  sha256=%s\n' "$f" "$size" "$sha"
  done
} > "$OUT/MANIFEST.txt"

log ""
log "=== Done ==="
log "Backup location: $OUT"
log "Restore quickstart:"
log "  MariaDB: gunzip -c mariadb-shared-kbs.sql.gz | kubectl -n $MARIADB_NS exec -i deploy/mariadb -- mariadb -uroot -p\$PW mimir"
log "  Qdrant : curl -X PUT '$QDRANT_URL/collections/<name>/snapshots/recover?wait=true' -F snapshot=@qdrant-<name>.snapshot"
log "  Neo4j  : re-run scripts/primekg_import.sh (idempotent)"

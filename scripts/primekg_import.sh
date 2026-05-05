#!/usr/bin/env bash
# PrimeKG Import — loads kg.csv into Neo4j using APOC LOAD CSV
#
# Usage:
#   ./scripts/primekg_import.sh /path/to/kg.csv
#
# Checkpoint file: ./primekg_import_checkpoint.json
# All phases are idempotent (MERGE) — safe to re-run after failure.
#
# kg.csv column order (actual PrimeKG v2):
#   relation, display_relation, x_index, x_id, x_type, x_name, x_source,
#   y_index, y_id, y_type, y_name, y_source
#
# Node types in CSV: anatomy, biological_process, cellular_component, disease,
#   drug, effect/phenotype, exposure, gene/protein, molecular_function, pathway
# Relation types (8.1M total): indication, contraindication, drug_drug,
#   disease_protein, drug_protein, protein_protein, anatomy_protein_present, ...

set -euo pipefail

NAMESPACE="asgard-infra"
NEO4J_USER="neo4j"
NEO4J_PASS="asgard_neo4j_password"
CHECKPOINT="./primekg_import_checkpoint.json"
KG_CSV="${1:-}"

# ── helpers ───────────────────────────────────────────────────────────────
log() { echo "[$(date '+%H:%M:%S')] $*"; }

neo4j_pod() {
  kubectl get pods -n "$NAMESPACE" -l app=neo4j \
    -o jsonpath='{.items[0].metadata.name}'
}

cypher_run() {
  local pod="$1"; shift
  local cypher_file="$1"
  kubectl exec -n "$NAMESPACE" "$pod" -- \
    cypher-shell -u "$NEO4J_USER" -p "$NEO4J_PASS" \
    --format plain -f "/tmp/$(basename "$cypher_file")"
}

checkpoint_get() { python3 -c "import json,sys; d=json.load(open('$CHECKPOINT')) if __import__('os').path.exists('$CHECKPOINT') else {}; print(d.get('$1',''))" 2>/dev/null || echo ""; }
checkpoint_set() { python3 -c "
import json, os
p = '$CHECKPOINT'
d = json.load(open(p)) if os.path.exists(p) else {}
d['$1'] = '$2'
json.dump(d, open(p,'w'), indent=2)
print('checkpoint: $1=$2')
"; }

# ── Step 0: Validate input ────────────────────────────────────────────────
if [[ -z "$KG_CSV" ]]; then
  echo "Usage: $0 /path/to/kg.csv"
  echo "Download kg.csv (~936MB) from: https://dataverse.harvard.edu/dataset.xhtml?persistentId=doi:10.7910/DVN/IXA7BM"
  exit 1
fi
[[ -f "$KG_CSV" ]] || { echo "ERROR: $KG_CSV not found"; exit 1; }

# Sanity check: must be > 100MB (error pages are tiny)
SIZE_BYTES=$(wc -c < "$KG_CSV")
if [[ "$SIZE_BYTES" -lt 104857600 ]]; then
  echo "ERROR: $KG_CSV is only ${SIZE_BYTES} bytes — expected ~936MB"
  exit 1
fi
log "kg.csv verified: $(du -h "$KG_CSV" | cut -f1)"

# ── Step 1: Find Neo4j pod ────────────────────────────────────────────────
POD=$(neo4j_pod)
log "Using Neo4j pod: $POD"

# ── Step 2: Copy CSV to Neo4j import directory ────────────────────────────
if [[ "$(checkpoint_get csv_copied)" != "done" ]]; then
  log "Copying kg.csv to Neo4j pod import directory (~$(du -h "$KG_CSV" | cut -f1))..."
  kubectl cp "$KG_CSV" "$NAMESPACE/$POD:/var/lib/neo4j/import/kg.csv"
  checkpoint_set csv_copied done
  log "CSV copied."
else
  log "Skipping CSV copy (already done)."
fi

# ── Step 3: Create PrimeKG indexes ───────────────────────────────────────
if [[ "$(checkpoint_get indexes_created)" != "done" ]]; then
  log "Creating PrimeKG indexes..."
  cat > /tmp/primekg_indexes.cypher << 'EOF'
CREATE INDEX primekg_entity_index IF NOT EXISTS FOR (n:PrimeKG) ON (n.entity_index);
CREATE INDEX primekg_entity_id IF NOT EXISTS FOR (n:PrimeKG) ON (n.entity_id);
CREATE INDEX primekg_name IF NOT EXISTS FOR (n:PrimeKG) ON (n.name);
CREATE FULLTEXT INDEX primekg_name_ft IF NOT EXISTS FOR (n:PrimeKG) ON EACH [n.name];
EOF
  kubectl cp /tmp/primekg_indexes.cypher "$NAMESPACE/$POD:/tmp/primekg_indexes.cypher"
  cypher_run "$POD" /tmp/primekg_indexes.cypher
  checkpoint_set indexes_created done
  log "Indexes ready."
else
  log "Skipping index creation (already done)."
fi

# ── Step 4: Import all nodes (Phase 1) ───────────────────────────────────
# Type sanitization: "gene/protein" -> "GeneProtein", "biological_process" -> "BiologicalProcess"
# Uses: reduce(s='', part IN split(replace(type, '/', '_'), '_') | s + capitalize(part))
if [[ "$(checkpoint_get nodes_imported)" != "done" ]]; then
  log "Phase 1: Importing nodes (129K expected)..."
  # Use x_index/y_index (global unique integer) as merge key — NOT entity_id.
  # entity_id is NOT unique across node types (e.g., Anatomy ID 9796 = Gene ID 9796).
  cat > /tmp/primekg_nodes.cypher << 'EOF'
CALL apoc.periodic.iterate(
  "LOAD CSV WITH HEADERS FROM 'file:///kg.csv' AS row RETURN row",
  "WITH row,
     reduce(s = '', part IN split(replace(coalesce(row.x_type,'unknown'), '/', '_'), '_') | s + apoc.text.capitalize(part)) AS xLabel,
     reduce(s = '', part IN split(replace(coalesce(row.y_type,'unknown'), '/', '_'), '_') | s + apoc.text.capitalize(part)) AS yLabel
   CALL apoc.merge.node(
     ['PrimeKG', xLabel],
     {entity_index: toInteger(row.x_index)},
     {entity_id: row.x_id, name: row.x_name, source: row.x_source, type: row.x_type},
     {}
   ) YIELD node AS n1
   WITH row, yLabel, n1
   CALL apoc.merge.node(
     ['PrimeKG', yLabel],
     {entity_index: toInteger(row.y_index)},
     {entity_id: row.y_id, name: row.y_name, source: row.y_source, type: row.y_type},
     {}
   ) YIELD node AS n2
   RETURN n1, n2",
  {batchSize: 5000, parallel: false, retries: 3}
) YIELD batches, total, errorMessages
RETURN batches, total, errorMessages;
EOF
  kubectl cp /tmp/primekg_nodes.cypher "$NAMESPACE/$POD:/tmp/primekg_nodes.cypher"
  log "Running node import (10-30 min)..."
  cypher_run "$POD" /tmp/primekg_nodes.cypher
  # Validate: node count must be ~129K (PrimeKG spec)
  NODE_COUNT=$(kubectl exec -n "$NAMESPACE" "$POD" -- \
    cypher-shell -u "$NEO4J_USER" -p "$NEO4J_PASS" --format plain \
    "MATCH (n:PrimeKG) RETURN count(n) AS c;" 2>/dev/null | tail -1)
  log "Node count after import: $NODE_COUNT (expected ~129375)"
  if [[ "$NODE_COUNT" -lt 120000 ]]; then
    log "ERROR: node count too low — aborting"
    exit 1
  fi
  checkpoint_set nodes_imported done
  log "Phase 1 complete."
else
  log "Skipping node import (already done)."
fi

# ── Step 5: Import edges by clinical priority (Phase 2) ───────────────────
# Actual relation types from kg.csv (verified against real data):
#   indication, contraindication, off-label use  — drug-disease (clinical)
#   drug_drug                                     — DDI (patient safety)
#   disease_protein, drug_protein                 — mechanism level
#   protein_protein                               — 642K edges
#   disease_phenotype_positive/negative, disease_disease
#   anatomy_protein_present, anatomy_protein_absent, anatomy_anatomy
#   bioprocess_*, cellcomp_*, molfunc_*, pathway_*, phenotype_*, exposure_*

declare -A REL_FILTERS=(
  ["indication_contraindication"]="row.relation IN ['indication', 'contraindication', 'off-label use']"
  ["drug_drug"]="row.relation = 'drug_drug'"
  ["disease_drug_protein"]="row.relation IN ['disease_protein', 'drug_protein']"
  ["protein_protein"]="row.relation = 'protein_protein'"
  ["disease_phenotype"]="row.relation IN ['disease_phenotype_positive', 'disease_phenotype_negative', 'disease_disease']"
  ["anatomy_protein"]="row.relation IN ['anatomy_protein_present', 'anatomy_protein_absent', 'anatomy_anatomy']"
  ["other"]="NOT (row.relation IN ['indication', 'contraindication', 'off-label use', 'drug_drug', 'disease_protein', 'drug_protein', 'protein_protein', 'disease_phenotype_positive', 'disease_phenotype_negative', 'disease_disease', 'anatomy_protein_present', 'anatomy_protein_absent', 'anatomy_anatomy'])"
)

PRIORITY_ORDER=(
  "indication_contraindication"
  "drug_drug"
  "disease_drug_protein"
  "protein_protein"
  "disease_phenotype"
  "anatomy_protein"
  "other"
)

for rel_type in "${PRIORITY_ORDER[@]}"; do
  checkpoint_key="edges_${rel_type}"
  if [[ "$(checkpoint_get "$checkpoint_key")" == "done" ]]; then
    log "Skipping edges '$rel_type' (already done)."
    continue
  fi

  filter="${REL_FILTERS[$rel_type]}"
  log "Phase 2 [$rel_type]: importing edges..."

  cat > /tmp/primekg_edges.cypher << CYPHER
CALL apoc.periodic.iterate(
  "LOAD CSV WITH HEADERS FROM 'file:///kg.csv' AS row WITH row WHERE $filter RETURN row",
  "MATCH (x:PrimeKG {entity_index: toInteger(row.x_index)})
   MATCH (y:PrimeKG {entity_index: toInteger(row.y_index)})
   CALL apoc.merge.relationship(
     x,
     toUpper(replace(replace(replace(row.relation, ' ', '_'), '-', '_'), '/', '_')),
     {},
     {display_relation: row.display_relation, source: 'primekg'},
     y,
     {}
   ) YIELD rel
   RETURN rel",
  {batchSize: 5000, parallel: false, retries: 3}
) YIELD batches, total, errorMessages
RETURN batches, total, errorMessages;
CYPHER

  kubectl cp /tmp/primekg_edges.cypher "$NAMESPACE/$POD:/tmp/primekg_edges.cypher"
  RESULT=$(cypher_run "$POD" /tmp/primekg_edges.cypher)
  echo "$RESULT"
  # Validate: errorMessages must be empty {}
  if echo "$RESULT" | grep -q '"[^"]\+":'; then
    log "WARNING: errorMessages non-empty for '$rel_type' — check Neo4j logs"
  fi
  # Validate: total edges created must be > 0
  EDGE_TOTAL=$(echo "$RESULT" | grep -E '^[0-9]' | awk '{print $2}' | head -1)
  if [[ -n "$EDGE_TOTAL" && "$EDGE_TOTAL" -eq 0 ]]; then
    log "ERROR: 0 edges created for '$rel_type' — MATCH may have failed (wrong entity_index?)"
    exit 1
  fi
  checkpoint_set "$checkpoint_key" done
  log "Edges '$rel_type' imported ✓ (total: $EDGE_TOTAL)"
done

# ── Step 6: SAME_AS cross-linking (Phase 3) ───────────────────────────────
if [[ "$(checkpoint_get same_as_linked)" != "done" ]]; then
  log "Phase 3: Building SAME_AS links (tenant entities ↔ PrimeKG nodes)..."
  cat > /tmp/primekg_same_as.cypher << 'EOF'
// Strategy 1: exact ontology_id match (confidence 1.0)
MATCH (t:Entity)
WHERE t.ontology_id IS NOT NULL
MATCH (p:PrimeKG {entity_id: t.ontology_id})
WHERE NOT EXISTS { MATCH (t)-[:SAME_AS]->(p) }
MERGE (t)-[:SAME_AS {
  confidence: 1.0,
  match_strategy: "ontology_id",
  linked_at: datetime(),
  primekg_version: "2022-04"
}]->(p);

// Strategy 2: normalized name exact match (confidence 0.9)
MATCH (t:Entity)
WHERE NOT EXISTS { MATCH (t)-[:SAME_AS]->() }
MATCH (p:PrimeKG)
WHERE toLower(trim(t.name)) = toLower(trim(p.name))
MERGE (t)-[:SAME_AS {
  confidence: 0.9,
  match_strategy: "name",
  linked_at: datetime(),
  primekg_version: "2022-04"
}]->(p);
EOF
  kubectl cp /tmp/primekg_same_as.cypher "$NAMESPACE/$POD:/tmp/primekg_same_as.cypher"
  cypher_run "$POD" /tmp/primekg_same_as.cypher
  checkpoint_set same_as_linked done
  log "SAME_AS links built ✓"
else
  log "Skipping SAME_AS (already done)."
fi

# ── Step 7: Verify ────────────────────────────────────────────────────────
log "Verifying import..."
cat > /tmp/primekg_verify.cypher << 'EOF'
MATCH (n:PrimeKG) RETURN labels(n) AS labels, count(n) AS count ORDER BY count DESC;
MATCH ()-[r]->(:PrimeKG) RETURN type(r) AS rel_type, count(r) AS cnt ORDER BY cnt DESC LIMIT 15;
MATCH (:Entity)-[:SAME_AS]->(:PrimeKG) RETURN count(*) AS same_as_links;
EOF
kubectl cp /tmp/primekg_verify.cypher "$NAMESPACE/$POD:/tmp/primekg_verify.cypher"
cypher_run "$POD" /tmp/primekg_verify.cypher

checkpoint_set import_complete done
log "PrimeKG import complete ✓"
log "Checkpoint: $CHECKPOINT"

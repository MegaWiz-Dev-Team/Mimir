#!/usr/bin/env bash
# Day 1 — Create asgard_surgical tenant + anesthesia_kb_001 Qdrant collection
# Idempotent: safe to re-run; uses INSERT ... ON DUPLICATE KEY UPDATE + Qdrant 200/409 OK
#
# Usage: bash 01_create_tenant_and_collection.sh
#
set -euo pipefail

INFRA_NS="asgard-infra"
QDRANT_PF_PORT=16333
COLLECTION="anesthesia_kb_001"

echo "==================================================================="
echo "Day 1 — Create asgard_surgical tenant + anesthesia_kb_001 collection"
echo "==================================================================="

# ────────────────────────────────────────────────────────────────────
# 1. Create tenant + tenant_config
# ────────────────────────────────────────────────────────────────────
echo ""
echo "[1/3] Creating asgard_surgical tenant + config..."

kubectl -n "$INFRA_NS" exec -i deploy/mariadb -- mariadb -uroot -proot \
  --default-character-set=utf8mb4 mimir <<'SQL'
-- Tenant master row (idempotent)
INSERT INTO tenants (id, name, domain, service_type, description)
VALUES (
  'asgard_surgical',
  'Asgard Surgical',
  'surgical.megacare.com',
  'medical',
  'Surgical AI — Phase 1 wedge (general + aesthetic). KB: RCAT 22 guidelines, op note templates, consent templates. Beachhead: private hospital general surgery + aesthetic clinic.'
)
ON DUPLICATE KEY UPDATE
  name=VALUES(name),
  description=VALUES(description),
  service_type=VALUES(service_type),
  updated_at=CURRENT_TIMESTAMP;

-- Tenant config (LLM = gemma-4-26b local, PII mask, on-prem only)
INSERT INTO tenant_configs (
  tenant_id,
  default_provider,
  default_model,
  pii_mode,
  max_daily_tokens,
  is_dedicated_vector_db,
  ocr_cloud_flash_enabled,
  ocr_cloud_pro_enabled,
  ocr_phi_strict
)
VALUES (
  'asgard_surgical',
  'heimdall',
  'mlx-community/gemma-3-26b-a4b-it-4bit',
  'mask-and-send',
  500000,
  0,
  0,
  0,
  1
)
ON DUPLICATE KEY UPDATE
  default_model=VALUES(default_model),
  pii_mode=VALUES(pii_mode),
  max_daily_tokens=VALUES(max_daily_tokens),
  updated_at=CURRENT_TIMESTAMP;

-- Verify
SELECT id, name, domain, service_type FROM tenants WHERE id='asgard_surgical';
SELECT tenant_id, default_model, pii_mode, max_daily_tokens
FROM tenant_configs WHERE tenant_id='asgard_surgical';
SQL

echo "✓ Tenant + config created"

# ────────────────────────────────────────────────────────────────────
# 2. Port-forward Qdrant + create collection
# ────────────────────────────────────────────────────────────────────
echo ""
echo "[2/3] Creating Qdrant collection $COLLECTION (1024-dim BGE-M3 Cosine)..."

# Kill any existing port-forward on that port
lsof -ti :$QDRANT_PF_PORT 2>/dev/null | xargs -r kill 2>/dev/null || true
sleep 1

kubectl port-forward -n "$INFRA_NS" svc/qdrant ${QDRANT_PF_PORT}:6333 > /tmp/qdrant-pf-day1.log 2>&1 &
PF_PID=$!
trap "kill $PF_PID 2>/dev/null || true" EXIT
sleep 3

# Create collection (idempotent — Qdrant returns 200 even if exists with same config)
HTTP=$(curl -sS -o /tmp/qdrant-create.json -w "%{http_code}" -X PUT "http://localhost:${QDRANT_PF_PORT}/collections/${COLLECTION}" \
  -H 'Content-Type: application/json' \
  -d '{
    "vectors": {"size": 1024, "distance": "Cosine"},
    "optimizers_config": {"indexing_threshold": 1000},
    "hnsw_config": {"m": 16, "ef_construct": 100}
  }')

case "$HTTP" in
  200) echo "✓ Collection $COLLECTION created" ;;
  409) echo "✓ Collection $COLLECTION already exists (idempotent OK)" ;;
  *)   echo "✗ Unexpected HTTP $HTTP"; cat /tmp/qdrant-create.json; exit 1 ;;
esac

# Verify
echo ""
echo "[3/3] Verifying collection..."
curl -sS "http://localhost:${QDRANT_PF_PORT}/collections/${COLLECTION}" | \
  python3 -c "import json,sys; d=json.load(sys.stdin); r=d['result']; print(f\"  Name:    {r.get('config',{}).get('params',{})}\"); print(f\"  Status:  {r.get('status')}\"); print(f\"  Vectors: {r.get('vectors_count', 0)}\")"

echo ""
echo "==================================================================="
echo "Day 1 COMPLETE — asgard_surgical tenant + anesthesia_kb_001 ready"
echo "Next: bash 02_ingest_rcat.sh"
echo "==================================================================="

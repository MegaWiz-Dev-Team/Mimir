#!/usr/bin/env bash
# Ingest the AI/Cyber Security reference KB into Mimir for the asgard_security tenant,
# then embed it into Qdrant (source_chunks) so Odin's knowledge_search can RAG it.
#
# Idempotent: re-running replaces the KB's data_source + chunks.
# Tenant-scoped (asgard_security) because Mimir's vector search filters by tenant
# (source_chunks, target_tenant) — a truly cross-tenant SHARED KB (tenant_id NULL)
# would additionally need a search-union change in vector.rs::search_vectors.
#
# Prereqs: kubectl context on the Asgard cluster; the asgard_security tenant exists
# (seed-asgard-security-tenant.sql); Mimir + Heimdall running.
set -euo pipefail

NS=asgard
INFRA_NS=asgard-infra
MD="$(dirname "$0")/ai-security-guidelines.md"
TENANT=asgard_security
KB_NAME='AI & Cyber Security Guidelines'

MPOD=$(kubectl -n "$NS" get pods -l app=mimir-api -o jsonpath='{.items[0].metadata.name}')
DBURL=$(kubectl -n "$NS" exec "$MPOD" -- printenv DATABASE_URL)
PASS=$(printf '%s' "$DBURL" | sed -E 's#mysql://[^:]+:([^@]+)@.*#\1#')
MARIAPOD=$(kubectl -n "$INFRA_NS" get pods -o name | grep -iE 'maria|mysql' | head -1)
MARIAPOD=${MARIAPOD#pod/}

# 1) markdown → idempotent SQL (one chunk per '## ' section)
SQL=$(python3 - "$MD" "$TENANT" "$KB_NAME" <<'PY'
import sys,re
md=open(sys.argv[1]).read(); tenant=sys.argv[2]; name=sys.argv[3]
chunks=[p.strip() for p in re.split(r'(?m)^(?=## )', md) if p.strip().startswith('## ')]
esc=lambda s: s.replace("\\","\\\\").replace("'","''")
ten=esc(tenant); nm=esc(name)
print(f"DELETE c FROM chunks c JOIN data_sources d ON c.source_id=d.id WHERE d.tenant_id='{ten}' AND d.name='{nm}';")
print(f"DELETE FROM data_sources WHERE tenant_id='{ten}' AND name='{nm}';")
print(f"INSERT INTO data_sources (tenant_id,name,source_type,config_json,storage_mode,total_chunks) VALUES ('{ten}','{nm}','document','{{}}','markdown',{len(chunks)});")
print("SET @sid=LAST_INSERT_ID();")
print("INSERT INTO chunks (source_id,chunk_index,content) VALUES "+",".join(f"(@sid,{i},'{esc(c)}')" for i,c in enumerate(chunks))+";")
print("SELECT @sid;")
PY
)
SID=$(echo "$SQL" | kubectl -n "$INFRA_NS" exec -i "$MARIAPOD" -- sh -c "mariadb -u mimir -p'$PASS' mimir -N" | tail -1 | awk '{print $1}')
echo "data_source id=$SID (tenant=$TENANT)"

# 2) embed chunks → Qdrant source_chunks (tenant-scoped)
PORT=$(kubectl -n "$NS" get svc mimir-api -o jsonpath='{.spec.ports[0].port}')
kubectl -n "$NS" port-forward svc/mimir-api 18090:"$PORT" >/tmp/mimir-kb-pf.log 2>&1 &
PF=$!; trap 'kill $PF 2>/dev/null' EXIT
for i in $(seq 1 30); do curl -s -o /dev/null http://localhost:18090/ && break; sleep 0.3; done
curl -s -X POST http://localhost:18090/api/v1/vector/embed-chunks \
  -H 'Content-Type: application/json' -H "X-Tenant-Id: $TENANT" \
  --data "{\"source_id\":$SID}"
echo
echo "Done. Point Odin: MIMIR_TENANT=$TENANT, MIMIR_KB_SOURCE_IDS=$SID"

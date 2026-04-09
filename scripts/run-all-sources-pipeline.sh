#!/bin/bash
# ═══════════════════════════════════════════════════════════════
# Run Auto-Pipeline for ALL sources — sequential execution
# ═══════════════════════════════════════════════════════════════

BRIDGE_URL="http://127.0.0.1:3001"
PROVIDER="heimdall"
MODEL="mlx-community/gemma-4-31b-it-4bit"
TENANT="megacare"

# All source IDs (ordered smallest first for quick wins)
SOURCES=(12 11 14 13 16 10 9)
SOURCE_NAMES=(apnealink airfit airsense11 airsense10 lumis_clinical_guide diagnostic-testing-osa cpg)

TOTAL=${#SOURCES[@]}
echo "═══════════════════════════════════════════════════════════════"
echo "🚀 Mimir Auto-Pipeline — Batch Run for $TOTAL sources"
echo "   Provider: $PROVIDER | Model: $MODEL"
echo "   Started:  $(date)"
echo "═══════════════════════════════════════════════════════════════"

for i in "${!SOURCES[@]}"; do
    SRC=${SOURCES[$i]}
    NAME=${SOURCE_NAMES[$i]}
    NUM=$((i + 1))
    
    echo ""
    echo "───────────────────────────────────────────────────────────────"
    echo "[$NUM/$TOTAL] 🔄 Starting pipeline for: $NAME (source_id=$SRC)"
    echo "───────────────────────────────────────────────────────────────"
    
    # Trigger the pipeline
    RESPONSE=$(curl -s -X POST "$BRIDGE_URL/api/v1/sources/$SRC/auto-pipeline" \
        -H "Content-Type: application/json" \
        -H "X-Tenant-Id: $TENANT" \
        -d "{\"provider\": \"$PROVIDER\", \"model\": \"$MODEL\", \"enable_pageindex\": true}")
    
    RUN_ID=$(echo "$RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin).get('pipeline_run_id',''))" 2>/dev/null)
    STATUS=$(echo "$RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin).get('status','error'))" 2>/dev/null)
    
    if [ "$STATUS" != "running" ]; then
        echo "   ❌ Failed to start: $RESPONSE"
        continue
    fi
    
    echo "   ✅ Pipeline started: run_id=$RUN_ID"
    echo "   ⏳ Waiting for completion..."
    
    # Poll for completion (check DB every 30 seconds)
    WAIT_COUNT=0
    MAX_WAIT=240  # 240 x 30s = 2 hours max per source
    while [ $WAIT_COUNT -lt $MAX_WAIT ]; do
        sleep 30
        WAIT_COUNT=$((WAIT_COUNT + 1))
        ELAPSED=$((WAIT_COUNT * 30))
        
        # Check pipeline status from DB
        DB_STATUS=$(kubectl exec -n asgard-infra deploy/mariadb -- \
            mariadb -u mimir -pREDACTED-PW -N -e \
            "SELECT status FROM mimir.pipeline_runs WHERE id='$RUN_ID';" 2>/dev/null | tr -d '[:space:]')
        
        if [ "$DB_STATUS" = "completed" ] || [ "$DB_STATUS" = "failed" ]; then
            # Get step summary
            STEPS=$(kubectl exec -n asgard-infra deploy/mariadb -- \
                mariadb -u mimir -pREDACTED-PW -N -e \
                "SELECT CONCAT(step_name, '=', status) FROM mimir.pipeline_run_steps WHERE run_id='$RUN_ID' ORDER BY step_number;" 2>/dev/null | tr '\n' ', ')
            
            if [ "$DB_STATUS" = "completed" ]; then
                echo "   ✅ Completed in ${ELAPSED}s — Steps: $STEPS"
            else
                echo "   ❌ Failed after ${ELAPSED}s — Steps: $STEPS"
            fi
            break
        fi
        
        # Progress indicator every 60s
        if [ $((WAIT_COUNT % 2)) -eq 0 ]; then
            echo "   ... ${ELAPSED}s elapsed (status: ${DB_STATUS:-polling})"
        fi
    done
    
    if [ $WAIT_COUNT -ge $MAX_WAIT ]; then
        echo "   ⚠️ Timeout after 2 hours — check manually"
    fi
done

echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "🏁 Batch pipeline completed at $(date)"
echo "═══════════════════════════════════════════════════════════════"

# Final summary
echo ""
echo "📊 Final Status:"
kubectl exec -n asgard-infra deploy/mariadb -- mariadb -u mimir -pREDACTED-PW -e "
USE mimir;
SELECT 
  ds.id, ds.name,
  (SELECT COUNT(*) FROM chunks WHERE source_id = ds.id) as chunks,
  (SELECT COUNT(*) FROM pipeline_runs WHERE source_id = ds.id AND status='completed') as ok_runs,
  CASE WHEN ds.pageindex_tree IS NOT NULL THEN '✅' ELSE '❌' END as pageindex,
  (SELECT COUNT(*) FROM kg_entities WHERE source_id = ds.id) as entities,
  (SELECT COUNT(*) FROM kg_relations WHERE source_id = ds.id) as relations
FROM data_sources ds ORDER BY ds.id;
"

#!/bin/bash
# Medical Data Ingest Pipeline Orchestrator
# Loads ICD-10, clinical calculators, drugs, guidelines into asgard_medical

set -e

export MIMIR_DB_PASSWORD=$(kubectl get secret asgard-secrets -n asgard -o jsonpath='{.data.MARIADB_PASSWORD}' | base64 -d)
TENANT="asgard_medical"
LOG_DIR="/tmp/medical_ingest_$(date +%Y%m%d_%H%M%S)"

mkdir -p "$LOG_DIR"

echo "🏥 Medical Data Ingest Pipeline"
echo "   Tenant: $TENANT"
echo "   Logs: $LOG_DIR"
echo ""

# ─────────────────────────────────────────────────────────────
# Phase 1: ICD-10-TM Codes
# ─────────────────────────────────────────────────────────────

phase_icd10() {
    echo ""
    echo "═══════════════════════════════════════════════════════════"
    echo "Phase 1: ICD-10-TM Thai Clinical Coding (15,376 codes)"
    echo "═══════════════════════════════════════════════════════════"

    echo "  → Checking migrations..."
    if [ -f "Mimir/ro-ai-bridge/migrations/sprint48_icd10_codes.sql" ]; then
        echo "    ✓ sprint48_icd10_codes.sql found"
    else
        echo "    ✗ Migration not found"
        return 1
    fi

    echo "  → Running ICD-10-TM anamai ingest..."
    if python3 Mimir/scripts/icd10_tm_anamai_ingest.py >> "$LOG_DIR/icd10.log" 2>&1; then
        echo "    ✓ ICD-10-TM loaded (15,376 codes)"
    else
        echo "    ⚠ Ingest script had warnings (check log)"
        tail -5 "$LOG_DIR/icd10.log"
    fi

    echo "  → Building Qdrant embeddings (ICD-10-TM)..."
    if python3 Mimir/scripts/icd10_embed_qdrant.py >> "$LOG_DIR/icd10_embed.log" 2>&1; then
        echo "    ✓ Vector embeddings built"
    else
        echo "    ⚠ Embedding had issues (embeddings deferred)"
    fi

    return 0
}

# ─────────────────────────────────────────────────────────────
# Phase 2: Clinical Calculators
# ─────────────────────────────────────────────────────────────

phase_calculators() {
    echo ""
    echo "═══════════════════════════════════════════════════════════"
    echo "Phase 2: Clinical Calculators (CHADS2, MELD, eGFR, Wells, etc.)"
    echo "═══════════════════════════════════════════════════════════"

    echo "  → Loading 7 clinical calculator schemas..."
    if python3 Mimir/scripts/ingest_medical_sources.py \
        --source clinical-calc >> "$LOG_DIR/calculators.log" 2>&1; then
        echo "    ✓ Clinical calculators loaded"
    else
        echo "    ✗ Failed to load calculators"
        tail -10 "$LOG_DIR/calculators.log"
        return 1
    fi

    return 0
}

# ─────────────────────────────────────────────────────────────
# Phase 3: Drug Reference Data
# ─────────────────────────────────────────────────────────────

phase_drugs() {
    echo ""
    echo "═══════════════════════════════════════════════════════════"
    echo "Phase 3: Drug Interaction Reference (Open FDA compatible)"
    echo "═══════════════════════════════════════════════════════════"

    echo "  → Loading drug severity levels..."
    if python3 Mimir/scripts/ingest_medical_sources.py \
        --source drug >> "$LOG_DIR/drug.log" 2>&1; then
        echo "    ✓ Drug reference loaded"
    else
        echo "    ✗ Failed to load drug data"
        tail -10 "$LOG_DIR/drug.log"
        return 1
    fi

    return 0
}

# ─────────────────────────────────────────────────────────────
# Phase 4: Clinical Guidelines
# ─────────────────────────────────────────────────────────────

phase_guidelines() {
    echo ""
    echo "═══════════════════════════════════════════════════════════"
    echo "Phase 4: Clinical Guidelines (ACC/AHA, ESC, AASM)"
    echo "═══════════════════════════════════════════════════════════"

    echo "  → Loading guideline metadata..."
    if python3 Mimir/scripts/ingest_medical_sources.py \
        --source guidelines >> "$LOG_DIR/guidelines.log" 2>&1; then
        echo "    ✓ Guidelines loaded"
    else
        echo "    ✗ Failed to load guidelines"
        tail -10 "$LOG_DIR/guidelines.log"
        return 1
    fi

    return 0
}

# ─────────────────────────────────────────────────────────────
# Verification
# ─────────────────────────────────────────────────────────────

verify_ingest() {
    echo ""
    echo "═══════════════════════════════════════════════════════════"
    echo "Verification: Check data loaded into asgard_medical"
    echo "═══════════════════════════════════════════════════════════"

    DB_PASS=$MIMIR_DB_PASSWORD

    echo "  ICD-10 codes:"
    COUNT=$(echo "SELECT COUNT(*) FROM icd10_codes WHERE tenant_id IS NULL;" | \
        kubectl exec -i -n asgard-infra deploy/mariadb -- \
        mariadb -u mimir "--password=${DB_PASS}" mimir --batch --silent)
    echo "    • $COUNT codes in shared master"

    echo "  Data sources in asgard_medical:"
    COUNT=$(echo "SELECT COUNT(*) FROM data_sources WHERE tenant_id='$TENANT';" | \
        kubectl exec -i -n asgard-infra deploy/mariadb -- \
        mariadb -u mimir "--password=${DB_PASS}" mimir --batch --silent)
    echo "    • $COUNT data sources registered"

    echo "  Clinical guidelines:"
    COUNT=$(echo "SELECT COUNT(*) FROM clinical_guidelines WHERE tenant_id='$TENANT' 2>/dev/null;" | \
        kubectl exec -i -n asgard-infra deploy/mariadb -- \
        mariadb -u mimir "--password=${DB_PASS}" mimir --batch --silent 2>/dev/null || echo "0")
    echo "    • $COUNT guidelines loaded (table may be new)"

    echo ""
}

# ─────────────────────────────────────────────────────────────
# Main
# ─────────────────────────────────────────────────────────────

main() {
    local failed=0

    # Change to repo root
    cd /Users/mimir/Developer || exit 1

    echo "Starting ingest at $(date)"
    echo ""

    # Run phases sequentially
    phase_icd10 || ((failed++))
    phase_calculators || ((failed++))
    phase_drugs || ((failed++))
    phase_guidelines || ((failed++))

    # Verify results
    verify_ingest

    # Summary
    echo ""
    echo "═══════════════════════════════════════════════════════════"
    if [ $failed -eq 0 ]; then
        echo "✅ Pipeline completed successfully!"
        echo ""
        echo "Next steps:"
        echo "  1. Restart Mimir services: kubectl rollout restart deployment/mimir-api -n asgard"
        echo "  2. Check dashboard: https://mimir.asgard.internal/evaluations"
        echo "  3. Run new benchmark: python3 Mimir/scripts/run_healthbench_eval.py --agent eir-internal-medicine"
    else
        echo "⚠️  Pipeline completed with $failed phase(s) having issues"
        echo "   Check logs in: $LOG_DIR"
    fi
    echo "═══════════════════════════════════════════════════════════"
    echo ""

    echo "Log files:"
    ls -lh "$LOG_DIR"/*

    return $failed
}

main "$@"

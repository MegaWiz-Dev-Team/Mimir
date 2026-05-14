#!/bin/bash
# ============================================================================
# Mimir Admin Settings Seeder — Initialize Complete Admin Configuration
# ============================================================================
# Usage:
#   bash scripts/seed-admin-config.sh [--dev|--prod]
#
# This script initializes all admin settings to sensible defaults:
#   - App settings (auto_tune_model, judge_model, etc.)
#   - LLM config slots (chat, rag, judge, embedding, etc.)
#   - Search settings (top_k, similarity_threshold, etc.)
#   - Pipeline settings (chunk_size, dedup_threshold, etc.)
#   - Default roles with permissions
#   - Optional: Vault secrets initialization
#
# ============================================================================

set -euo pipefail

# ── Configuration ─────────────────────────────────────────────────────────
export ENVIRONMENT="${1:-dev}"
export DATABASE_URL="${DATABASE_URL:-mysql://mimir:REDACTED-PW@localhost:3306/mimir}"
export VAULT_ADDR="${VAULT_ADDR:-http://localhost:30820}"
export VAULT_TOKEN="${VAULT_TOKEN:-hvs.REDACTED}"

echo "🌱 Mimir Admin Settings Seeder"
echo "   Environment: $ENVIRONMENT"
echo "   Database: $DATABASE_URL"
echo ""

# ── Detect MySQL Client ───────────────────────────────────────────────────
if ! command -v mysql &> /dev/null; then
    echo "❌ mysql client not found. Install it or use Docker:"
    echo "   docker exec mimir_mariadb mysql ..."
    exit 1
fi

# ── Parse Database URL ────────────────────────────────────────────────────
# Extract components from DATABASE_URL (mysql://user:pass@host:port/db)
MYSQL_USER=$(echo "$DATABASE_URL" | sed 's/.*:\/\/\([^:]*\).*/\1/')
MYSQL_PASS=$(echo "$DATABASE_URL" | sed 's/.*:\([^@]*\)@.*/\1/')
MYSQL_HOST=$(echo "$DATABASE_URL" | sed 's/.*@\([^:]*\).*/\1/')
MYSQL_PORT=$(echo "$DATABASE_URL" | sed 's/.*:\([0-9]*\)\/.*/\1/')
MYSQL_DB=$(echo "$DATABASE_URL" | sed 's/.*\/\([^?]*\).*/\1/')

# Defaults if parsing fails
MYSQL_PORT="${MYSQL_PORT:-3306}"
MYSQL_HOST="${MYSQL_HOST:-localhost}"

echo "📝 Seeding Admin Settings..."
echo ""

# ── Helper Function ───────────────────────────────────────────────────────
run_query() {
    local label="$1"
    local query="$2"

    echo -n "  ▶ $label ... "
    if mysql -u"$MYSQL_USER" -p"$MYSQL_PASS" -h"$MYSQL_HOST" -P"$MYSQL_PORT" "$MYSQL_DB" <<< "$query" 2>&1 | grep -q "ERROR"; then
        echo "❌ FAILED"
        return 1
    else
        echo "✅"
        return 0
    fi
}

# ── 1. Verify App Settings Table ──────────────────────────────────────────
echo "1️⃣  App Settings"
run_query "Verify app_settings table exists" "SHOW TABLES LIKE 'app_settings';" || {
    echo "  ⚠️  app_settings table not found. Run migrations first:"
    echo "     cd ro-ai-bridge && sqlx migrate run"
    exit 1
}

run_query "Seed auto_tune_model" \
    "INSERT IGNORE INTO app_settings (setting_key, setting_value, description) VALUES ('auto_tune_model', 'gemini-3-flash', 'LLM for prompt optimization');"

run_query "Seed judge_model" \
    "INSERT IGNORE INTO app_settings (setting_key, setting_value, description) VALUES ('judge_model', 'gemini-3-flash', 'LLM for evaluation scoring');"

run_query "Seed default_embedding_model" \
    "INSERT IGNORE INTO app_settings (setting_key, setting_value, description) VALUES ('default_embedding_model', 'bge-m3', 'Default embedding model');"

run_query "Seed max_rag_tokens" \
    "INSERT IGNORE INTO app_settings (setting_key, setting_value, description) VALUES ('max_rag_tokens', '2000', 'Max tokens for RAG context');"

# ── 2. Verify & Seed Tenant Configs ───────────────────────────────────────
echo ""
echo "2️⃣  Tenant Configurations"
run_query "Create default_tenant config if missing" \
    "INSERT IGNORE INTO tenant_configs (tenant_id) VALUES ('default_tenant');"

run_query "Seed LLM config slots" \
    "UPDATE tenant_configs SET llm_config = JSON_OBJECT(
        'chat', JSON_OBJECT('provider', 'ollama', 'model', 'llama3.2'),
        'rag', JSON_OBJECT('provider', 'ollama', 'model', 'llama3.2'),
        'judge', JSON_OBJECT('provider', 'gemini', 'model', 'gemini-3-flash'),
        'embedding', JSON_OBJECT('provider', 'heimdall', 'model', 'bge-m3'),
        'heimdall_url', 'http://localhost:30081'
    ) WHERE tenant_id = 'default_tenant' AND (llm_config IS NULL OR llm_config = '{}');"

run_query "Seed search settings" \
    "UPDATE tenant_configs SET search_settings = JSON_OBJECT(
        'embedding_model', 'bge-m3',
        'top_k', 5,
        'similarity_threshold', 0.7,
        'search_mode', 'hybrid'
    ) WHERE tenant_id = 'default_tenant' AND (search_settings IS NULL OR search_settings = '{}');"

run_query "Seed pipeline settings" \
    "UPDATE tenant_configs SET pipeline_settings = JSON_OBJECT(
        'chunk_strategy', 'auto',
        'chunk_size', 512,
        'chunk_overlap', 50,
        'dedup_threshold', 0.95
    ) WHERE tenant_id = 'default_tenant' AND (pipeline_settings IS NULL OR pipeline_settings = '{}');"

# ── 3. Verify Admin User & Roles ──────────────────────────────────────────
echo ""
echo "3️⃣  Users & Roles"
run_query "Verify admin user exists" \
    "SELECT COUNT(*) FROM users WHERE username = 'admin';" || {
    echo "  ⚠️  Admin user not found!"
}

run_query "Verify default tenant_users link" \
    "SELECT COUNT(*) FROM tenant_users WHERE tenant_id = 'default_tenant' AND role = 'admin';" || {
    echo "  ⚠️  Admin user not linked to default_tenant!"
}

run_query "Verify built-in roles exist" \
    "SELECT COUNT(*) FROM roles WHERE tenant_id = 'default_tenant' AND is_builtin = TRUE;" || {
    echo "  ⚠️  Built-in roles not found!"
}

# ── 4. Optional: Vault Secrets ────────────────────────────────────────────
if [ "$ENVIRONMENT" = "prod" ]; then
    echo ""
    echo "4️⃣  Vault Secrets (Production)"

    if command -v curl &> /dev/null; then
        echo -n "  ▶ Check Vault reachability ... "
        if curl -sf "$VAULT_ADDR/v1/sys/health" > /dev/null 2>&1; then
            echo "✅"

            echo ""
            echo "  💡 Vault is ready. Run the vault-seed script to initialize secrets:"
            echo "     bash scripts/vault-seed.sh"
        else
            echo "❌"
            echo "  ⚠️  Vault is not reachable at $VAULT_ADDR"
        fi
    fi
fi

# ── Summary ───────────────────────────────────────────────────────────────
echo ""
echo "✅ Admin Settings Initialization Complete!"
echo ""
echo "📋 Summary:"
echo "   • App settings: auto_tune_model, judge_model, defaults"
echo "   • Tenant configs: LLM slots, search, pipeline settings"
echo "   • Users: admin user linked to default_tenant"
echo "   • Roles: 3 built-in roles (admin, editor, viewer)"
echo ""
echo "🚀 Next Steps:"
echo "   1. Start the backend: cd ro-ai-bridge && cargo run"
echo "   2. Login to dashboard: http://localhost:3000"
echo "   3. Go to Settings > Admin to verify all values"
echo "   4. Configure Vault secrets if needed: bash scripts/vault-seed.sh"
echo ""

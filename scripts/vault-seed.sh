#!/bin/bash
# ============================================================================
# Vault Secrets Seed Script for Project Mimir
# ============================================================================
# Usage:
#   1. Start Vault:  docker compose up -d vault
#   2. Run script:   bash scripts/vault-seed.sh
#
# This script stores all required secrets in Vault KV v2.
# After running, Mimir will auto-resolve secrets via resolve_secret().
# ============================================================================

set -euo pipefail

# ── Vault Connection ─────────────────────────────────────────────────────
export VAULT_ADDR="${VAULT_ADDR:-http://localhost:8200}"
export VAULT_TOKEN="${VAULT_TOKEN:-mimir-dev-token}"
VAULT_PATH="${VAULT_PATH:-mimir}"

echo "🛡️  Mimir Vault Secrets Seeder"
echo "   Vault: $VAULT_ADDR"
echo "   Path:  secret/$VAULT_PATH"
echo ""

# ── Check Vault is reachable ─────────────────────────────────────────────
if ! curl -sf "$VAULT_ADDR/v1/sys/health" > /dev/null 2>&1; then
    echo "❌ Vault is not reachable at $VAULT_ADDR"
    echo "   Start it with: docker compose up -d vault"
    exit 1
fi
echo "✅ Vault is healthy"

# ── Prompt for secrets ───────────────────────────────────────────────────
echo ""
echo "Enter secrets (press Enter to skip and keep existing value):"
echo ""

read -rp "  GEMINI_API_KEY: " GEMINI_API_KEY
read -rp "  GITHUB_TOKEN: " GITHUB_TOKEN
read -rp "  HEIMDALL_API_KEY: " HEIMDALL_API_KEY
read -rp "  JWT_SECRET: " JWT_SECRET
read -rp "  S3_ACCESS_KEY [minioadmin]: " S3_ACCESS_KEY
read -rp "  S3_SECRET_KEY [minioadmin]: " S3_SECRET_KEY
read -rp "  MARIADB_ROOT_PASSWORD [root]: " MARIADB_ROOT_PASSWORD
read -rp "  MARIADB_PASSWORD [mimir_password]: " MARIADB_PASSWORD

# ── Build JSON payload (only non-empty values) ──────────────────────────
JSON="{"
FIRST=true

add_field() {
    local key="$1"
    local value="$2"
    if [ -n "$value" ]; then
        if [ "$FIRST" = true ]; then
            FIRST=false
        else
            JSON="$JSON,"
        fi
        # Escape special characters in value
        escaped=$(echo "$value" | sed 's/\\/\\\\/g; s/"/\\"/g')
        JSON="$JSON \"$key\": \"$escaped\""
    fi
}

add_field "gemini_api_key" "$GEMINI_API_KEY"
add_field "github_token" "$GITHUB_TOKEN"
add_field "heimdall_api_key" "$HEIMDALL_API_KEY"
add_field "jwt_secret" "$JWT_SECRET"
add_field "s3_access_key" "${S3_ACCESS_KEY:-}"
add_field "s3_secret_key" "${S3_SECRET_KEY:-}"
add_field "mariadb_root_password" "${MARIADB_ROOT_PASSWORD:-}"
add_field "mariadb_password" "${MARIADB_PASSWORD:-}"

JSON="$JSON }"

if [ "$FIRST" = true ]; then
    echo ""
    echo "⚠️  No secrets entered, nothing to store."
    exit 0
fi

# ── Merge with existing secrets (read-modify-write) ──────────────────────
echo ""
echo "📝 Storing secrets in Vault..."

# Read existing secrets (may not exist yet)
EXISTING=$(curl -sf \
    -H "X-Vault-Token: $VAULT_TOKEN" \
    "$VAULT_ADDR/v1/secret/data/$VAULT_PATH" 2>/dev/null \
    | python3 -c "import sys,json; d=json.load(sys.stdin).get('data',{}).get('data',{}); print(json.dumps(d))" 2>/dev/null \
    || echo "{}")

# Merge: existing + new (new values override)
MERGED=$(python3 -c "
import json, sys
existing = json.loads('$EXISTING')
new = json.loads('''$JSON''')
existing.update({k:v for k,v in new.items() if v})
print(json.dumps(existing))
")

# Write merged secrets
HTTP_CODE=$(curl -sf -o /dev/null -w "%{http_code}" \
    -X POST \
    -H "X-Vault-Token: $VAULT_TOKEN" \
    -H "Content-Type: application/json" \
    -d "{\"data\": $MERGED}" \
    "$VAULT_ADDR/v1/secret/data/$VAULT_PATH")

if [ "$HTTP_CODE" = "200" ] || [ "$HTTP_CODE" = "204" ]; then
    echo "✅ Secrets stored successfully!"
else
    echo "❌ Failed to store secrets (HTTP $HTTP_CODE)"
    exit 1
fi

# ── Verify ───────────────────────────────────────────────────────────────
echo ""
echo "📋 Stored keys in secret/$VAULT_PATH:"
curl -sf \
    -H "X-Vault-Token: $VAULT_TOKEN" \
    "$VAULT_ADDR/v1/secret/data/$VAULT_PATH" \
    | python3 -c "
import sys, json
data = json.load(sys.stdin)['data']['data']
for k, v in sorted(data.items()):
    masked = v[:4] + '***' + v[-2:] if len(v) > 8 else '****'
    print(f'   ✓ {k} = {masked}')
"

echo ""
echo "🎉 Done! Mimir will auto-resolve these via resolve_secret()"
echo "   Vault UI: $VAULT_ADDR/ui"

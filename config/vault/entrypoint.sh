#!/bin/sh
# Vault Auto-Init & Auto-Unseal Entrypoint
# -----------------------------------------
# Uses HTTP API (wget) instead of vault CLI for status checks.
# On first run: initializes Vault, saves keys to persistent volume.
# On restart: reads saved keys and auto-unseals.
# Keys stored in /vault/file/init-keys.json

VAULT_ADDR="http://127.0.0.1:8200"
export VAULT_ADDR
KEYS_FILE="/vault/file/init-keys.json"

# Helper: extract JSON string value from pretty-printed file
json_val() {
  sed -n "s/.*\"$1\"[[:space:]]*:[[:space:]]*\"\\([^\"]*\\)\".*/\\1/p" "$2" | head -1
}

# Helper: extract first element from JSON array
json_arr_first() {
  sed -n "/\"$1\"/,/]/p" "$2" | sed -n 's/.*"\([^"]*\)".*/\1/p' | tail -1
}

echo "🔐 Starting Vault server..."
vault server -config=/vault/config/vault-config.hcl &
VAULT_PID=$!

# Wait for Vault HTTP to be ready
echo "⏳ Waiting for Vault to start..."
for i in $(seq 1 30); do
  sleep 1
  wget -q -O /tmp/vault-status "$VAULT_ADDR/v1/sys/health?standbyok=true&sealedcode=200&uninitcode=200" 2>/dev/null && break
done
echo "✅ Vault is responding"

# Get status via HTTP API
STATUS=$(wget -q -O - "$VAULT_ADDR/v1/sys/health?standbyok=true&sealedcode=200&uninitcode=200" 2>/dev/null || echo '{}')
IS_INIT=$(echo "$STATUS" | sed -n 's/.*"initialized":\([a-z]*\).*/\1/p')
IS_SEALED=$(echo "$STATUS" | sed -n 's/.*"sealed":\([a-z]*\).*/\1/p')

echo "📋 Status: initialized=$IS_INIT, sealed=$IS_SEALED"

if [ "$IS_INIT" != "true" ]; then
  echo "🆕 First run — initializing Vault..."

  # Initialize via HTTP API
  INIT_RESP=$(wget -q -O - --post-data='{"secret_shares":1,"secret_threshold":1}' \
    --header="Content-Type: application/json" \
    "$VAULT_ADDR/v1/sys/init" 2>/dev/null)

  # Save init response
  echo "$INIT_RESP" > "$KEYS_FILE"

  # Extract keys (JSON is single-line from HTTP API)
  UNSEAL_KEY=$(echo "$INIT_RESP" | sed 's/.*"keys_base64":\["\([^"]*\)".*/\1/')
  ROOT_TOKEN=$(echo "$INIT_RESP" | sed 's/.*"root_token":"\([^"]*\)".*/\1/')

  echo "🔓 Unsealing Vault..."
  wget -q -O /dev/null --post-data="{\"key\":\"$UNSEAL_KEY\"}" \
    --header="Content-Type: application/json" \
    "$VAULT_ADDR/v1/sys/unseal" 2>/dev/null

  echo "🔑 Root Token: $ROOT_TOKEN"

  # Enable KV v2 secrets engine
  sleep 1
  wget -q -O /dev/null --post-data='{"type":"kv","options":{"version":"2"}}' \
    --header="Content-Type: application/json" \
    --header="X-Vault-Token: $ROOT_TOKEN" \
    "$VAULT_ADDR/v1/sys/mounts/secret" 2>/dev/null || true

  # Also save in a simpler format for the init-keys file
  # Rewrite to pretty JSON using sed
  cat > "$KEYS_FILE" << EOF
{
  "unseal_keys_b64": [
    "$UNSEAL_KEY"
  ],
  "root_token": "$ROOT_TOKEN"
}
EOF

  echo "✅ Vault initialized, unsealed, KV v2 enabled"
  echo "📁 Keys saved to $KEYS_FILE"

elif [ "$IS_SEALED" = "true" ]; then
  echo "🔄 Vault is sealed — auto-unsealing..."

  if [ -f "$KEYS_FILE" ]; then
    UNSEAL_KEY=$(json_arr_first "unseal_keys_b64" "$KEYS_FILE")
    ROOT_TOKEN=$(json_val "root_token" "$KEYS_FILE")

    wget -q -O /dev/null --post-data="{\"key\":\"$UNSEAL_KEY\"}" \
      --header="Content-Type: application/json" \
      "$VAULT_ADDR/v1/sys/unseal" 2>/dev/null

    echo "🔓 Vault unsealed"
    echo "🔑 Root Token: $ROOT_TOKEN"
  else
    echo "⚠️  No keys file — manual unseal required"
  fi

else
  echo "✅ Vault is already initialized and unsealed"
  if [ -f "$KEYS_FILE" ]; then
    ROOT_TOKEN=$(json_val "root_token" "$KEYS_FILE")
    echo "🔑 Root Token: $ROOT_TOKEN"
  fi
fi

echo "🚀 Vault is ready! (Storage: file, Data: /vault/file)"
wait $VAULT_PID

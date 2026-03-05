# Vault Server Configuration — File Backend (persistent)
# Data stored at /vault/file inside container, mapped to ./data/vault on host

storage "file" {
  path = "/vault/file"
}

listener "tcp" {
  address     = "0.0.0.0:8200"
  tls_disable = 1
}

# Allow mlock for security
disable_mlock = true

# API address for CLI usage inside container
api_addr = "http://0.0.0.0:8200"

# UI (optional, useful for debugging)
ui = true

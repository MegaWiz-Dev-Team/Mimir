#!/usr/bin/env bash
# ============================================================================
# Asgard Platform — Deploy Internal Networking & DNS Setup
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
K8S_DIR="$ROOT_DIR/k8s/networking"

# ── Colors ─────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info()  { echo -e "${BLUE}[INFO]${NC}  $*"; }
log_ok()    { echo -e "${GREEN}[OK]${NC}    $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }

if [ "$#" -ne 1 ]; then
    echo "Usage: ./deploy-networking.sh <NODE_IP>"
    echo "Example: ./deploy-networking.sh 100.107.26.89"
    exit 1
fi

NODE_IP="$1"

# 1. Update CoreDNS with Node IP mapping
log_info "Applying CoreDNS Custom Config for NODE_IP: $NODE_IP..."
TMP_COREDNS="/tmp/coredns-custom-$$.yaml"
sed "s/\${NODE_IP}/$NODE_IP/g" "$K8S_DIR/coredns-custom.yaml" > "$TMP_COREDNS"
kubectl apply -f "$TMP_COREDNS"
rm -f "$TMP_COREDNS"
log_ok "CoreDNS configured."

# Restart CoreDNS pod to pick up changes immediately
log_info "Restarting CoreDNS to load new zones..."
kubectl rollout restart deployment/coredns -n kube-system
kubectl rollout status deployment/coredns -n kube-system --timeout=60s
log_ok "CoreDNS restarted."

# 2. Check if cert-manager exists
log_info "Validating cert-manager installation..."
if ! kubectl get namespace cert-manager >/dev/null 2>&1; then
    log_warn "cert-manager not found! Installing automatically..."
    kubectl apply -f https://github.com/cert-manager/cert-manager/releases/download/v1.16.1/cert-manager.yaml
    
    log_info "Waiting for cert-manager to become ready (this might take a minute)..."
    kubectl wait --for=condition=ready pod -l app.kubernetes.io/component=webhook -n cert-manager --timeout=120s || true
    sleep 15
fi

# 3. Apply Cert-Manager Issuers
log_info "Applying Private CA Issuers..."
kubectl apply -f "$K8S_DIR/cert-manager-issuer.yaml"
log_ok "Private CA configured."

# 4. Apply Ingress Routes
log_info "Applying Traefik Ingress Rules..."
kubectl apply -f "$K8S_DIR/ingress-routes.yaml"
log_ok "Ingress active."

echo ""
echo -e "${GREEN}═══════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}  Internal Networking & TLS Deployed Successfully! 🚀${NC}"
echo -e "${GREEN}═══════════════════════════════════════════════════════${NC}"
echo "Test the deployment by checking DNS from inside the cluster:"
echo "  kubectl run -it --rm debug --image=busybox --restart=Never -- nslookup sso.asgard.internal"

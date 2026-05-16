#!/bin/bash

# Deploy S1 Insurance Sprint Dashboard to Grafana
# Usage: bash scripts/deploy_s1_grafana_dashboard.sh

set -e

GRAFANA_URL="http://localhost:30300"
GRAFANA_USER="admin"
GRAFANA_PASSWORD="asgard-grafana"
DASHBOARD_FILE="scripts/s1_grafana_dashboard.json"

echo "🚀 Deploying S1 Dashboard to Grafana..."
echo ""

# Check if Grafana is accessible
echo "🔍 Checking Grafana connectivity..."
if ! curl -s -u "$GRAFANA_USER:$GRAFANA_PASSWORD" "$GRAFANA_URL/api/health" > /dev/null; then
    echo "❌ Error: Cannot connect to Grafana at $GRAFANA_URL"
    echo "   Please ensure:"
    echo "   1. Grafana is running: kubectl get pods -n asgard-monitoring | grep grafana"
    echo "   2. Port 30300 is accessible: http://localhost:30300"
    exit 1
fi

echo "✅ Grafana is accessible"
echo ""

# Check if dashboard JSON exists
if [ ! -f "$DASHBOARD_FILE" ]; then
    echo "❌ Error: Dashboard file not found: $DASHBOARD_FILE"
    exit 1
fi

echo "📤 Uploading dashboard..."

# Prepare dashboard JSON (add folder and overwrite flags)
DASHBOARD_PAYLOAD=$(jq '{
    dashboard: .,
    folderId: 0,
    overwrite: true
}' "$DASHBOARD_FILE")

# Upload dashboard
RESPONSE=$(curl -s -X POST \
    -H "Content-Type: application/json" \
    -d "$DASHBOARD_PAYLOAD" \
    -u "$GRAFANA_USER:$GRAFANA_PASSWORD" \
    "$GRAFANA_URL/api/dashboards/db")

# Check response
if echo "$RESPONSE" | grep -q '"status":"success"'; then
    DASHBOARD_ID=$(echo "$RESPONSE" | jq '.id')
    DASHBOARD_UID=$(echo "$RESPONSE" | jq -r '.uid')
    echo "✅ Dashboard deployed successfully!"
    echo ""
    echo "📊 Dashboard Details:"
    echo "   ID:  $DASHBOARD_ID"
    echo "   UID: $DASHBOARD_UID"
    echo "   URL: $GRAFANA_URL/d/$DASHBOARD_UID/s1-insurance-sprint-progress"
    echo ""
elif echo "$RESPONSE" | grep -q '"status":"success"'; then
    echo "✅ Dashboard updated successfully!"
    DASHBOARD_UID=$(echo "$RESPONSE" | jq -r '.uid')
    echo ""
    echo "📊 Dashboard URL:"
    echo "   $GRAFANA_URL/d/$DASHBOARD_UID/s1-insurance-sprint-progress"
    echo ""
else
    echo "❌ Error uploading dashboard:"
    echo "$RESPONSE" | jq '.'
    exit 1
fi

# Wait for dashboard to be ready
echo "⏳ Waiting for dashboard to be ready..."
sleep 2

# Test dashboard is accessible
if curl -s -u "$GRAFANA_USER:$GRAFANA_PASSWORD" \
    "$GRAFANA_URL/api/dashboards/uid/s1-insurance-progress" > /dev/null; then
    echo "✅ Dashboard is ready!"
else
    echo "⚠️  Dashboard uploaded but not yet accessible. Wait a moment and refresh."
fi

echo ""
echo "🎉 S1 Dashboard deployment complete!"
echo ""
echo "📊 Access your dashboard:"
echo "   URL: $GRAFANA_URL/d/s1-insurance-progress/s1-insurance-sprint-progress"
echo "   Credentials: $GRAFANA_USER / $GRAFANA_PASSWORD"
echo ""
echo "📝 Next steps:"
echo "   1. Verify all 6 panels are visible"
echo "   2. Metrics will start flowing once extraction begins"
echo "   3. Refresh interval is set to 30 seconds"
echo ""

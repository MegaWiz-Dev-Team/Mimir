kubectl port-forward -n asgard deployment/mimir-api 8000:8000 > /dev/null 2>&1 &
PF_PID=$!
sleep 4
response=$(curl -s -w "\nHTTP_STATUS:%{http_code}" http://127.0.0.1:8000/api/v1/sources -H "X-Tenant-Id: megacare")
echo "DEBUG OUTPUT: $response"
kill $PF_PID
wait $PF_PID 2>/dev/null

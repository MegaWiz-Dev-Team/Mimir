import urllib.request
import json
import time

API_BASE = "http://localhost:30000/api/v1"

print("Fetching models catalog...")
req = urllib.request.Request(f"{API_BASE}/models")
models = []
try:
    with urllib.request.urlopen(req) as response:
        data = json.loads(response.read().decode('utf-8'))
        models = data.get("data", [])
except Exception as e:
    print(f"Error fetching catalog: {e}")

print(f"Found {len(models)} models. Beginning test (this might take a while)...")

success = 0
failed = 0

for m in models:
    if not m.get("is_active"):
        continue
    
    model_id = m.get("id", m.get("model_id"))
    provider = m.get("provider")
    print(f"Testing [ {provider} ] -> {model_id} ... ", end="", flush=True)

    payload = {
        "prompt": "Say exactly: 'OK'",
        "provider": provider,
        "model_id": model_id
    }
    data_bytes = json.dumps(payload).encode('utf-8')
    post_req = urllib.request.Request(
        f"{API_BASE}/agents/generate",
        data=data_bytes,
        headers={"Content-Type": "application/json"}
    )
    
    try:
        t0 = time.time()
        with urllib.request.urlopen(post_req) as resp:
            resp_body = json.loads(resp.read().decode('utf-8'))
            t1 = time.time()
            if "draft" in resp_body:
                print(f"✅ Success ({(t1-t0)*1000:.0f}ms)")
                success += 1
            else:
                print(f"⚠️ Unexpected response: {resp_body}")
                failed += 1
    except urllib.error.HTTPError as e:
        err_msg = e.read().decode('utf-8')
        print(f"❌ Failed: HTTP {e.code} - {err_msg}")
        failed += 1
    except Exception as e:
        print(f"❌ Failed: {e}")
        failed += 1

print(f"\nTest Complete. Success: {success}, Failed: {failed}")

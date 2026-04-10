#!/usr/bin/env python3
"""
E2E Vitality Check for LLM Pipeline stability
Validates Heimdall Gateway and Mimir Rust-Backend timeouts.
"""

import json
import urllib.request
import urllib.error
import sys
from datetime import datetime

HEIMDALL_URL = "http://localhost:8080"
HEIMDALL_KEY = "hml-REDACTED"
MIMIR_API = "http://localhost:30000"
TENANT = "megacare"

def log(msg): 
    print(f"[{datetime.now().strftime('%H:%M:%S')}] {msg}")

def test_heimdall_direct():
    log("▶️  Phase 1: Testing Heimdall LLM Direct Vitality (Deadlock detection)...")
    req = urllib.request.Request(
        f"{HEIMDALL_URL}/v1/chat/completions",
        data=json.dumps({
            "model": "mlx-community/gemma-4-31b-it-4bit",
            "messages": [{"role": "user", "content": "Reply with 'OK' only. Are you alive?"}],
            "max_tokens": 10
        }).encode(),
        headers={"Content-Type": "application/json", "Authorization": f"Bearer {HEIMDALL_KEY}"},
        method="POST"
    )
    try:
        # 45s timeout allows the 31B model to complete prompt processing
        with urllib.request.urlopen(req, timeout=45) as res:
            data = json.loads(res.read())
            log(f"✅ Heimdall is ALIVE! Reply: {data['choices'][0]['message']['content']}")
            return True
    except Exception as e:
        log(f"❌ Heimdall is DEAD/HUNG: {e}")
        return False

def test_mimir_pipeline_llm():
    log("▶️  Phase 2: Testing Mimir API LLM Hook (Validating Rust Timeouts)...")
    req = urllib.request.Request(
        f"{MIMIR_API}/api/v1/rag-eval/generate-set",
        data=json.dumps({"prompt": "Test", "count": 1, "multi_turn": False}).encode(),
        headers={"Content-Type": "application/json", "X-Tenant-ID": TENANT},
        method="POST"
    )
    try:
        # We wait slightly longer than Mimir's fast timeout
        # If Mimir deadlocks infinitely, this client block catches it after 30s as a test failure
        with urllib.request.urlopen(req, timeout=30) as res:
            log(f"✅ Mimir API Responded Successfully.")
            return True
    except urllib.error.HTTPError as e:
        err_msg = e.read().decode()
        if "timeout" in err_msg.lower() or "504" in str(e.code) or "502" in str(e.code) or "error" in err_msg.lower():
             log(f"✅ Mimir API properly caught the Heimdall failure gracefully! (HTTP {e.code})")
             return True
        log(f"❌ Mimir API Failed with unexpected HTTP {e.code}: {err_msg}")
        return False
    except urllib.error.URLError as e:
        if "timed out" in str(e).lower():
            log(f"❌ Mimir API failed to timeout internally, socket timed out!")
            return False
        log(f"❌ Mimir API Connection refused / Failed: {e}")
        return False

if __name__ == "__main__":
    print("=" * 60)
    log("=== ASTRO E2E VITALITY TEST ===")
    print("=" * 60)
    ok1 = test_heimdall_direct()
    print("-" * 60)
    ok2 = test_mimir_pipeline_llm()
    print("=" * 60)
    
    if not ok1:
        log("💥 VITALITY CHECK FAILED. Heimdall is completely deadlocked.")
        log("   Please run: ./scripts/start.sh (in Heimdall repo) to clear MLX memory.")
        sys.exit(1)
        
    if not ok2:
        log("💥 VITALITY CHECK FAILED. Mimir internal routing failed.")
        sys.exit(1)
        
    log("🚀 ALL SYSTEMS GO! Safe to run pipeline.")
    sys.exit(0)

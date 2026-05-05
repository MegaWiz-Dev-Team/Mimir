#!/usr/bin/env python3
"""
Import HealthBench Professional into Mimir as a benchmark dataset.

Downloads openai/healthbench-professional from HuggingFace, converts each item
to Mimir's extended QAPair format (with rubric_items + tags), then inserts the
full dataset into eval_benchmark_datasets for the specified tenant.

Usage:
    python scripts/import_healthbench.py
    MIMIR_TENANT=mega-care python scripts/import_healthbench.py
    MIMIR_TENANT=mega-care DRY_RUN=1 python scripts/import_healthbench.py

Requirements:
    pip install datasets huggingface_hub

Environment variables (all optional, fall back to defaults):
    MIMIR_TENANT      Tenant to import into            (default: mega-care)
    MIMIR_API_URL     Mimir API base URL               (default: http://localhost:3000)
    MIMIR_API_KEY     API key / JWT for the tenant     (default: from .env)
    HF_TOKEN          HuggingFace token for private datasets
    DRY_RUN           Set to 1 to skip the DB insert and just print a summary
    OUTPUT_JSON       Path to also write a qa_dataset.json for run_eval        (optional)
"""

import json
import os
import sys
import uuid
from datetime import datetime

# ─── Config ──────────────────────────────────────────────────────────────────

TENANT_ID   = os.environ.get("MIMIR_TENANT", "mega-care")
API_URL     = os.environ.get("MIMIR_API_URL", "http://localhost:3000")
API_KEY     = os.environ.get("MIMIR_API_KEY", "")
HF_TOKEN    = os.environ.get("HF_TOKEN", None)
DRY_RUN     = os.environ.get("DRY_RUN", "0") == "1"
OUTPUT_JSON = os.environ.get("OUTPUT_JSON", "")

HF_DATASET  = "openai/healthbench-professional"
HF_SPLIT    = "test"

# ─── Helpers ─────────────────────────────────────────────────────────────────

def load_hf_dataset():
    try:
        from datasets import load_dataset
    except ImportError:
        print("❌ 'datasets' package not found. Run: pip install datasets huggingface_hub")
        sys.exit(1)

    print(f"📥 Downloading {HF_DATASET} ({HF_SPLIT} split)…")
    kwargs = {"token": HF_TOKEN} if HF_TOKEN else {}
    ds = load_dataset(HF_DATASET, split=HF_SPLIT, **kwargs)
    print(f"✅ Loaded {len(ds)} rows")
    return ds


def extract_question(conversation: list) -> str:
    """Return the last user turn as the question text."""
    for msg in reversed(conversation):
        if isinstance(msg, dict) and msg.get("role") == "user":
            content = msg.get("content", "")
            if isinstance(content, list):
                # Multi-part content — join text parts
                return " ".join(
                    part.get("text", "") for part in content if isinstance(part, dict)
                ).strip()
            return str(content).strip()
    # Fallback: join all turns
    return " | ".join(
        str(m.get("content", "")) for m in conversation if isinstance(m, dict)
    )


def convert_row(row: dict) -> dict:
    """Convert a single HealthBench row to Mimir's extended QAPair format."""
    conversation = row.get("conversation", [])
    question = extract_question(conversation)

    # Reference answer is the physician response
    answer = row.get("physician_response", "")

    # Rubric items — preserve original points (may be negative for safety criteria)
    raw_rubric = row.get("rubric_items", []) or []
    rubric_items = []
    for item in raw_rubric:
        if isinstance(item, dict):
            rubric_items.append({
                "criterion_text": item.get("criterion_text", ""),
                "points": int(item.get("points", 0)),
            })

    return {
        "question":     question,
        "answer":       answer,
        "specialty":    row.get("specialty", None),
        "use_case":     row.get("use_case", None),
        "difficulty":   row.get("difficulty", None),
        "eval_type":    row.get("type", None),      # good_faith | red_teaming
        "rubric_items": rubric_items if rubric_items else None,
        # Store full conversation for reference (not used by run_eval)
        "_conversation": conversation,
        "_source_id":    row.get("id", None),
    }


def insert_via_api(dataset_id: str, name: str, items: list, total: int):
    """POST directly to the Mimir API (requires a running server)."""
    import urllib.request
    import urllib.error

    payload = {
        "id":          dataset_id,
        "name":        name,
        "source":      "healthbench_professional",
        "description": "HealthBench Professional (OpenAI) — 525 clinical evaluation cases across 28 specialties",
        "items":       items,
        "total_items": total,
        "is_active":   True,
    }

    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        f"{API_URL}/api/v1/eval/benchmark-datasets",
        data=data,
        headers={
            "Content-Type":  "application/json",
            "X-Tenant-ID":   TENANT_ID,
            "Authorization": f"Bearer {API_KEY}",
        },
        method="POST",
    )

    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            body = json.loads(resp.read())
            print(f"✅ Inserted via API: {body}")
    except urllib.error.HTTPError as e:
        print(f"❌ API error {e.code}: {e.read().decode()}")
        sys.exit(1)


def insert_via_sql(dataset_id: str, name: str, items: list, total: int):
    """Print a SQL INSERT statement (for manual execution or pipe to mysql)."""
    items_json = json.dumps(items, ensure_ascii=False).replace("'", "''")
    now = datetime.utcnow().strftime("%Y-%m-%d %H:%M:%S")
    sql = f"""
INSERT INTO eval_benchmark_datasets
    (id, tenant_id, name, source, description, items, total_items, version, is_active, created_at, updated_at)
VALUES (
    '{dataset_id}',
    '{TENANT_ID}',
    '{name}',
    'healthbench_professional',
    'HealthBench Professional (OpenAI) — 525 clinical evaluation cases across 28 specialties',
    '{items_json}',
    {total},
    1,
    1,
    '{now}',
    '{now}'
);
""".strip()
    print("\n" + "─" * 60)
    print("SQL (pipe to: mysql -u root -p mimir_db):")
    print("─" * 60)
    print(sql[:500] + " …[truncated]" if len(sql) > 500 else sql)
    print("─" * 60)

    sql_path = f"/tmp/healthbench_import_{TENANT_ID}.sql"
    with open(sql_path, "w", encoding="utf-8") as f:
        f.write(sql)
    print(f"✅ Full SQL written to {sql_path}")


# ─── Main ─────────────────────────────────────────────────────────────────────

def main():
    print("=" * 60)
    print("🏥 HealthBench Professional → Mimir Importer")
    print(f"   Tenant : {TENANT_ID}")
    print(f"   API    : {API_URL}")
    print(f"   DryRun : {DRY_RUN}")
    print("=" * 60)

    ds = load_hf_dataset()

    print("🔄 Converting rows…")
    items = [convert_row(dict(row)) for row in ds]
    total = len(items)

    # Stats
    use_cases  = {}
    specialties = {}
    types      = {}
    unsafe_count = 0

    for item in items:
        use_cases[item.get("use_case") or "unknown"]     = use_cases.get(item.get("use_case") or "unknown", 0) + 1
        specialties[item.get("specialty") or "unknown"]  = specialties.get(item.get("specialty") or "unknown", 0) + 1
        types[item.get("eval_type") or "unknown"]        = types.get(item.get("eval_type") or "unknown", 0) + 1
        if item.get("rubric_items"):
            has_negative = any(r["points"] < 0 for r in item["rubric_items"])
            if has_negative:
                unsafe_count += 1

    print(f"\n📊 Dataset summary ({total} items):")
    print(f"   Use cases   : {dict(sorted(use_cases.items(), key=lambda x: -x[1]))}")
    print(f"   Eval types  : {types}")
    print(f"   Specialties : {len(specialties)} unique")
    print(f"   Items with safety (negative) rubric criteria: {unsafe_count}")

    # Optionally write qa_dataset.json for run_eval
    if OUTPUT_JSON:
        qa_pairs = [{"question": i["question"], "answer": i["answer"],
                     "specialty": i.get("specialty"), "use_case": i.get("use_case"),
                     "difficulty": i.get("difficulty"), "eval_type": i.get("eval_type"),
                     "rubric_items": i.get("rubric_items")} for i in items]
        with open(OUTPUT_JSON, "w", encoding="utf-8") as f:
            json.dump(qa_pairs, f, ensure_ascii=False, indent=2)
        print(f"\n📁 qa_dataset.json written to {OUTPUT_JSON}")

    if DRY_RUN:
        print("\n🔍 DRY RUN — skipping insert")
        print(f"   Would insert {total} items for tenant '{TENANT_ID}'")
        return

    dataset_id = str(uuid.uuid4())
    name = f"HealthBench Professional ({datetime.utcnow().strftime('%Y-%m-%d')})"

    print(f"\n⬆️  Inserting dataset id={dataset_id}…")

    # Try API first, fall back to SQL file
    if API_KEY:
        insert_via_api(dataset_id, name, items, total)
    else:
        print("ℹ️  No MIMIR_API_KEY — generating SQL file instead")
        insert_via_sql(dataset_id, name, items, total)

    print(f"\n✅ Done! Dataset '{name}' ready for tenant '{TENANT_ID}'")
    print("   Next: POST /api/v1/eval/runs with benchmark_dataset_id to run evaluation")


if __name__ == "__main__":
    main()

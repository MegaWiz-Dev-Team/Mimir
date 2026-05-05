#!/usr/bin/env python3
"""Load 5 downloaded medical benchmarks into Mimir's eval_benchmark_datasets.

Usage:
    /opt/homebrew/opt/python@3.14/bin/python3.14 \
        scripts/load_medical_benchmarks_to_db.py \
        [--limit 200] [--tenant_id __global__] [--dry-run]

Reads from /benchmarks/medical/<name>/, normalizes each benchmark's items into
Mimir's standard schema, then INSERTs one row per benchmark with the items as
inline JSON.

Mimir schema (eval_benchmark_datasets):
    id            varchar(36)     — slug like 'med-medqa-v1'
    tenant_id     varchar(50)     — '__global__' for shared
    name          varchar(255)    — display name
    source        varchar(100)    — taxonomy: medqa | medmcqa | pubmedqa | healthbench | medxpertqa
    scoring_fn    varchar(32)     — mcq_accuracy | binary_yes_no | healthbench_likert | paper_rubric_pct
    items         longtext (JSON) — normalized items array
    total_items   int             — len(items)
    is_active     tinyint         — 1
    description   text            — paper + license

Normalized item schema:
    {
        "id": "<benchmark>_<orig_id>",
        "question": "..." | conversation_list,
        "options": {"A":"...", ...} | null,             # for MCQ
        "ground_truth": "A" | "yes" | None,             # for MCQ/binary
        "context": "..." | None,                         # for PubMedQA
        "rubrics": [...] | None,                         # for HealthBench
        "metadata": {"specialty": ..., "subject": ..., ...}
    }

Idempotent: skips datasets already present (by id) unless --force.
"""
import argparse
import json
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path
from typing import Iterable

ROOT = Path("/Users/mimir/Developer/Mimir/benchmarks/medical")

# ─── Normalizers — one per benchmark, returns iterator of dict items ─────────

def load_medqa(limit: int | None) -> Iterable[dict]:
    p = ROOT / "medqa/data_clean/questions/US/test.jsonl"
    if not p.exists():
        # fallback: unzip if needed
        zp = ROOT / "medqa/data_clean.zip"
        if zp.exists():
            with zipfile.ZipFile(zp) as z:
                z.extractall(ROOT / "medqa")
    for i, line in enumerate(open(p)):
        if limit and i >= limit: break
        ex = json.loads(line)
        gt_letter = next((k for k, v in ex["options"].items() if v == ex["answer"]), None)
        # Format question with options inline so the agent sees the choices.
        opts = "\n".join(f"({k}) {v}" for k, v in ex["options"].items())
        yield {
            # runner.rs expects `_source_id`, `question` (str), `answer` (str)
            "_source_id": f"medqa_{i}",
            "question": f"{ex['question']}\n\nChoices:\n{opts}",
            "answer": f"{gt_letter}: {ex['answer']}" if gt_letter else ex["answer"],
            # Auxiliary fields (kept for richer scoring/UI later)
            "options": ex["options"],
            "ground_truth": gt_letter,
            "metadata": {"answer_text": ex["answer"]},
        }


def load_medmcqa(limit: int | None) -> Iterable[dict]:
    import pyarrow.parquet as pq
    # Use validation split — has labels (test split has cop=-1)
    t = pq.read_table(ROOT / "medmcqa/data/validation-00000-of-00001.parquet")
    n = min(t.num_rows, limit) if limit else t.num_rows
    cop_to_letter = {0: "A", 1: "B", 2: "C", 3: "D"}
    for i in range(n):
        row = {c: t.column(c)[i].as_py() for c in t.column_names}
        gt_letter = cop_to_letter.get(row.get("cop"), None)
        opts_map = {"A": row["opa"], "B": row["opb"], "C": row["opc"], "D": row["opd"]}
        opts = "\n".join(f"({k}) {v}" for k, v in opts_map.items())
        yield {
            "_source_id": f"medmcqa_{row['id']}",
            "question": f"{row['question']}\n\nChoices:\n{opts}",
            "answer": f"{gt_letter}: {opts_map.get(gt_letter,'')}" if gt_letter else "",
            "options": opts_map,
            "ground_truth": gt_letter,
            "metadata": {"subject": row.get("subject_name"), "topic": row.get("topic_name"),
                         "explanation": row.get("exp")},
        }


def load_pubmedqa(limit: int | None) -> Iterable[dict]:
    import pyarrow.parquet as pq
    t = pq.read_table(ROOT / "pubmedqa/pqa_labeled/train-00000-of-00001.parquet")
    n = min(t.num_rows, limit) if limit else t.num_rows
    for i in range(n):
        row = {c: t.column(c)[i].as_py() for c in t.column_names}
        # context is a dict {"contexts": [...], "labels": [...]}
        ctx = row["context"]
        if isinstance(ctx, dict):
            ctx_text = "\n\n".join(ctx.get("contexts", []))
        else:
            ctx_text = str(ctx)
        yield {
            "_source_id": f"pubmedqa_{row['pubid']}",
            "question": (
                f"Context (from PubMed abstract):\n{ctx_text}\n\n"
                f"Question: {row['question']}\n"
                f"Answer with one of: yes / no / maybe."
            ),
            "answer": row["final_decision"],
            "options": {"yes": "yes", "no": "no", "maybe": "maybe"},
            "ground_truth": row["final_decision"],
            "metadata": {"long_answer": row.get("long_answer"), "pubid": row["pubid"]},
        }


def load_healthbench(limit: int | None, variant: str = "oss_eval") -> Iterable[dict]:
    p = ROOT / f"healthbench/{variant}.jsonl"
    for i, line in enumerate(open(p)):
        if limit and i >= limit: break
        ex = json.loads(line)
        # Multi-turn prompt → flatten into single question string. Use the
        # last user turn's content as the primary question; preceding turns
        # become "Conversation context:" prefix (rare for healthbench oss_eval
        # but supported).
        prompt_list = ex.get("prompt") or []
        if isinstance(prompt_list, list) and prompt_list:
            user_turns = [t for t in prompt_list if t.get("role") == "user"]
            primary = user_turns[-1].get("content", "") if user_turns else ""
            ctx_turns = prompt_list[:-1] if len(prompt_list) > 1 else []
            if ctx_turns:
                ctx = "\n".join(f"[{t.get('role')}] {t.get('content','')}" for t in ctx_turns)
                question_str = f"Conversation context:\n{ctx}\n\nUser: {primary}"
            else:
                question_str = primary
        else:
            question_str = str(prompt_list)
        # No single "right answer" — the rubric defines partial credit
        yield {
            "_source_id": f"healthbench_{ex.get('prompt_id', i)}",
            "question": question_str,
            "answer": "(scored against rubric criteria — see metadata.rubrics)",
            "rubrics": ex.get("rubrics", []),
            "metadata": {"tags": ex.get("example_tags", []),
                         "rubrics": ex.get("rubrics", [])},
        }


def load_medxpertqa(limit: int | None) -> Iterable[dict]:
    # Use Text/test (skip MM — needs vision)
    p = ROOT / "medxpertqa/Text/test.jsonl"
    for i, line in enumerate(open(p)):
        if limit and i >= limit: break
        ex = json.loads(line)
        # MedXpertQA already includes "Answer Choices: ..." in question text
        # → no need to re-format options. Just hand it to the agent as-is.
        yield {
            "_source_id": f"medxpertqa_{ex['id']}",
            "question": ex["question"],
            "answer": ex.get("label", ""),
            "options": ex.get("options", {}),
            "ground_truth": ex.get("label"),
            "metadata": {
                "medical_task": ex.get("medical_task"),
                "body_system": ex.get("body_system"),
                "question_type": ex.get("question_type"),
            },
        }


# ─── Dataset registry ────────────────────────────────────────────────────────

DATASETS = [
    {
        "id": "med-medqa-v1",
        "name": "MedQA (USMLE, English)",
        "source": "medqa",
        "scoring_fn": "mcq_accuracy",
        "loader": load_medqa,
        "description": "USMLE-style 4-5 choice MCQ (Jin 2020, arXiv:2009.13081). MIT licence (bigbio/med_qa).",
    },
    {
        "id": "med-medmcqa-v1",
        "name": "MedMCQA (Indian AIIMS/NEET)",
        "source": "medmcqa",
        "scoring_fn": "mcq_accuracy",
        "loader": load_medmcqa,
        "description": "Indian medical entrance MCQ (Pal 2022, arXiv:2203.14371). MIT licence. Using `validation` split (has labels).",
    },
    {
        "id": "med-pubmedqa-v1",
        "name": "PubMedQA (labeled)",
        "source": "pubmedqa",
        "scoring_fn": "binary_yes_no",
        "loader": load_pubmedqa,
        "description": "Y/N/Maybe over PubMed abstract (Jin 2019, arXiv:1909.06146). MIT licence. Tests RAG comprehension.",
    },
    {
        "id": "med-healthbench-v1",
        "name": "HealthBench (paper-original)",
        "source": "healthbench",
        "scoring_fn": "paper_rubric_pct",
        "loader": lambda lim: load_healthbench(lim, "oss_eval"),
        "description": "OpenAI HealthBench multi-turn convs + physician rubrics (Arora 2025, arXiv:2505.08775). MIT licence.",
    },
    {
        "id": "med-healthbench-hard-v1",
        "name": "HealthBench Hard",
        "source": "healthbench",
        "scoring_fn": "paper_rubric_pct",
        "loader": lambda lim: load_healthbench(lim, "hard"),
        "description": "Frontier-model-discriminating subset of HealthBench (1000 items).",
    },
    {
        "id": "med-medxpertqa-v1",
        "name": "MedXpertQA (Text)",
        "source": "medxpertqa",
        "scoring_fn": "mcq_accuracy",
        "loader": load_medxpertqa,
        "description": "Expert-level reasoning MCQ (TsinghuaC3I 2025). Text track only — MM needs vision.",
    },
]


def existing_ids() -> set[str]:
    """Query DB for already-loaded benchmark dataset IDs."""
    out = subprocess.check_output(
        ["kubectl", "exec", "-n", "asgard-infra", "mariadb-fb55894c5-xjjvb", "--",
         "mariadb", "-u", "root", "-proot", "-D", "mimir", "-N", "-e",
         "SELECT id FROM eval_benchmark_datasets;"],
        text=True, timeout=10)
    return {line.strip() for line in out.splitlines() if line.strip()}


def insert_dataset(ds: dict, items: list[dict], tenant_id: str, dry_run: bool):
    """INSERT one row into eval_benchmark_datasets via kubectl exec mariadb."""
    items_json = json.dumps(items)
    n = len(items)
    print(f"   ↳ inserting {n} items ({len(items_json)/1024/1024:.1f} MB JSON)")

    if dry_run:
        print(f"   ⊕ DRY RUN — would INSERT {ds['id']}")
        # show first item preview
        print(f"   sample item[0]: {json.dumps(items[0], default=str)[:200]}")
        return

    # Use temp file + LOAD DATA equivalent via kubectl cp + INSERT from file
    # Simpler: use a heredoc with escaped JSON. Items can be huge so write SQL file.
    sql_path = Path(tempfile.gettempdir()) / f"insert_{ds['id']}.sql"
    # MariaDB-safe: escape single quotes in JSON by doubling them
    safe_json = items_json.replace("\\", "\\\\").replace("'", "\\'")
    safe_desc = ds['description'].replace("'", "\\'")
    sql = (
        f"INSERT INTO eval_benchmark_datasets "
        f"(id, tenant_id, name, source, scoring_fn, description, items, total_items, version, is_active) "
        f"VALUES ('{ds['id']}', '{tenant_id}', '{ds['name']}', '{ds['source']}', "
        f"'{ds['scoring_fn']}', '{safe_desc}', '{safe_json}', {n}, 1, 1);\n"
    )
    sql_path.write_text(sql)
    # kubectl cp + exec
    cp = subprocess.run(
        ["kubectl", "cp", str(sql_path),
         f"asgard-infra/mariadb-fb55894c5-xjjvb:/tmp/{sql_path.name}"],
        capture_output=True, text=True, timeout=120)
    if cp.returncode != 0:
        print(f"   ❌ kubectl cp failed: {cp.stderr[:300]}")
        return
    exe = subprocess.run(
        ["kubectl", "exec", "-n", "asgard-infra", "mariadb-fb55894c5-xjjvb", "--",
         "bash", "-c", f"mariadb -u root -proot -D mimir < /tmp/{sql_path.name}"],
        capture_output=True, text=True, timeout=120)
    if exe.returncode != 0:
        print(f"   ❌ INSERT failed: {exe.stderr[:300]}")
    else:
        print(f"   ✓ INSERTED {ds['id']}")


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--limit", type=int, default=None,
                   help="Max items per dataset (default: all). Use small N for first load.")
    p.add_argument("--tenant_id", default="__global__")
    p.add_argument("--dry-run", action="store_true")
    p.add_argument("--force", action="store_true",
                   help="Re-insert even if dataset id already exists (will fail unless DELETE first)")
    p.add_argument("--only", default="",
                   help="Comma-separated dataset ids to load (default: all)")
    args = p.parse_args()

    print("═" * 64)
    print(f" 📚 Loading medical benchmarks into Mimir DB")
    print(f"    tenant: {args.tenant_id}  limit: {args.limit or 'all'}")
    print("═" * 64)

    only = {s.strip() for s in args.only.split(",") if s.strip()}
    targets = [d for d in DATASETS if not only or d["id"] in only]

    print(f"\n→ Checking existing IDs in DB...")
    have = existing_ids() if not args.dry_run else set()
    print(f"   already present: {sorted(have)}")

    for ds in targets:
        print(f"\n📦 {ds['id']}  ({ds['source']}, scoring={ds['scoring_fn']})")
        if ds["id"] in have and not args.force:
            print(f"   ⊕ skip (already in DB; --force to overwrite)")
            continue

        print(f"   ↳ loading items from disk...")
        try:
            items = list(ds["loader"](args.limit))
        except FileNotFoundError as e:
            print(f"   ❌ source not downloaded: {e}")
            continue
        except Exception as e:
            print(f"   ❌ loader error: {type(e).__name__}: {e}")
            continue

        if not items:
            print(f"   ⚠️  loader returned 0 items — skipping")
            continue

        insert_dataset(ds, items, args.tenant_id, args.dry_run)

    print("\n✓ done")


if __name__ == "__main__":
    sys.exit(main() or 0)

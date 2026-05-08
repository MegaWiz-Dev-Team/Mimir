#!/usr/bin/env python3
"""Sprint 51c step 5 — backfill standalone HBp bench reports into Mimir's
eval_runs + eval_scores tables.

bench_typhoon_si_med_hbp.py writes JSON files only; this script reads
those files and INSERTs the equivalent rows into MariaDB so the existing
dashboard `/evaluations/diagnose` page can render them, and so cross-run
SQL analysis works without grep'ing JSON.

Idempotent: each report carries a deterministic run_id derived from
SHA-256(filename); rerunning the script just refuses to clobber existing
rows.

Usage:
    python3 backfill_bench_to_db.py                    # backfill everything under reports/
    python3 backfill_bench_to_db.py --report <path>    # one specific JSON
    python3 backfill_bench_to_db.py --dry-run          # show what would be inserted
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

REPORTS_DIR = Path(
    "/Users/mimir/Developer/Mimir/docs/04_evaluation_and_testing/reports"
)
TENANT_ID = "asgard_medical"


def stable_run_id(report_path: Path) -> str:
    """Deterministic UUID-shaped id from filename so reruns are idempotent."""
    h = hashlib.sha256(report_path.name.encode()).hexdigest()
    return f"{h[:8]}-{h[8:12]}-{h[12:16]}-{h[16:20]}-{h[20:32]}"


# Map bench script's --label values to canonical ai_models.model_id
# (foreign-key target). Add a row when introducing a new label.
LABEL_TO_MODEL_ID = {
    "gemma-4-26b-a4b-it-4bit": "mlx-community/gemma-4-26b-a4b-it-4bit",
    "typhoon-si-med-thinking-4b": "typhoon-si-med-thinking-4b-mlx-4bit",
    "unknown": "typhoon-si-med-thinking-4b-mlx-4bit",  # legacy Day-2 file
}


def derive_run_metadata(report_path: Path, summary: dict) -> dict:
    """Pull dataset / anchor / judge-config + label from the report filename
    + summary fields populated by bench_typhoon_si_med_hbp.py."""
    name = report_path.stem
    # filename pattern from bench script:
    #   hbp-<label>-<run8>-n<N>-<judgeStrict|judgeThink>-<UTC>
    # also handles legacy "typhoon-si-med-hbp-<UTC>" (Day-2 first run).
    label = summary.get("label") or "unknown"
    baseline_run = summary.get("baseline_run") or "unknown"
    judge_mode = "judgeThink" if summary.get("judge_thinking") else "judgeStrict"
    anchor = "locked-20" if baseline_run.startswith("195e8912") else (
        "broader-100" if baseline_run.startswith("f2eeb239") else "unknown"
    )
    # Legacy default Day-2 file name parser:
    if "typhoon-si-med-hbp-" in name and label == "unknown":
        label = "typhoon-si-med-thinking-4b"
        anchor = "locked-20"
    return {
        "label": label,
        "anchor": anchor,
        "baseline_run": baseline_run,
        "judge_mode": judge_mode,
    }


def kubectl_mariadb(sql: str, dry_run: bool) -> None:
    """Run a SQL block in the asgard-infra MariaDB pod via kubectl exec -i."""
    if dry_run:
        # Truncate long SQL for readability
        excerpt = sql.strip()
        if len(excerpt) > 500:
            excerpt = excerpt[:500] + " …"
        print(f"  [dry-run] SQL:\n{excerpt}\n")
        return
    proc = subprocess.run(
        [
            "kubectl", "exec", "-i", "-n", "asgard-infra", "deploy/mariadb",
            "--", "mariadb", "-u", "mimir", "-pREDACTED-PW", "mimir",
        ],
        input=sql, capture_output=True, text=True, check=False,
    )
    if proc.returncode != 0:
        print(f"  [SQL ERROR] {proc.stderr}", file=sys.stderr)
        raise RuntimeError(proc.stderr)
    if proc.stdout.strip():
        print(f"  [SQL OK] {proc.stdout.strip()[:200]}")


def already_exists(run_id: str) -> bool:
    proc = subprocess.run(
        [
            "kubectl", "exec", "-n", "asgard-infra", "deploy/mariadb",
            "--", "mariadb", "-u", "mimir", "-pREDACTED-PW", "mimir",
            "--batch", "--silent",
            "-e", f"SELECT id FROM eval_runs WHERE id='{run_id}';",
        ],
        capture_output=True, text=True, check=False,
    )
    return run_id in proc.stdout


def sql_escape(s) -> str:
    """Single-quote-safe SQL string literal. NULL when None."""
    if s is None:
        return "NULL"
    s = str(s).replace("\\", "\\\\").replace("'", "\\'")
    return f"'{s}'"


def backfill(report_path: Path, dry_run: bool) -> None:
    print(f"\n=== {report_path.name}")
    data = json.loads(report_path.read_text())
    summary = data.get("summary") or {}
    rows = data.get("rows") or []
    if not rows:
        print("  (empty rows — skipped)")
        return

    run_id = stable_run_id(report_path)
    meta = derive_run_metadata(report_path, summary)

    if not dry_run and already_exists(run_id):
        print(f"  [skip] run_id {run_id} already in eval_runs")
        return

    valid = [r for r in rows if r.get("accuracy") is not None]
    n_valid = len(valid)
    n_total = len(rows)
    started_at = re.search(r"(\d{8}T\d{6}Z)", report_path.name)
    started_iso = (
        datetime.strptime(started_at.group(1), "%Y%m%dT%H%M%SZ")
        .replace(tzinfo=timezone.utc)
        .strftime("%Y-%m-%d %H:%M:%S")
        if started_at else "2026-05-07 00:00:00"
    )

    name = f"sprint51b-{meta['label']}-{meta['anchor']}-n{n_total}-{meta['judge_mode']}"
    config_json = json.dumps({
        "source": "bench_typhoon_si_med_hbp.py",
        "anchor": meta["anchor"],
        "baseline_run": meta["baseline_run"],
        "judge_mode": meta["judge_mode"],
        "summary_hbp_percent": summary.get("hbp_percent"),
        "wall_seconds": summary.get("wall_seconds"),
        "valid_rows": n_valid,
        "total_rows": n_total,
        "report_file": report_path.name,
    }).replace("'", "\\'")

    # eval_runs INSERT
    sql_run = f"""
INSERT INTO eval_runs (
    id, name, status, total_combinations, completed_combinations,
    started_at, finished_at, config, tenant_id, total_cost_usd
) VALUES (
    '{run_id}',
    {sql_escape(name)},
    'COMPLETED',
    {n_total}, {n_valid},
    '{started_iso}', '{started_iso}',
    '{config_json}',
    '{TENANT_ID}',
    NULL
);
"""
    kubectl_mariadb(sql_run, dry_run)

    # eval_scores INSERTs in chunks of 25 to keep statements small
    score_inserts = []
    for r in valid:
        actual = r.get("answer") or ""
        question = r.get("question_preview") or ""
        # The bench script truncates question preview to ~120 chars; the
        # full question is in the source eval_scores via baseline_run,
        # but we want the data here to be self-contained. Since rows have
        # 'question_preview' but not full text, we'll log preview as is.
        expected = ""  # we didn't store expected_answer in JSON; backfill empty
        reasoning = r.get("reasoning") or ""
        rubric_items = json.dumps({
            "engine": meta["label"],
            "anchor": meta["anchor"],
            "judge_mode": meta["judge_mode"],
            "gen_seconds": r.get("gen_seconds"),
            "reasoning_len": r.get("reasoning_len"),
        })
        canonical_model_id = LABEL_TO_MODEL_ID.get(meta["label"], meta["label"])
        score_inserts.append(
            "(" + ",".join([
                f"'{run_id}'",
                sql_escape(meta["label"]),  # agent_name (free-form)
                sql_escape(canonical_model_id),  # model_id (FK → ai_models)
                sql_escape(question),
                sql_escape(expected),
                sql_escape(actual),
                str(r.get("accuracy") or "NULL"),
                str(r.get("completeness") or "NULL"),
                str(r.get("relevance") or "NULL"),
                str(int((r.get("gen_seconds") or 0) * 1000)),  # latency_ms
                "'gemini-2.5-flash'",
                sql_escape(reasoning),
                f"'{TENANT_ID}'",
                str(r.get("safety") if r.get("safety") is not None else "NULL"),
                sql_escape(rubric_items),
            ]) + ")"
        )

    BATCH = 20
    for i in range(0, len(score_inserts), BATCH):
        batch = score_inserts[i:i + BATCH]
        sql = (
            "INSERT INTO eval_scores ("
            "run_id, agent_name, model_id, question, expected_answer, actual_answer, "
            "accuracy_score, completeness_score, relevance_score, latency_ms, "
            "judge_model, judge_reasoning, tenant_id, safety_score, rubric_items"
            ") VALUES " + ",\n".join(batch) + ";"
        )
        kubectl_mariadb(sql, dry_run)

    print(f"  [done] run_id={run_id} · {n_valid} score rows inserted")


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--report", type=str, default=None)
    p.add_argument("--dry-run", action="store_true")
    args = p.parse_args()

    if args.report:
        backfill(Path(args.report), args.dry_run)
    else:
        # Backfill every JSON report under reports/, skipping the Day-2 first-attempt
        # n=56 partial run.
        for path in sorted(REPORTS_DIR.glob("*.json")):
            if "162324Z" in path.name:
                print(f"  [skip] {path.name} — first failed attempt (n=56 partial)")
                continue
            backfill(path, args.dry_run)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

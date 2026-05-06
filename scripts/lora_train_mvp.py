#!/usr/bin/env python3
"""
Sprint 39 Phase 2 MVP — fine-tune gemma-4-26b on Curator corpus via mlx_lm.lora.

Workflow:
  1. Pull approved JSONL from Curator dataset
  2. Split 90/10 train/valid into a temp directory ({train,valid}.jsonl)
  3. Register run in Mimir lora_training_runs (gets run_id)
  4. Subprocess `mlx_lm lora --train` with sane MVP hyperparams
  5. (manual next: mlx_lm fuse → eval)

Run via Heimdall's Python venv (only one with mlx_lm installed):
    /Users/mimir/Developer/Heimdall/.venv/bin/python lora_train_mvp.py \\
        --dataset-id 8f5f524e-... [--iters 200]

Cost: $0 (local MLX, M3 Max electric).
Time: ~30-90 min depending on iters and dataset size.
"""

from __future__ import annotations
import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

import requests

MIMIR_API = os.environ.get("MIMIR_API_URL", "http://localhost:30000/api/v1")
TENANT = os.environ.get("MIMIR_TENANT", "asgard_medical")
HEIMDALL_VENV = "/Users/mimir/Developer/Heimdall/.venv/bin/python"
DEFAULT_BASE_MODEL = "mlx-community/gemma-4-26b-a4b-it-4bit"
DEFAULT_MIN_PAIRS = 50  # MVP minimum corpus size


def pull_approved_jsonl(dataset_id: str, out_path: Path) -> int:
    """Pull approved items as JSONL via Curator export. Returns count."""
    headers = {"X-Tenant-Id": TENANT}
    url = f"{MIMIR_API}/training/datasets/{dataset_id}/export.jsonl"
    r = requests.get(url, headers=headers, timeout=30)
    r.raise_for_status()
    lines = [ln for ln in r.text.split("\n") if ln.strip()]
    out_path.write_text("\n".join(lines) + ("\n" if lines else ""))
    return len(lines)


def split_train_valid(src: Path, out_dir: Path, valid_frac: float = 0.1):
    """Split JSONL into train.jsonl + valid.jsonl in out_dir.

    mlx_lm lora wants a directory with these specific filenames.
    """
    lines = [ln for ln in src.read_text().split("\n") if ln.strip()]
    n_valid = max(1, int(len(lines) * valid_frac))
    valid = lines[-n_valid:]
    train = lines[:-n_valid] if n_valid > 0 else lines
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "train.jsonl").write_text("\n".join(train) + "\n")
    (out_dir / "valid.jsonl").write_text("\n".join(valid) + "\n")
    return len(train), len(valid)


def register_run(
    dataset_id: str,
    base_model: str,
    hyperparams: dict,
    notes: str,
) -> str | None:
    """Register a lora_training_runs row. Returns run_id or None on failure."""
    headers = {"Content-Type": "application/json", "X-Tenant-Id": TENANT}
    payload = {
        "name": f"mvp-lora-{int(time.time())}",
        "dataset_id": dataset_id,
        "base_model_id": base_model,
        "hyperparams": hyperparams,
        "notes": notes,
    }
    try:
        r = requests.post(f"{MIMIR_API}/training/runs", json=payload, headers=headers, timeout=10)
        r.raise_for_status()
        return r.json()["id"]
    except Exception as e:
        print(f"  ⚠️  register_run failed (continuing without DB tracking): {e}", file=sys.stderr)
        return None


def patch_run(run_id: str, **kwargs):
    """PATCH lora_training_runs row (status, adapter_path, finished_at, ...)."""
    if not run_id:
        return
    headers = {"Content-Type": "application/json", "X-Tenant-Id": TENANT}
    try:
        requests.patch(
            f"{MIMIR_API}/training/runs/{run_id}",
            json=kwargs,
            headers=headers,
            timeout=10,
        )
    except Exception as e:
        print(f"  ⚠️  patch_run({run_id}) failed: {e}", file=sys.stderr)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dataset-id", required=True, help="Curator dataset ID with approved items")
    ap.add_argument("--base-model", default=DEFAULT_BASE_MODEL)
    ap.add_argument("--adapter-out", default="/tmp/lora_mvp_adapter", help="Output dir for adapter")
    ap.add_argument("--iters", type=int, default=200, help="LoRA training iterations (MVP=200)")
    ap.add_argument("--batch-size", type=int, default=2)
    ap.add_argument("--learning-rate", type=float, default=1e-4)
    ap.add_argument("--num-layers", type=int, default=8, help="LoRA target N attention layers")
    ap.add_argument("--max-seq-length", type=int, default=2048)
    ap.add_argument("--min-pairs", type=int, default=DEFAULT_MIN_PAIRS)
    ap.add_argument("--dry-run", action="store_true", help="Stop before subprocess")
    args = ap.parse_args()

    print(f"╔══════════════════════════════════════════════════════════════╗")
    print(f"║ Sprint 39 Phase 2 MVP — LoRA training                        ║")
    print(f"╚══════════════════════════════════════════════════════════════╝")
    print(f"Dataset:       {args.dataset_id}")
    print(f"Base model:    {args.base_model}")
    print(f"Adapter out:   {args.adapter_out}")
    print(f"Iters:         {args.iters}")
    print(f"Batch size:    {args.batch_size}")
    print(f"LR:            {args.learning_rate}")
    print()

    # ─── 1. Pull JSONL ────────────────────────────────────────────────────
    work_dir = Path("/tmp/lora_mvp_data")
    if work_dir.exists():
        shutil.rmtree(work_dir)
    work_dir.mkdir(parents=True)
    jsonl_path = work_dir / "approved.jsonl"
    print(f"Pulling approved JSONL from Curator…")
    n = pull_approved_jsonl(args.dataset_id, jsonl_path)
    print(f"  pulled {n} approved items")
    if n < args.min_pairs:
        print(f"❌ Below minimum {args.min_pairs} approved pairs. Approve more before training.")
        sys.exit(2)

    # ─── 2. Split into train/valid ───────────────────────────────────────
    n_train, n_valid = split_train_valid(jsonl_path, work_dir)
    print(f"  split: train={n_train} valid={n_valid}")

    # ─── 3. Register run in Mimir ────────────────────────────────────────
    hyperparams = {
        "base_model": args.base_model,
        "iters": args.iters,
        "batch_size": args.batch_size,
        "learning_rate": args.learning_rate,
        "num_layers": args.num_layers,
        "max_seq_length": args.max_seq_length,
        "fine_tune_type": "lora",
        "n_train": n_train,
        "n_valid": n_valid,
    }
    run_id = register_run(
        dataset_id=args.dataset_id,
        base_model=args.base_model,
        hyperparams=hyperparams,
        notes=f"Sprint 39 Phase 2 MVP run, {n_train} train + {n_valid} valid items",
    )
    if run_id:
        print(f"  registered run_id={run_id}")
    print()

    if args.dry_run:
        print("(dry-run: stopping before subprocess)")
        return

    # ─── 4. Run mlx_lm lora ──────────────────────────────────────────────
    adapter_dir = Path(args.adapter_out)
    adapter_dir.mkdir(parents=True, exist_ok=True)
    cmd = [
        HEIMDALL_VENV, "-m", "mlx_lm", "lora",
        "--train",
        "--model", args.base_model,
        "--data", str(work_dir),
        "--adapter-path", str(adapter_dir),
        "--iters", str(args.iters),
        "--batch-size", str(args.batch_size),
        "--learning-rate", str(args.learning_rate),
        "--num-layers", str(args.num_layers),
        "--max-seq-length", str(args.max_seq_length),
        "--steps-per-report", "10",
        "--steps-per-eval", "50",
        "--save-every", "100",
        "--grad-checkpoint",
    ]
    print(f"Running: {' '.join(cmd[:5])} ...")
    print()

    if run_id:
        patch_run(run_id, status="RUNNING")

    t0 = time.time()
    try:
        result = subprocess.run(cmd, check=True)
        elapsed = time.time() - t0
        print(f"\n✅ Training completed in {elapsed:.0f}s ({elapsed/60:.1f} min)")
        if run_id:
            patch_run(
                run_id,
                status="COMPLETED",
                adapter_path=str(adapter_dir),
                finished_at=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            )
        print(f"\nAdapter saved to: {adapter_dir}")
        print(f"\nNext step (manual):")
        print(f"  Fuse adapter into merged model:")
        print(f"    {HEIMDALL_VENV} -m mlx_lm fuse \\")
        print(f"      --model {args.base_model} \\")
        print(f"      --adapter-path {adapter_dir} \\")
        print(f"      --save-path /tmp/gemma-4-26b-eir-lora-mvp")
        print(f"\n  Then trigger eval via Mimir:")
        print(f'    POST /api/v1/eval/runs with model_ids=["/tmp/gemma-4-26b-eir-lora-mvp"]')
    except subprocess.CalledProcessError as e:
        print(f"\n❌ Training failed (exit {e.returncode})")
        if run_id:
            patch_run(run_id, status="FAILED", status_message=f"subprocess exit {e.returncode}")
        sys.exit(1)


if __name__ == "__main__":
    main()

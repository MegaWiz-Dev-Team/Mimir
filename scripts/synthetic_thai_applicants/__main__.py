"""CLI entry point for the synthetic Thai applicant generator.

Usage:
    python -m synthetic_thai_applicants \\
        --applicants 1000 --claims 500 --seed 42 \\
        --output ./out [--pdf]
"""
from __future__ import annotations
import argparse
import json
import sys
import time
from pathlib import Path

from .applicant import generate_applicants
from .medical import generate_medical_records
from .claim import generate_claims


def write_jsonl(path: Path, items: list[dict]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as f:
        for item in items:
            f.write(json.dumps(item, ensure_ascii=False) + "\n")


def main() -> int:
    ap = argparse.ArgumentParser(description="Synthetic Thai insurance applicant generator")
    ap.add_argument("--applicants", type=int, default=1000)
    ap.add_argument("--claims", type=int, default=500)
    ap.add_argument("--seed", type=int, default=42, help="Reproducibility seed")
    ap.add_argument("--output", type=Path, default=Path("./out"))
    ap.add_argument("--pdf", action="store_true",
                    help="Also render one medical certificate PDF per applicant (slow)")
    ap.add_argument("--pdf-limit", type=int, default=0,
                    help="If >0, only render this many PDFs (useful for sampling)")
    args = ap.parse_args()

    print(f"=== Synthetic Thai Applicants Generator ===")
    print(f"seed:        {args.seed}")
    print(f"applicants:  {args.applicants}")
    print(f"claims:      {args.claims}")
    print(f"output:      {args.output}")
    print()

    t = time.time()
    print(f"Generating applicants...", flush=True)
    applicants = generate_applicants(args.applicants, args.seed)
    print(f"  {len(applicants)} done ({time.time()-t:.1f}s)")

    t = time.time()
    print(f"Generating medical records...", flush=True)
    medical = generate_medical_records(applicants, args.seed)
    print(f"  {len(medical)} done ({time.time()-t:.1f}s)")

    t = time.time()
    print(f"Generating claims...", flush=True)
    claims = generate_claims(applicants, args.claims, args.seed)
    n_fraud = sum(1 for c in claims if c["is_canonical_fraud"])
    print(f"  {len(claims)} done ({time.time()-t:.1f}s) — {n_fraud} canonical-fraud")

    write_jsonl(args.output / "applicants.jsonl", applicants)
    write_jsonl(args.output / "medical_records.jsonl", medical)
    write_jsonl(args.output / "claims.jsonl", claims)
    print()
    print(f"Wrote → {args.output}/{{applicants,medical_records,claims}}.jsonl")

    if args.pdf:
        from .pdf_renderer import render_certificate
        pdf_dir = args.output / "pdfs"
        pdf_dir.mkdir(parents=True, exist_ok=True)
        # Build a quick applicant_id → medical map.
        med_by_id = {m["applicant_id"]: m for m in medical}
        limit = args.pdf_limit if args.pdf_limit > 0 else len(applicants)
        print()
        print(f"Rendering {limit} PDFs...", flush=True)
        t = time.time()
        for i, app in enumerate(applicants[:limit]):
            med = med_by_id.get(app["applicant_id"])
            if med is None:
                continue
            render_certificate(app, med, pdf_dir / f"{app['applicant_id']}.pdf")
            if (i + 1) % 50 == 0:
                sys.stdout.write(f"\r  {i+1}/{limit}")
                sys.stdout.flush()
        print(f"\n  done ({time.time()-t:.1f}s)")
        print(f"PDFs in: {pdf_dir}")

    # Summary stats
    print()
    print("=== Summary ===")
    print(f"Applicants:        {len(applicants)}")
    print(f"  with chronic dx: {sum(1 for m in medical if m['diagnoses'])}")
    print(f"  on medication:   {sum(1 for m in medical if m['medications'])}")
    print(f"Claims:            {len(claims)}")
    print(f"  canonical fraud: {n_fraud} ({n_fraud/len(claims):.1%})")
    print(f"  status distrib:")
    from collections import Counter
    for s, n in Counter(c["status"] for c in claims).most_common():
        print(f"    {s:25s}  {n}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

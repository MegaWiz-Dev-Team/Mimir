"""Download proposed medical benchmarks for Mimir/Asgard.

Run with:
    /opt/homebrew/opt/python@3.14/bin/python3.14 scripts/download_medical_benchmarks.py
(or any Python with `huggingface_hub` installed)

Outputs to: benchmarks/medical/<name>/  — total ~480 MB.
Idempotent — re-running skips existing dirs (delete to refresh).

Targets:
  Tier A (paper-comparable):
    1. MedQA          — bigbio/med_qa             (USMLE MCQ; data inside data_clean.zip)
    2. MedMCQA        — openlifescienceai/medmcqa (Indian AIIMS/NEET MCQ, 187K train)
    3. PubMedQA       — qiaojin/PubMedQA          (Y/N/Maybe over abstracts)
    4. HealthBench    — Azure blob (OpenAI)        (5K convs + Hard 1K + Consensus 3.7K)

  Tier B (reasoning + expert):
    5. MedXpertQA     — TsinghuaC3I/MedXpertQA    (Text 2,450 + MM 2,000 expert MCQ)

For MedQA, this script also unzips data_clean.zip into data_clean/ so
downstream loaders can read JSONL directly (US split is `data_clean/questions/US/`).
"""
import os, sys, json, urllib.request
from pathlib import Path

ROOT = Path("/Users/mimir/Developer/Mimir/benchmarks/medical")
ROOT.mkdir(parents=True, exist_ok=True)

def header(s):
    print()
    print("═" * 66)
    print(f" {s}")
    print("═" * 66)

# Use huggingface_hub.snapshot_download for resilient download
from huggingface_hub import snapshot_download, list_repo_files

TARGETS = [
    {
        "name":    "medqa",
        "repo":    "bigbio/med_qa",
        "type":    "dataset",
        "license": "MIT (per bigbio collection)",
        "paper":   "https://arxiv.org/abs/2009.13081",
        "format":  "MCQ (4-5 choices), USMLE-style",
        "n":       "~1273 test (en)",
    },
    {
        "name":    "medmcqa",
        "repo":    "openlifescienceai/medmcqa",
        "type":    "dataset",
        "license": "MIT",
        "paper":   "https://arxiv.org/abs/2203.14371",
        "format":  "MCQ (4 choices), AIIMS/NEET (India)",
        "n":       "187K dev / 6.1K test",
    },
    {
        "name":    "pubmedqa",
        "repo":    "qiaojin/PubMedQA",
        "type":    "dataset",
        "license": "MIT",
        "paper":   "https://arxiv.org/abs/1909.06146",
        "format":  "Y/N/Maybe Q over PubMed abstracts",
        "n":       "1K labeled + 211K unlabeled",
    },
    {
        "name":    "medxpertqa",
        "repo":    "TsinghuaC3I/MedXpertQA",
        "type":    "dataset",
        "license": "(check repo)",
        "paper":   "2025",
        "format":  "Expert-level reasoning",
        "n":       "TBD",
    },
]

for t in TARGETS:
    header(f"📥 {t['name']}  ({t['repo']})")
    out = ROOT / t["name"]
    if out.exists() and any(out.iterdir()):
        print(f"  ⊕ exists at {out} — skipping (delete to re-download)")
        continue
    try:
        files = list_repo_files(t["repo"], repo_type=t["type"])
        # Filter likely heavy or unrelated files
        wanted = [f for f in files if not any(
            f.lower().endswith(ext) for ext in
            (".jpg", ".png", ".mp4", ".bin", ".pt", ".onnx")
        )]
        # Print preview
        for f in wanted[:8]:
            print(f"   · {f}")
        if len(wanted) > 8:
            print(f"   · ... +{len(wanted)-8} more")

        path = snapshot_download(
            repo_id=t["repo"], repo_type=t["type"],
            local_dir=str(out),
            allow_patterns=[
                "*.json", "*.jsonl", "*.csv", "*.parquet", "*.md", "*.txt",
                "*.yaml", "*.yml", "*.zip", "*.py",  # .zip for bigbio/med_qa, .py for loader scripts
                "data/*", "**/data/**",
            ],
        )
        print(f"  ✓ downloaded → {path}")

        # MedQA's data is inside data_clean.zip — auto-unzip so downstream loaders
        # can read JSONL files directly without an extra step.
        if t["name"] == "medqa":
            zip_path = Path(path) / "data_clean.zip"
            data_dir = Path(path) / "data_clean"
            if zip_path.exists() and not (data_dir / "questions").exists():
                import zipfile
                with zipfile.ZipFile(zip_path) as z:
                    z.extractall(path)
                print(f"  📦 unzipped data_clean.zip → {data_dir}")
    except Exception as e:
        print(f"  ❌ FAILED: {type(e).__name__}: {str(e)[:200]}")

# HealthBench from direct Azure blob
header("📥 healthbench (OpenAI public blob)")
hb_dir = ROOT / "healthbench"
hb_dir.mkdir(exist_ok=True)
hb_files = [
    ("oss_eval.jsonl", "https://openaipublic.blob.core.windows.net/simple-evals/healthbench/2025-05-07-06-14-12_oss_eval.jsonl"),
    ("hard.jsonl",     "https://openaipublic.blob.core.windows.net/simple-evals/healthbench/hard_2025-05-08-21-00-10.jsonl"),
    ("consensus.jsonl","https://openaipublic.blob.core.windows.net/simple-evals/healthbench/consensus_2025-05-09-20-00-46.jsonl"),
]
for fname, url in hb_files:
    out = hb_dir / fname
    if out.exists() and out.stat().st_size > 0:
        print(f"  ⊕ {fname} exists ({out.stat().st_size/1024/1024:.1f} MB)")
        continue
    print(f"  ↓ {fname} ← {url}")
    try:
        urllib.request.urlretrieve(url, out)
        print(f"  ✓ {out.stat().st_size/1024/1024:.1f} MB")
    except Exception as e:
        print(f"  ❌ {e}")

# Per-benchmark README
header("📝 Writing README per benchmark")
for t in TARGETS:
    p = ROOT / t["name"] / "README.md"
    p.write_text(f"""# {t['name']} (Mimir mirror of {t['repo']})

- **Source HF repo:** [{t['repo']}](https://huggingface.co/datasets/{t['repo']})
- **Paper:** {t['paper']}
- **Format:** {t['format']}
- **Size:** {t['n']}
- **License:** {t['license']}

To rebuild this mirror, see `scripts/download_medical_benchmarks.py`.
""")
hb_readme = ROOT / "healthbench" / "README.md"
hb_readme.write_text("""# HealthBench (OpenAI)

- **Source:** https://openai.com/index/healthbench/
- **Paper:** https://arxiv.org/abs/2505.08775
- **Files:**
  - `oss_eval.jsonl` (5,000 conversations, main)
  - `hard.jsonl` (subset of harder examples)
  - `consensus.jsonl` (physician-consensus rubrics)
- **Grader (paper):** gpt-4.1-2025-04-14
- **License:** MIT (per simple-evals)
- **Reference grader code:** https://github.com/openai/simple-evals/blob/main/healthbench_eval.py
""")

# Summary
header("📊 Summary")
total = 0
for d in sorted(ROOT.glob("*")):
    if not d.is_dir(): continue
    files = list(d.rglob("*"))
    sz = sum(f.stat().st_size for f in files if f.is_file())
    total += sz
    print(f"  {d.name:20} {len(files):>4} files  {sz/1024/1024:>8.1f} MB")
print(f"  {'TOTAL':20} {'    '}        {total/1024/1024:>8.1f} MB")

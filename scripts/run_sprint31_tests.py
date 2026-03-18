#!/usr/bin/env python3
"""Sprint 31 — Run Mimir Rust unit tests and push results to Forseti.

Usage:
  python3 run_sprint31_tests.py

Runs `cargo test --lib` in ro-ai-bridge, parses output,
and stores results in Forseti's SQLite ResultsDB.
"""
import json
import re
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path

# ── Forseti integration ──────────────────────────────────
# Use absolute path to find Forseti regardless of CWD
SCRIPT_DIR = Path(__file__).resolve().parent
MIMIR_ROOT = SCRIPT_DIR.parent
DEVELOPER_DIR = MIMIR_ROOT.parent
FORSETI_ROOT = DEVELOPER_DIR / "Forseti"
sys.path.insert(0, str(FORSETI_ROOT / "src"))

from forseti.db.results_db import ResultsDB

# ── Configuration ────────────────────────────────────────
MIMIR_BRIDGE_DIR = MIMIR_ROOT / "ro-ai-bridge"
FORSETI_DB_PATH = str(FORSETI_ROOT / "forseti_results.db")
SUITE_NAME = "mimir-sprint31-vector-search"
PHASE = "unit"
BASE_URL = "cargo test (local)"


def run_cargo_tests() -> tuple[str, int, float]:
    """Run cargo test --lib and capture output.

    Returns: (stdout, return_code, duration_seconds)
    """
    start = time.time()
    result = subprocess.run(
        ["cargo", "test", "--lib"],
        cwd=str(MIMIR_BRIDGE_DIR),
        capture_output=True,
        text=True,
        timeout=300,
    )
    duration = time.time() - start
    output = result.stdout + "\n" + result.stderr
    return output, result.returncode, duration


def parse_test_output(output: str) -> list[dict]:
    """Parse `cargo test` output into scenario results.

    Returns list of dicts: [{name, status, duration_ms, error_message}]
    """
    scenarios = []
    # Match lines like: test retrieval::qdrant::tests::test_foo ... ok
    pattern = re.compile(r"^test\s+(\S+)\s+\.\.\.\s+(\w+)", re.MULTILINE)

    for match in pattern.finditer(output):
        name = match.group(1)
        raw_status = match.group(2).lower()
        status = "passed" if raw_status == "ok" else "failed"
        scenarios.append({
            "name": name,
            "status": status,
            "duration_ms": 0,  # cargo test doesn't report per-test timing
            "error_message": None if status == "passed" else f"Test {raw_status}",
        })

    return scenarios


def parse_summary(output: str) -> dict:
    """Parse the summary line: test result: ok. X passed; Y failed; ..."""
    summary = {"total": 0, "passed": 0, "failed": 0, "errors": 0, "skipped": 0}
    match = re.search(
        r"test result: (\w+)\.\s+(\d+) passed;\s+(\d+) failed;\s+(\d+) ignored;",
        output,
    )
    if match:
        summary["passed"] = int(match.group(2))
        summary["failed"] = int(match.group(3))
        summary["skipped"] = int(match.group(4))
        summary["total"] = summary["passed"] + summary["failed"] + summary["skipped"]
    return summary


def get_git_info() -> tuple[str, str]:
    """Get current version and commit from Cargo.toml / git."""
    version = "unknown"
    commit = "unknown"

    cargo_toml = MIMIR_BRIDGE_DIR / "Cargo.toml"
    if cargo_toml.exists():
        for line in cargo_toml.read_text().splitlines():
            if line.startswith("version"):
                version = line.split('"')[1]
                break

    try:
        result = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"],
            cwd=str(MIMIR_BRIDGE_DIR),
            capture_output=True, text=True, timeout=5,
        )
        if result.returncode == 0:
            commit = result.stdout.strip()
    except Exception:
        pass

    return version, commit


def push_to_fenrir(run_id: int, summary: dict) -> None:
    """Push results to Fenrir /api/test-results endpoint (best-effort)."""
    import urllib.request

    fenrir_url = "http://localhost:8200/api/test-results"
    payload = {
        "suite": SUITE_NAME,
        "run_id": run_id,
        "total": summary["total"],
        "passed": summary["passed"],
        "failed": summary["failed"],
        "timestamp": datetime.now().isoformat(),
    }
    try:
        req = urllib.request.Request(
            fenrir_url,
            data=json.dumps(payload).encode(),
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        urllib.request.urlopen(req, timeout=5)
        print(f"   📤 Pushed to Fenrir: {fenrir_url}")
    except Exception as e:
        print(f"   ⚠️  Fenrir push skipped (not running): {e}")


def main():
    print("=" * 60)
    print("⚖️  Sprint 31 — Mimir Unit Tests → Forseti")
    print("=" * 60)

    # 1) Run tests
    print("\n🧪 Running cargo test --lib ...")
    output, returncode, duration = run_cargo_tests()

    # 2) Parse results
    scenarios = parse_test_output(output)
    summary = parse_summary(output)
    version, commit = get_git_info()
    duration_ms = int(duration * 1000)

    print(f"\n📊 Results: {summary['passed']} passed, {summary['failed']} failed, "
          f"{summary['skipped']} skipped (total: {summary['total']})")
    print(f"   Version: {version} | Commit: {commit} | Duration: {duration:.1f}s")

    # 3) Store in Forseti
    print(f"\n💾 Storing in Forseti DB: {FORSETI_DB_PATH}")
    db = ResultsDB(db_path=FORSETI_DB_PATH)

    run_id = db.save_run(
        suite_name=SUITE_NAME,
        phase=PHASE,
        base_url=BASE_URL,
        total=summary["total"],
        passed=summary["passed"],
        failed=summary["failed"],
        errors=summary["errors"],
        skipped=summary["skipped"],
        duration_ms=duration_ms,
        project_version=version,
        project_commit=commit,
    )
    print(f"   Run ID: {run_id}")

    # Save individual scenarios
    for scenario in scenarios:
        db.save_scenario(
            run_id=run_id,
            name=scenario["name"],
            status=scenario["status"],
            duration_ms=scenario["duration_ms"],
            error_message=scenario["error_message"],
        )

    # Also export as JSON to Forseti results dir
    results_dir = FORSETI_ROOT / "results"
    results_dir.mkdir(exist_ok=True)
    ts = datetime.now().strftime("%Y%m%d_%H%M%S")
    result_file = results_dir / f"{SUITE_NAME}_{ts}.json"
    result_data = {
        "suite": SUITE_NAME,
        "phase": PHASE,
        "run_id": run_id,
        "version": version,
        "commit": commit,
        "timestamp": datetime.now().isoformat(),
        "summary": summary,
        "duration_ms": duration_ms,
        "scenarios": scenarios,
    }
    result_file.write_text(json.dumps(result_data, indent=2, ensure_ascii=False))
    print(f"   📄 JSON: {result_file}")

    # 4) Push to Fenrir (best-effort)
    push_to_fenrir(run_id, summary)

    # 5) Print retrieval-specific tests
    retrieval_tests = [s for s in scenarios if "retrieval" in s["name"]]
    if retrieval_tests:
        print(f"\n🎯 Sprint 31 Retrieval Tests ({len(retrieval_tests)}):")
        for t in retrieval_tests:
            icon = "✅" if t["status"] == "passed" else "❌"
            short_name = t["name"].split("::")[-1]
            print(f"   {icon} {short_name}")

    db.close()

    print(f"\n{'=' * 60}")
    if summary["failed"] == 0:
        print(f"✅ All {summary['total']} tests passed! Results saved to Forseti (Run #{run_id})")
    else:
        print(f"❌ {summary['failed']} tests failed. Results saved to Forseti (Run #{run_id})")
    print(f"{'=' * 60}")

    sys.exit(returncode)


if __name__ == "__main__":
    main()

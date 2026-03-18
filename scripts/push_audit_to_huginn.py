#!/usr/bin/env python3
"""Push cargo-audit results into Huginn's SQLite database.

Usage: python3 push_audit_to_huginn.py [path/to/huginn.db]
"""
import json
import sqlite3
import subprocess
import sys
import uuid
from datetime import datetime, timezone


def run_cargo_audit(project_path: str) -> list[dict]:
    """Run cargo audit --json and return parsed findings."""
    result = subprocess.run(
        ["cargo", "audit", "--json"],
        capture_output=True, text=True,
        cwd=project_path,
    )
    try:
        data = json.loads(result.stdout)
    except json.JSONDecodeError:
        print("⚠️  cargo-audit --json failed, parsing text output")
        return []

    findings = []
    for vuln in data.get("vulnerabilities", {}).get("list", []):
        advisory = vuln.get("advisory", {})
        pkg = vuln.get("package", {})
        cvss = advisory.get("cvss", None)
        try:
            score = float(cvss) if cvss and str(cvss).replace(".", "").isdigit() else None
        except (ValueError, TypeError):
            score = None

        findings.append({
            "id": advisory.get("id", str(uuid.uuid4())),
            "severity": map_severity(score),
            "title": f"{advisory.get('id', 'Unknown')}: {advisory.get('title', 'Unknown')}",
            "description": advisory.get("description", "")[:500],
            "location": f"{pkg.get('name', '?')} v{pkg.get('version', '?')}",
            "tool": "cargo-audit",
            "cwe": None,
            "owasp": None,
            "remediation": advisory.get("url", ""),
            "confidence": "high",
            "evidence": None,
            "cvss_score": score,
            "status": "open",
            "fixed_in": None,
        })

    for warning_list in data.get("warnings", {}).values():
        if isinstance(warning_list, list):
            for w in warning_list:
                advisory = w.get("advisory", w)
                pkg = w.get("package", {})
                findings.append({
                    "id": advisory.get("id", str(uuid.uuid4())),
                    "severity": "low",
                    "title": f"{advisory.get('id', 'Warning')}: {advisory.get('title', 'Unmaintained')}",
                    "description": advisory.get("description", "")[:500],
                    "location": f"{pkg.get('name', '?')} v{pkg.get('version', '?')}",
                    "tool": "cargo-audit",
                    "cwe": None,
                    "owasp": None,
                    "remediation": advisory.get("url", ""),
                    "confidence": "medium",
                    "evidence": None,
                    "cvss_score": None,
                    "status": "open",
                    "fixed_in": None,
                })
    return findings


def map_severity(score: float | None) -> str:
    if score is None:
        return "medium"
    if score >= 9.0: return "critical"
    if score >= 7.0: return "high"
    if score >= 4.0: return "medium"
    if score >= 0.1: return "low"
    return "info"


def get_git_info(project_path: str) -> tuple[str, str]:
    """Get current commit hash and branch from git."""
    try:
        commit = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"],
            capture_output=True, text=True, cwd=project_path,
        ).stdout.strip()
        branch = subprocess.run(
            ["git", "rev-parse", "--abbrev-ref", "HEAD"],
            capture_output=True, text=True, cwd=project_path,
        ).stdout.strip()
        return commit, branch
    except Exception:
        return "", ""


def push_to_huginn(
    db_path: str,
    findings: list[dict],
    project: str,
    version: str,
    sprint: str,
    commit_hash: str,
    branch: str,
) -> tuple[str, int, int]:
    """Insert scan + findings into Huginn's SQLite DB."""
    conn = sqlite3.connect(db_path)
    conn.execute("PRAGMA journal_mode=WAL")
    conn.execute("PRAGMA foreign_keys=ON")

    # Ensure tables exist (same schema as Huginn's Rust migration)
    conn.executescript("""
        CREATE TABLE IF NOT EXISTS scans (
            scan_id TEXT PRIMARY KEY,
            target TEXT NOT NULL,
            scan_type TEXT NOT NULL DEFAULT 'whitebox',
            status TEXT NOT NULL DEFAULT 'pending',
            started_at TEXT NOT NULL,
            finished_at TEXT,
            report_hash TEXT,
            error TEXT,
            project TEXT,
            version TEXT,
            sprint TEXT,
            commit_hash TEXT,
            branch TEXT
        );
        CREATE TABLE IF NOT EXISTS findings (
            id TEXT PRIMARY KEY,
            scan_id TEXT NOT NULL,
            severity TEXT NOT NULL,
            title TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            location TEXT NOT NULL DEFAULT '',
            tool TEXT NOT NULL DEFAULT '',
            cwe TEXT,
            owasp TEXT,
            remediation TEXT,
            confidence TEXT NOT NULL DEFAULT 'medium',
            evidence TEXT,
            cvss_score REAL,
            status TEXT NOT NULL DEFAULT 'open',
            fixed_in TEXT,
            FOREIGN KEY (scan_id) REFERENCES scans(scan_id)
        );
        CREATE INDEX IF NOT EXISTS idx_findings_scan_id ON findings(scan_id);
        CREATE INDEX IF NOT EXISTS idx_scans_project ON scans(project);
    """)

    scan_id = f"{project}-cargo-audit-{datetime.now().strftime('%Y%m%d-%H%M%S')}"
    now = datetime.now(timezone.utc).isoformat()

    conn.execute(
        """INSERT INTO scans (scan_id, target, scan_type, status, started_at, finished_at,
           project, version, sprint, commit_hash, branch)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)""",
        (scan_id, f"{project}/ro-ai-bridge", "whitebox", "completed", now, now,
         project, version, sprint, commit_hash, branch),
    )

    for f in findings:
        conn.execute(
            """INSERT OR REPLACE INTO findings
               (id, scan_id, severity, title, description, location, tool, cwe, owasp,
                remediation, confidence, evidence, cvss_score, status, fixed_in)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)""",
            (f["id"], scan_id, f["severity"], f["title"], f["description"],
             f["location"], f["tool"], f["cwe"], f["owasp"], f["remediation"],
             json.dumps(f["confidence"]), f["evidence"],
             f["cvss_score"], f["status"], f["fixed_in"]),
        )

    conn.commit()

    cur = conn.execute("SELECT COUNT(*) FROM scans")
    total_scans = cur.fetchone()[0]
    cur = conn.execute("SELECT COUNT(*) FROM findings WHERE scan_id = ?", (scan_id,))
    new_findings = cur.fetchone()[0]
    conn.close()

    return scan_id, new_findings, total_scans


if __name__ == "__main__":
    db_path = sys.argv[1] if len(sys.argv) > 1 else "/Users/mimir/Developer/Huginn/huginn.db"
    project_path = sys.argv[2] if len(sys.argv) > 2 else "/Users/mimir/Developer/Mimir/ro-ai-bridge"
    project_name = sys.argv[3] if len(sys.argv) > 3 else "mimir"
    project_version = sys.argv[4] if len(sys.argv) > 4 else "0.31.0"
    sprint_name = sys.argv[5] if len(sys.argv) > 5 else "sprint-31"

    print("=" * 60)
    print("🐦‍⬛ Huginn — cargo-audit → SQLite")
    print("=" * 60)

    commit_hash, branch = get_git_info(project_path)
    print(f"   Project: {project_name} v{project_version}")
    print(f"   Sprint:  {sprint_name}")
    print(f"   Commit:  {commit_hash} ({branch})")

    print(f"\n🔍 Running cargo audit on {project_path}...")
    findings = run_cargo_audit(project_path)
    print(f"   Found {len(findings)} findings")

    print(f"\n💾 Pushing to Huginn DB: {db_path}")
    scan_id, count, total = push_to_huginn(
        db_path, findings, project_name, project_version, sprint_name, commit_hash, branch,
    )
    print(f"   Scan ID: {scan_id}")
    print(f"   Findings stored: {count}")
    print(f"   Total scans in DB: {total}")

    print("\n" + "=" * 60)
    sev_summary: dict[str, int] = {}
    for f in findings:
        sev_summary[f["severity"]] = sev_summary.get(f["severity"], 0) + 1
    for sev, cnt in sorted(sev_summary.items()):
        icon = {"critical": "🔴", "high": "🟠", "medium": "🟡", "low": "🔵", "info": "⚪"}.get(sev, "❓")
        print(f"   {icon} {sev}: {cnt}")
    print("=" * 60)

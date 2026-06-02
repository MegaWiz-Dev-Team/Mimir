#!/usr/bin/env python3
"""B3 — resolve a TMT medicine id to a FHIR R5 ``Medication.doseForm`` CodeableConcept.

Chain (all built in Sprint 58, see docs/03_implementation_plans/03_19_*.md):
    tmt_id ──snomed_tmt_dose_link──▶ SNOMED dose-form concept ──snomed_edqm_dose_map──▶ EDQM code

Only ``needs_review=0`` links are trusted (exact / normalized matches). token_subset
links are needs_review=1 and are deliberately NOT auto-coded — they need human
confirmation first — so this resolver returns ``None`` for them, leaving doseForm
absent rather than asserting a possibly-wrong subtype.

Reusable: ``resolve_dose_form(tmt_id, query)`` is pure given a ``query`` callable
(sql -> tab-separated rows), so it unit-tests without a DB and the same function
serves the Python FHIR generator now and a future Rust mimir-fhir path.

CLI:  python3 fhir_dose_form.py <tmt_id> [<tmt_id> ...]
"""
from __future__ import annotations

import json
import os
import subprocess
import sys

SYS_SNOMED = "http://snomed.info/sct"
SYS_EDQM = "https://standardterms.edqm.eu"   # EDQM Standard Terms code system


def _default_query(sql: str) -> str:
    """MariaDB query via local mysql CLI, else kubectl exec (mirrors the loaders)."""
    user = os.environ.get("MARIADB_USER", "root")
    pw = os.environ.get("MARIADB_PASS", "root")
    db = os.environ.get("MARIADB_DB", "mimir")
    try:
        subprocess.run(["mysql", "--version"], capture_output=True, check=True)
        host = os.environ.get("MARIADB_HOST", "127.0.0.1")
        port = os.environ.get("MARIADB_PORT", "33306")
        cmd = ["mysql", "-h", host, "-P", port, "-u", user, f"-p{pw}", db, "-B", "-N"]
    except (FileNotFoundError, subprocess.CalledProcessError):
        ns = os.environ.get("MARIADB_NAMESPACE", "asgard-infra")
        cmd = ["kubectl", "-n", ns, "exec", "-i", "deploy/mariadb", "--",
               "mariadb", "-u", user, f"-p{pw}", db, "-B", "-N"]
    r = subprocess.run(cmd, input=sql.encode("utf-8"), capture_output=True)
    if r.returncode != 0:
        raise RuntimeError(f"mariadb query error: {r.stderr.decode()[:300]}")
    return r.stdout.decode("utf-8")


def _esc(s: str) -> str:
    return s.replace("\\", "\\\\").replace("'", "\\'")


def resolve_dose_form(tmt_id: str, query=_default_query) -> dict | None:
    """Return a FHIR CodeableConcept for the TMT id's dose form, or None.

    Coding always includes SNOMED; EDQM is added when the concept carries an EDQM
    map. ``None`` when there is no trusted (needs_review=0) link.
    """
    sql = (
        "SELECT l.snomed_concept_id, "
        "  (SELECT term FROM snomed_descriptions d WHERE d.concept_id=l.snomed_concept_id "
        "     AND d.term_type='fsn' LIMIT 1) AS fsn, "
        "  (SELECT e.edqm_code FROM snomed_edqm_dose_map e "
        "     WHERE e.snomed_concept_id=l.snomed_concept_id ORDER BY e.edqm_code LIMIT 1) AS edqm "
        f"FROM snomed_tmt_dose_link l WHERE l.tmt_id='{_esc(tmt_id)}' AND l.needs_review=0 LIMIT 1"
    )
    out = query(sql).strip()
    if not out:
        return None
    parts = out.split("\t")
    concept = parts[0] if parts else ""
    fsn = parts[1] if len(parts) > 1 else ""
    edqm = parts[2] if len(parts) > 2 else ""
    if not concept or concept == "NULL":
        return None
    display = fsn.replace("(dose form)", "").strip() if fsn and fsn != "NULL" else None
    codings = [{"system": SYS_SNOMED, "code": concept,
                **({"display": display} if display else {})}]
    if edqm and edqm != "NULL":
        codings.append({"system": SYS_EDQM, "code": edqm})
    cc: dict = {"coding": codings}
    if display:
        cc["text"] = display
    return cc


def main(argv: list[str]) -> int:
    if not argv:
        print("usage: fhir_dose_form.py <tmt_id> [<tmt_id> ...]", file=sys.stderr)
        return 2
    for tmt_id in argv:
        cc = resolve_dose_form(tmt_id)
        print(f"# tmt_id={tmt_id}")
        print(json.dumps(cc, ensure_ascii=False, indent=2) if cc else "  (no trusted dose form)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

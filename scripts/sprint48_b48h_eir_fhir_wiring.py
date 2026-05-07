#!/usr/bin/env python3
"""
Sprint 48 B-48h — wire ICD-10-TM + FHIR Condition coding into Eir agent prompts.

Updates the system_prompt of the 5 deployed Eir agents (eir, eir-cardio,
eir-pediatrics, eir-ent, eir-sleep) to teach the LLM HOW to use the
icd10_lookup tool and emit FHIR-shaped Condition.code output.

Idempotent: detects the marker '## ICD-10 Coding & FHIR Condition Output'
and skips agents that already have the section.

Usage:
  python3 sprint48_b48h_eir_fhir_wiring.py [--dry-run] [--revert]
"""
from __future__ import annotations
import argparse
import subprocess
import sys

MARIADB_POD = (
    "k8s_mariadb_mariadb-fb55894c5-xjjvb_asgard-infra_"
    "78f65c51-6439-4d1c-a9f6-ebcdad463f5c_58"
)

MARKER = "## ICD-10 Coding & FHIR Condition Output"
TENANT = "asgard_medical"
AGENTS = ["eir", "eir-cardio", "eir-pediatrics", "eir-ent", "eir-sleep"]
INSERT_BEFORE = "Respond in the same language"

# The new section — clear, structured, calls out FHIR system URI per
# Asgard's Sprint 48 ICD-10-TM source (anamai-moph-2010 → MoPH 2017).
FHIR_SECTION = """## ICD-10 Coding & FHIR Condition Output

Whenever you propose a diagnosis, differential, or assessment:

1. **Lookup the code.** Call the `icd10_lookup` tool with the diagnosis term
   (Thai or English natural language is fine — the tool handles bilingual
   semantic search via BGE-M3 multilingual embedding).
   - Tool returns top-K matches with code, en_label, th_label, chapter.
   - Prefer the most general code (shorter `code` length) when appropriate
     unless the clinical context calls for a sub-code.

2. **Include the code inline** in narrative output:
   - Thai: "หลอดเลือดสมองตีบ (ICD-10-TM: **I63.9**)"
   - English: "Acute myocardial infarction (ICD-10-TM: **I21**)"

3. **Emit structured FHIR Condition.code** when generating an assessment for
   downstream HIS/EHR integration. Use this exact shape:
   ```json
   {
     "resourceType": "Condition",
     "code": {
       "coding": [{
         "system": "https://www.who.int/icd-10-tm",
         "version": "ICD-10-TM 2017",
         "code": "<code>",
         "display": "<en_label>"
       }],
       "text": "<original_diagnosis_text>"
     }
   }
   ```
   Use `system` = `https://www.who.int/icd-10-tm` for Thai context;
   `http://hl7.org/fhir/sid/icd-10` for international.

4. **Multiple candidate codes.** If `icd10_lookup` returns several plausible
   matches with similar relevance, present the top-3 with brief Thai +
   English labels and ask the clinician to confirm.

5. **Never invent codes.** If `icd10_lookup` returns no plausible match,
   say so explicitly and recommend manual coder review — do NOT fabricate
   an ICD code from training-data memory.
"""


def mariadb_query(sql: str) -> str:
    r = subprocess.run(
        ["docker", "exec", "-i", MARIADB_POD,
         "mariadb", "-u", "mimir", "-pREDACTED-PW", "mimir", "-B", "-N"],
        input=sql.encode("utf-8"), capture_output=True,
    )
    if r.returncode != 0:
        raise RuntimeError(f"mariadb error: {r.stderr.decode()}")
    return r.stdout.decode("utf-8")


def fetch_prompt(name: str) -> str | None:
    rows = mariadb_query(
        f"SELECT system_prompt FROM agent_configs "
        f"WHERE tenant_id='{TENANT}' AND name='{name}'"
    )
    return rows.strip() or None


def write_prompt(name: str, prompt: str) -> None:
    # Use mariadb stdin with the prompt as a file to avoid quoting issues.
    sql = (
        f"UPDATE agent_configs "
        f"SET system_prompt = ?, updated_at = CURRENT_TIMESTAMP "
        f"WHERE tenant_id = '{TENANT}' AND name = '{name}';"
    )
    # Direct UPDATE via subprocess with prompt as parameter — easier to do
    # via temp file + LOAD_FILE or just heredoc through stdin.
    import tempfile, os
    with tempfile.NamedTemporaryFile("w", suffix=".sql", delete=False, encoding="utf-8") as f:
        # Escape single quotes for SQL string literal.
        escaped = prompt.replace("\\", "\\\\").replace("'", "\\'")
        f.write(
            f"UPDATE agent_configs "
            f"SET system_prompt = '{escaped}', "
            f"updated_at = CURRENT_TIMESTAMP "
            f"WHERE tenant_id = '{TENANT}' AND name = '{name}';\n"
        )
        f.flush()
        path = f.name
    try:
        # Copy file into pod, then run.
        copy = subprocess.run(
            ["docker", "cp", path, f"{MARIADB_POD}:/tmp/_b48h.sql"],
            capture_output=True, check=True,
        )
        run = subprocess.run(
            ["docker", "exec", MARIADB_POD,
             "mariadb", "-u", "mimir", "-pREDACTED-PW", "mimir",
             "-e", "SOURCE /tmp/_b48h.sql;"],
            capture_output=True,
        )
        if run.returncode != 0:
            raise RuntimeError(f"update {name}: {run.stderr.decode()}")
    finally:
        os.unlink(path)


def patch_prompt(prompt: str) -> str:
    if MARKER in prompt:
        return prompt  # already patched
    if INSERT_BEFORE in prompt:
        return prompt.replace(
            INSERT_BEFORE,
            f"{FHIR_SECTION}\n{INSERT_BEFORE}",
        )
    # Append at end if anchor not found.
    return f"{prompt.rstrip()}\n\n{FHIR_SECTION}\n"


def revert_prompt(prompt: str) -> str:
    if MARKER not in prompt:
        return prompt
    # Strip from MARKER through to end of section (next ## or EOF).
    idx = prompt.find(MARKER)
    # End = next "## " heading or end of string
    after = prompt[idx + len(MARKER):]
    next_section = after.find("\n## ")
    if next_section == -1:
        # Section runs to end of file.
        return prompt[:idx].rstrip() + "\n"
    return prompt[:idx] + after[next_section + 1:]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--revert", action="store_true",
                    help="Remove the FHIR section instead of adding it.")
    args = ap.parse_args()

    print(f"=== B-48h FHIR Condition wiring · {len(AGENTS)} agents ===")
    if args.revert:
        print("  mode: REVERT")
    elif args.dry_run:
        print("  mode: dry-run")
    else:
        print("  mode: APPLY")

    for name in AGENTS:
        prompt = fetch_prompt(name)
        if prompt is None:
            print(f"  ⚠️  {name}: not found in agent_configs (skip)")
            continue
        if args.revert:
            new_prompt = revert_prompt(prompt)
            action = "revert"
        else:
            new_prompt = patch_prompt(prompt)
            action = "patch"

        if new_prompt == prompt:
            print(f"  = {name}: no change ({MARKER!r:.40s} {'present' if MARKER in prompt else 'absent'})")
            continue

        delta = len(new_prompt) - len(prompt)
        print(f"  {'+' if delta > 0 else '-'} {name}: {action} (Δ={delta:+d} chars)")

        if not args.dry_run:
            write_prompt(name, new_prompt)

    if args.dry_run:
        print("\n[dry-run] no changes written")
    else:
        print("\n=== Done ===")
        print("Verify: agents will pick up new prompt on next chat (no restart needed).")

    return 0


if __name__ == "__main__":
    sys.exit(main())

# Sprint 49 W2.3a — LOINC ValueSet Ingest Runbook

**Purpose:** Populate `loinc_codes` master table with LOINC official ValueSet.
Powers FHIR `Observation.code` binding validation in Phase B.3 (canonical
coding-validator service).

**Status:** Schema + script ready. Awaiting manual `Loinc.csv` download
(requires free LOINC account registration).

---

## Why LOINC

FHIR R4 mandates LOINC for `Observation.code` in most clinical contexts:
- Lab results (chem panel, CBC, urinalysis)
- Vital signs (BP, HR, temp, SpO2)
- Imaging study types
- Survey instruments (PHQ-9, GAD-7)

Without LOINC populated, our `validate_coding(system='http://loinc.org', code=X)`
returns `SystemMissing` — Eir agents calling `read_fhir` can't trust Observation
coding, and FHIR Bundle emit cannot achieve "Validated" binding.

ICD-10 covers diagnoses; LOINC covers observations. They are non-overlapping.

---

## Source

- **URL:** https://loinc.org/downloads/
- **License:** Free for any use (LOINC license, public-domain-equivalent for
  the master ValueSet; commercial use of derivative LOINC-mapped tools may
  need terms review). **No payment, no per-deployment fee.**
- **Registration:** One-time free account required to download. Use the
  Megawiz Asgard service account (not personal).
- **File:** `LOINC_<version>_Source.zip` → unzip → `Loinc.csv` (~50 columns,
  ~98K rows for v2.78).
- **Vintage:** Use the latest stable release (LOINC ships ~2x/year).

---

## Schema

Migration: [migrations/sprint49_loinc_codes.sql](../../ro-ai-bridge/migrations/sprint49_loinc_codes.sql)

Two tables, mirroring the ICD-10 pattern from Sprint 48:

| Table | Purpose |
|---|---|
| `loinc_codes` | Master codes. PK `(loinc_num, source_version)`. Multi-vintage parallel. `tenant_id=NULL` for shared master. FULLTEXT index on `(long_common_name, short_name, component)` for free-text fallback. |
| `loinc_ingest_runs` | Audit trail per ingest. Tracks source URL, SHA-256, row counts, status. |

Columns persisted from Loinc.csv:

- `LOINC_NUM` → `loinc_num` (PK)
- `LONG_COMMON_NAME` → `long_common_name` (display)
- `SHORTNAME` / `COMPONENT` → analytic naming
- 5-axis: `PROPERTY` / `TIME_ASPCT` / `SYSTEM` (→ `system_axis` to avoid SQL keyword) / `SCALE_TYP` / `METHOD_TYP`
- `CLASS` → grouping (CHEM, HEM/BC, MICRO, etc.)
- `STATUS` (`ACTIVE` / `DEPRECATED` / `DISCOURAGED` / `TRIAL`)
- `EXAMPLE_UCUM_UNITS` → preferred unit hint
- `RELATEDNAMES2` / `CONSUMER_NAME` / `VERSION_LAST_CHANGED` → `locale_metadata` JSON

Skipped columns (not clinically useful for Asgard): the LOINC molecular-ID
columns, panel hierarchy, multilingual translations beyond English (we'll
handle Thai lab-name display in a follow-on once we have a Thai-LOINC
translation source confirmed).

---

## Running the ingest

```bash
# 1. Manual download (one-time):
#    a. Register a free LOINC account: https://loinc.org/join-loinc/
#    b. Download LOINC_<ver>_Source.zip from https://loinc.org/downloads/
#    c. Unzip → keep Loinc.csv

# 2. Apply migration (per repo migration convention):
mysql -h 127.0.0.1 -P 33306 -u root -proot mimir \
  < ro-ai-bridge/migrations/sprint49_loinc_codes.sql

# 3. Smoke parse first (no DB writes):
python scripts/loinc_ingest.py \
  --csv /path/to/Loinc.csv \
  --source-version loinc-2.78 \
  --dry-run

# 4. Full ingest:
python scripts/loinc_ingest.py \
  --csv /path/to/Loinc.csv \
  --source-version loinc-2.78

# 5. Verify:
mysql -h 127.0.0.1 -P 33306 -u root -proot mimir -e \
  "SELECT class, COUNT(*) FROM loinc_codes
   WHERE source_version='loinc-2.78'
   GROUP BY class ORDER BY 2 DESC LIMIT 10;"
```

Expected: ~98K rows for v2.78, dominated by CHEM/HEM/MICRO classes.

---

## Database connection env

Script reads these env vars (matches the local dev recipe in
`s1_e2e_manual_2026_05_18` memory; defaults match the asgard-infra
port-forward style):

| Var | Default | Notes |
|---|---|---|
| `MARIADB_HOST` | `127.0.0.1` | |
| `MARIADB_PORT` | `33306` | `kubectl port-forward svc/mariadb 33306:3306 -n asgard-infra` |
| `MARIADB_USER` | `root` | |
| `MARIADB_PASS` | `root` | Literal `root` for asgard-infra/mariadb |
| `MARIADB_DB` | `mimir` | |

---

## Downstream wiring (Sprint 2 W2.3d)

Once `loinc_codes` is populated, the coding-validator service can answer:

```rust
validate_coding(system="http://loinc.org", code="2160-0")
  → Validated { display: "Creatinine [Mass/volume] in Serum or Plasma",
                class: "CHEM", status: "ACTIVE" }

validate_coding(system="http://loinc.org", code="bogus-999")
  → Unknown
```

Then FHIR Bundle emit (S3.F.2) can guarantee Validated bindings for
`Observation.code` in the OCR-pipeline output.

---

## Why not LOINC fhirbolt / fhir-validator JAR

We considered using the official Java FHIR validator, which embeds LOINC.
Rejected because:
- 200MB+ JAR + JVM cold-start (~10s) per validation call → too heavy
- Asgard is Rust-first; want in-process Rust validator
- Master table + FULLTEXT index gives us O(1) lookup + free-text fallback,
  which the JAR validator doesn't provide

The Rust validator we'll write in W2.3d is a thin SELECT over these tables.

---

## Risks / open items

- **Bilingual Thai display** — LOINC ships English-only. Thai display
  (lab names a doctor or patient sees in UI) is out of scope for v1
  ingest; if Mega Care needs Thai LOINC names, ingest a Thai-LOINC
  mapping CSV as a separate `source_version` like `loinc-th-megacare-1`.
- **Version drift** — LOINC ships 2x/year. Plan a refresh cadence; the
  multi-source-version PK means we never overwrite previous ingest.
- **TMT / TPC parity** — Same pattern will repeat when (if) we get TMT
  (Thai meds) and TPC (Thai procedures) license, per W2.3b/c.

---

## See also

- [migrations/sprint49_loinc_codes.sql](../../ro-ai-bridge/migrations/sprint49_loinc_codes.sql)
- [scripts/loinc_ingest.py](../../scripts/loinc_ingest.py)
- ADR-006 FHIR canonical design (Asgard/docs/decisions/ADR-006-fhir-canonical-design.md)
- W2.3d Coding validator (sprint 2 task, not yet implemented)

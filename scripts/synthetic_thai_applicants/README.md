# Synthetic Thai Insurance Applicant Generator

Reproducible synthetic data generator for **Sprint 2 W2.1b** (I1 dataset).

Produces Thai-localized insurance applicants + medical records + claims with
**correlated fraud injection** (NOT random — unlike the AWS sample's
`fraud_indicators = randint(0,5)` pattern).

## Why this exists

Multiple Asgard datasets need synthetic input that's realistic enough for
downstream tools to actually learn from:

| Dataset | What this generator provides |
|---|---|
| **I1** Underwriter regression | `applicants.jsonl` — feed pipeline, assert determinism |
| **I2** Fraud detection | `claims.jsonl` with `is_canonical_fraud` ground-truth label |
| **M3** Medical chart OCR | `pdfs/APP-NNNNN.pdf` for Syn OCR + Skuggi PII benchmark |
| **X1** Skuggi PII detection | Thai citizen IDs (Luhn-valid 13-digit) + Thai names |

All outputs are deterministic per seed — `--seed 42` produces byte-identical
files across runs. Tests in `tests/test_synthetic_thai_applicants/` pin this
contract.

## Install

```bash
pip install -r scripts/synthetic_thai_applicants/requirements.txt
```

On homebrew Python 3.14:

```bash
pip3 install --user --break-system-packages faker reportlab
```

## Usage

```bash
PYTHONPATH=scripts /opt/homebrew/bin/python3 -m synthetic_thai_applicants \
    --applicants 1000 \
    --claims 500 \
    --seed 42 \
    --output ./out \
    --pdf --pdf-limit 50      # render 50 medical-certificate PDFs (optional)
```

Output layout:

```
out/
├── applicants.jsonl         # 1 JSON per line
├── medical_records.jsonl    # 1:1 with applicants
├── claims.jsonl             # 1 JSON per line; includes is_canonical_fraud
└── pdfs/                    # iff --pdf
    ├── APP-00001.pdf
    └── ...
```

## Fraud rule semantics

Per `config.FRAUD_RULES`:

| Rule | Points | Trigger |
|---|---|---|
| `short_policy_high_amount` | 3 | `days_since < 90` AND `amount > 500K THB` |
| `very_short_policy` | 2 | `days_since < 30` |
| `amount_near_limit` | 2 | `amount / policy_limit > 0.8` |
| `under_investigation` | 2 | `status == "Under Investigation"` |
| `repeat_claimant_3plus` | 1 | applicant has 3+ claims in the run |

Total capped at 5; `is_canonical_fraud = True` when total ≥ 4 (configurable
via `FRAUD_FLAG_THRESHOLD`). Typical fraud rate is 10-25% of claims per the
default generator settings.

The generator **deliberately concentrates** ~20% of claims into a very-short-
policy window and ~40% of those into high amounts, so the correlation is
strong enough for downstream models to learn from but not so strong that
real-world generalization is impossible.

## Schema

### Applicant (subset)

```jsonc
{
  "applicant_id": "APP-00001",
  "citizen_id": "2433218196003",       // Luhn-valid Thai 13-digit
  "name": "กิติชัย บุญส่ง",
  "age": 58,
  "gender": "ชาย",
  "occupation": "ครู",
  "income_thb_monthly": 72314,
  "bmi": 41.1,                          // consistent with weight/height
  "smoker": false,
  "family_history_diabetes": true,
  // ... 14 more fields
}
```

### Medical record

```jsonc
{
  "applicant_id": "APP-00001",
  "blood_pressure_systolic": 115,       // correlates with age/BMI/smoker
  "blood_pressure_diastolic": 73,
  "hba1c_pct": 5.2,                     // elevated if E10/E11 in diagnoses
  "egfr_ml_min": 70,
  "diagnoses": [                        // 0-3 chronic, weighted by risk
    {"icd10_code": "E11", "en_label": "...", "th_label": "..."}
  ],
  "medications": [                      // mapped from diagnoses, not random
    {"generic_en": "Metformin", "generic_th": "เมตฟอร์มิน", "dose": "500 mg BID"}
  ],
  "hospitalizations_last_year": 0,
  "surgeries_history": [],
  "allergies": []
}
```

### Claim (with fraud)

```jsonc
{
  "claim_id": "CLM-000004",
  "applicant_id": "APP-00018",
  "claim_type": "ทันตกรรม",
  "claim_amount_thb": 901868,
  "policy_limit_thb": 16064891,
  "days_since_policy_start": 45,
  "status": "Under Investigation",
  "fraud_indicators": 5,                              // 0-5, capped
  "fraud_rules_fired": [                              // which rules fired
    "short_policy_high_amount", "under_investigation"
  ],
  "is_canonical_fraud": true                          // sum >= threshold
}
```

## Tests

```bash
PYTHONPATH=scripts /opt/homebrew/bin/python3 -m pytest tests/test_synthetic_thai_applicants/ -v
```

19 tests covering:
- Reproducibility (same seed → identical output)
- Thai citizen ID Luhn-checksum validity (every generated ID)
- Schema invariants (no field drift)
- Correlation: every `is_canonical_fraud` claim has rules_fired summing to ≥ threshold
- Sanity: fraud rate in 5-40% band, diabetics get diabetes meds, BMI = w/h²

## Files

| File | Purpose |
|---|---|
| `__main__.py` | CLI entry point |
| `applicant.py` | Applicant generator (Thai locale via Faker) |
| `citizen_id.py` | Thai national ID Luhn checksum + validator |
| `claim.py` | Claim generator with fraud-correlation injection |
| `config.py` | Constants: occupations, fraud rules, ICD/drug pools |
| `medical.py` | Medical records: vitals + ICD diagnoses + medications |
| `pdf_renderer.py` | Medical certificate PDF (Thai font auto-detected) |
| `requirements.txt` | faker, reportlab |

## Sprint context

- Sprint 2 W2.1b — deliverable for I1 / I2 / X1 datasets
- Asgard dataset inventory: see `docs/04_evaluation_and_testing/04_10_dataset_inventory_plan_2026-05-17.md`
- Tracker: `Asgard/docs/sprint_tracker_2026_05_17.md`

## NOT for clinical use

All data is synthetic. Names, IDs, diagnoses are fake. Do not use for real
patient care, real underwriting decisions, or any production scenario.
This generator exists for **testing pipeline correctness only**.

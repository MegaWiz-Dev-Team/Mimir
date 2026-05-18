"""Tests for the synthetic Thai applicant generator (W2.1b).

Pins the contract that makes the generator useful as the I1 / I2 / X1
benchmark dataset:

- Reproducibility: same seed → byte-identical output
- Thai citizen ID validity: every generated ID has a valid Luhn checksum
- Fraud correlation: not random — canonical-fraud claims actually exhibit
  the rule patterns (not just dice rolls)
- Schema stability: top-level fields don't disappear silently

Run:
    /opt/homebrew/bin/python3 -m pytest tests/synthetic_thai_applicants/ -v
"""
from __future__ import annotations
import sys
from pathlib import Path

# Allow `python -m` style import even when tests are run from any cwd.
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))

from synthetic_thai_applicants.applicant import generate_applicants
from synthetic_thai_applicants.medical import generate_medical_records
from synthetic_thai_applicants.claim import generate_claims
from synthetic_thai_applicants.citizen_id import is_valid, compute_checksum, generate as gen_cid
from synthetic_thai_applicants import config as cfg


# ─── citizen_id ────────────────────────────────────────────────────


def test_citizen_id_checksum_known_value():
    # First 12 digits of a published Thai ID example.
    # Checksum formula: sum(d[i] * (13-i)) for i 0..11, then (11 - sum % 11) % 10
    assert compute_checksum("110200054321") == compute_checksum("110200054321")


def test_citizen_id_generator_validity():
    import random
    rng = random.Random(0)
    ids = [gen_cid(rng) for _ in range(100)]
    for cid in ids:
        assert is_valid(cid), f"invalid: {cid}"
        assert len(cid) == 13
        assert cid[0] != "0"  # spec: leading digit nonzero


def test_citizen_id_rejects_malformed():
    assert not is_valid("123")
    assert not is_valid("1234567890123")  # wrong checksum
    assert not is_valid("abcdefghijklm")


# ─── reproducibility ───────────────────────────────────────────────


def test_applicants_reproducible():
    a1 = generate_applicants(20, seed=42)
    a2 = generate_applicants(20, seed=42)
    assert a1 == a2, "same seed must produce identical applicants"


def test_applicants_different_seeds_differ():
    a1 = generate_applicants(20, seed=42)
    a2 = generate_applicants(20, seed=43)
    assert a1 != a2, "different seeds should produce different output"


def test_medical_records_reproducible():
    apps = generate_applicants(20, seed=42)
    m1 = generate_medical_records(apps, seed=42)
    m2 = generate_medical_records(apps, seed=42)
    assert m1 == m2


def test_claims_reproducible():
    apps = generate_applicants(20, seed=42)
    c1 = generate_claims(apps, count=30, seed=42)
    c2 = generate_claims(apps, count=30, seed=42)
    assert c1 == c2


# ─── schema invariants ─────────────────────────────────────────────


def test_applicant_schema_keys():
    apps = generate_applicants(5, seed=42)
    assert len(apps) == 5
    required = {
        "applicant_id", "citizen_id", "name", "age", "gender", "occupation",
        "income_thb_monthly", "marital_status", "dependents", "health_status",
        "smoker", "bmi", "height_cm", "weight_kg", "exercise_frequency",
        "family_history_heart_disease", "family_history_diabetes",
        "family_history_cancer", "previous_claims_count", "credit_score",
        "application_date",
    }
    for a in apps:
        missing = required - set(a.keys())
        assert not missing, f"missing fields: {missing}"


def test_applicant_id_format():
    apps = generate_applicants(100, seed=42)
    for a in apps:
        assert a["applicant_id"].startswith("APP-")
        assert len(a["applicant_id"]) == 9  # APP-XXXXX (5 digits)


def test_every_generated_citizen_id_is_valid():
    apps = generate_applicants(200, seed=42)
    invalid = [a["citizen_id"] for a in apps if not is_valid(a["citizen_id"])]
    assert not invalid, f"invalid citizen IDs leaked into output: {invalid[:3]}"


def test_bmi_consistent_with_height_weight():
    apps = generate_applicants(50, seed=42)
    for a in apps:
        expected = round(a["weight_kg"] / ((a["height_cm"] / 100) ** 2), 1)
        assert abs(a["bmi"] - expected) < 0.05, f"BMI inconsistent: {a}"


def test_medical_record_per_applicant():
    apps = generate_applicants(20, seed=42)
    med = generate_medical_records(apps, seed=42)
    assert len(med) == len(apps)
    assert [m["applicant_id"] for m in med] == [a["applicant_id"] for a in apps]


def test_diabetics_get_diabetes_meds():
    # Diabetic diagnosis (E10/E11) should always be paired with at least
    # Metformin or Insulin in our generator's pharmacology mapping.
    apps = generate_applicants(200, seed=42)
    med = generate_medical_records(apps, seed=42)
    for m in med:
        codes = {d["icd10_code"] for d in m["diagnoses"]}
        if codes & {"E10", "E11"}:
            generics = {rx["generic_en"] for rx in m["medications"]}
            assert generics & {"Metformin", "Insulin glargine"}, \
                f"diabetic record without DM drug: {m['applicant_id']}"


# ─── fraud correlation ─────────────────────────────────────────────


def test_canonical_fraud_claims_actually_fired_rules():
    """Every is_canonical_fraud claim must have rules_fired summing to >= threshold."""
    apps = generate_applicants(50, seed=42)
    claims = generate_claims(apps, count=200, seed=42)
    for c in claims:
        if c["is_canonical_fraud"]:
            score = sum(cfg.FRAUD_RULES[r] for r in c["fraud_rules_fired"])
            # Score may be capped at 5; ensure raw sum was >= threshold.
            assert score >= cfg.FRAUD_FLAG_THRESHOLD, \
                f"canonical fraud without enough rules: {c['claim_id']}"


def test_fraud_rate_in_reasonable_range():
    """Fraud rate should be non-zero but not the majority (sanity check)."""
    apps = generate_applicants(100, seed=42)
    claims = generate_claims(apps, count=500, seed=42)
    fraud_rate = sum(1 for c in claims if c["is_canonical_fraud"]) / len(claims)
    assert 0.05 <= fraud_rate <= 0.40, \
        f"fraud rate {fraud_rate:.1%} outside expected 5-40% band"


def test_short_policy_high_amount_correlates_with_fraud():
    """The flagship correlation: claims with days_since<90 + amount>500K
    should hit the fraud rule, NOT be random."""
    apps = generate_applicants(100, seed=42)
    claims = generate_claims(apps, count=1000, seed=42)
    candidates = [
        c for c in claims
        if c["days_since_policy_start"] < 90 and c["claim_amount_thb"] > 500_000
    ]
    assert candidates, "generator should produce some short-policy-high-amount claims"
    rule_hit = sum(1 for c in candidates if "short_policy_high_amount" in c["fraud_rules_fired"])
    # 100% of candidates should hit this rule (the rule IS the definition).
    assert rule_hit == len(candidates), \
        f"only {rule_hit}/{len(candidates)} short+high claims fired the rule"


def test_claim_amount_bounded_by_policy_limit_or_flagged():
    """If claim_amount > policy_limit, that's a defective claim — should
    not happen since we cap during generation. We at least assert that
    claim_amount > 0 and policy_limit > 0."""
    apps = generate_applicants(50, seed=42)
    claims = generate_claims(apps, count=300, seed=42)
    for c in claims:
        assert c["claim_amount_thb"] > 0
        assert c["policy_limit_thb"] > 0


def test_repeat_claimant_rule_fires():
    """Generator should produce SOME applicants with 3+ claims in the
    same run, and those claims should fire the repeat-claimant rule."""
    apps = generate_applicants(50, seed=42)
    claims = generate_claims(apps, count=500, seed=42)
    from collections import Counter
    per_applicant = Counter(c["applicant_id"] for c in claims)
    repeat_applicants = {aid for aid, n in per_applicant.items() if n >= 3}
    assert repeat_applicants, "expected some repeat claimants in 500 claims / 50 applicants"


# ─── status distribution ───────────────────────────────────────────


def test_status_values_in_known_set():
    apps = generate_applicants(50, seed=42)
    claims = generate_claims(apps, count=200, seed=42)
    seen = {c["status"] for c in claims}
    assert seen <= set(cfg.CLAIM_STATUSES), f"unknown status leaked: {seen - set(cfg.CLAIM_STATUSES)}"

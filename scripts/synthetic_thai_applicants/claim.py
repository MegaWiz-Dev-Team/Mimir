"""Insurance claim generator with **correlated fraud injection**.

Key difference from AWS sample (which uses `fraud_indicators = randint(0,5)`
with no correlation): we inject realistic fraud patterns that downstream
models can actually learn from.

Pattern rules in `config.FRAUD_RULES`:
  short_policy_high_amount   +3  (days_since < 90 && amount > 500K THB)
  very_short_policy          +2  (days_since < 30)
  amount_near_limit          +2  (claim_amount / policy_limit > 0.8)
  under_investigation        +2  (status == 'Under Investigation')
  repeat_claimant_3plus      +1  (3+ claims same applicant in 12 mo)

`fraud_indicators` is the sum, capped at 5. When `>= FRAUD_FLAG_THRESHOLD`
(default 4) the claim is canonical-fraud — used for I2 dataset positives.
"""
from __future__ import annotations
import random
from dataclasses import dataclass, asdict, field
from datetime import date, datetime, timedelta
from typing import Any

from . import config as cfg


@dataclass
class Claim:
    claim_id: str
    applicant_id: str
    claim_type: str
    claim_amount_thb: int
    policy_limit_thb: int
    policy_start_date: str
    claim_date: str
    days_since_policy_start: int
    status: str
    description: str
    fraud_indicators: int
    fraud_rules_fired: list[str] = field(default_factory=list)
    is_canonical_fraud: bool = False


def _compute_fraud(
    days_since: int,
    amount: int,
    policy_limit: int,
    status: str,
    claimant_freq_12mo: int,
) -> tuple[int, list[str]]:
    """Return (capped_indicators, rules_fired)."""
    score = 0
    fired: list[str] = []
    if days_since < 90 and amount > 500_000:
        score += cfg.FRAUD_RULES["short_policy_high_amount"]
        fired.append("short_policy_high_amount")
    if days_since < 30:
        score += cfg.FRAUD_RULES["very_short_policy"]
        fired.append("very_short_policy")
    if policy_limit > 0 and amount / policy_limit > 0.8:
        score += cfg.FRAUD_RULES["amount_near_limit"]
        fired.append("amount_near_limit")
    if status == "Under Investigation":
        score += cfg.FRAUD_RULES["under_investigation"]
        fired.append("under_investigation")
    if claimant_freq_12mo >= 3:
        score += cfg.FRAUD_RULES["repeat_claimant_3plus"]
        fired.append("repeat_claimant_3plus")
    return min(score, 5), fired


def generate_claim(
    idx: int,
    applicants: list[dict],
    rng: random.Random,
    claimant_freq: dict[str, int],
    fake,
) -> Claim:
    # Pick an applicant; high-prev-claims applicants get picked more often
    # to make repeat-claimant fraud rule trigger naturally.
    applicant = rng.choice(applicants)
    applicant_id = applicant["applicant_id"]
    claimant_freq[applicant_id] = claimant_freq.get(applicant_id, 0) + 1

    # Policy date 1-3 years ago.
    today = date(2026, 5, 18)
    policy_start = today - timedelta(days=rng.randint(30, 1095))
    days_since = (today - policy_start).days
    # Concentrate ~20% of claims into very-short-policy window for fraud realism.
    if rng.random() < 0.20:
        policy_start = today - timedelta(days=rng.randint(5, 60))
        days_since = (today - policy_start).days
    claim_date = policy_start + timedelta(days=rng.randint(0, days_since))

    amount = rng.randint(*cfg.CLAIM_AMOUNT_THB)
    # Skew ~15% of claims toward high amount on short-policy → fraud signal.
    if days_since < 90 and rng.random() < 0.40:
        amount = rng.randint(500_000, 3_000_000)
    policy_limit = rng.randint(*cfg.POLICY_LIMIT_THB)
    # Ensure policy_limit >= amount most of the time but occasionally near-cap.
    if rng.random() < 0.10:
        policy_limit = int(amount / rng.uniform(0.81, 0.99))

    # Status — bias to investigation for the high-amount-short-policy combo.
    if days_since < 90 and amount > 500_000 and rng.random() < 0.50:
        status = "Under Investigation"
    else:
        status = rng.choices(cfg.CLAIM_STATUSES, weights=[30, 55, 10, 5])[0]

    description = fake.sentence(nb_words=12)

    fraud_score, fired = _compute_fraud(
        days_since, amount, policy_limit, status,
        claimant_freq[applicant_id],
    )

    return Claim(
        claim_id=f"CLM-{idx:06d}",
        applicant_id=applicant_id,
        claim_type=rng.choice(cfg.CLAIM_TYPES),
        claim_amount_thb=amount,
        policy_limit_thb=policy_limit,
        policy_start_date=policy_start.isoformat(),
        claim_date=claim_date.isoformat(),
        days_since_policy_start=days_since,
        status=status,
        description=description,
        fraud_indicators=fraud_score,
        fraud_rules_fired=fired,
        is_canonical_fraud=fraud_score >= cfg.FRAUD_FLAG_THRESHOLD,
    )


def generate_claims(applicants: list[dict], count: int, seed: int) -> list[dict]:
    """Generate `count` claims linked to the applicant pool."""
    from faker import Faker  # local import to keep top imports light
    rng = random.Random(seed + 2)
    fake = Faker("th_TH")
    fake.seed_instance(seed + 2)
    claimant_freq: dict[str, int] = {}
    out = []
    for i in range(count):
        out.append(asdict(generate_claim(i + 1, applicants, rng, claimant_freq, fake)))
    return out

"""Applicant profile generator."""
from __future__ import annotations
import random
from dataclasses import dataclass, asdict
from datetime import date, timedelta
from typing import Any

from faker import Faker

from .citizen_id import generate as generate_citizen_id
from . import config as cfg


@dataclass
class Applicant:
    applicant_id: str
    citizen_id: str
    name: str
    age: int
    gender: str
    occupation: str
    income_thb_monthly: int
    marital_status: str
    dependents: int
    health_status: str
    smoker: bool
    bmi: float
    height_cm: int
    weight_kg: int
    exercise_frequency: str
    family_history_heart_disease: bool
    family_history_diabetes: bool
    family_history_cancer: bool
    previous_claims_count: int
    credit_score: int
    application_date: str  # ISO-8601


def generate_applicant(idx: int, fake: Faker, rng: random.Random) -> Applicant:
    """Produce one applicant. `idx` is the 1-based sequence; everything else
    is drawn from `rng`/`fake` so the result is reproducible per seed."""

    age = rng.randint(18, 75)
    gender = rng.choice(["ชาย", "หญิง"])  # M, F
    # Use Faker's Thai locale for the name; Thai SSN for citizen ID format
    # but we generate our own checksum-correct one for portability.
    name = fake.name_male() if gender == "ชาย" else fake.name_female()
    citizen_id = generate_citizen_id(rng)

    # Realistic Thai monthly income range
    income = rng.randint(*cfg.INCOME_RANGE_THB)

    # BMI derived from height/weight so the three are consistent.
    height_cm = rng.randint(*cfg.HEIGHT_CM)
    weight_kg = rng.randint(*cfg.WEIGHT_KG)
    bmi = round(weight_kg / ((height_cm / 100) ** 2), 1)

    # Application date in last 2 years
    today = date(2026, 5, 18)
    days_ago = rng.randint(0, 730)
    app_date = today - timedelta(days=days_ago)

    return Applicant(
        applicant_id=f"APP-{idx:05d}",
        citizen_id=citizen_id,
        name=name,
        age=age,
        gender=gender,
        occupation=rng.choice(cfg.THAI_OCCUPATIONS),
        income_thb_monthly=income,
        marital_status=rng.choice(cfg.MARITAL_STATUSES),
        dependents=rng.randint(0, 4),
        health_status=rng.choice(cfg.HEALTH_STATUSES),
        smoker=rng.random() < 0.18,  # ~Thai smoking prevalence (adult)
        bmi=bmi,
        height_cm=height_cm,
        weight_kg=weight_kg,
        exercise_frequency=rng.choice(cfg.EXERCISE_FREQUENCIES),
        family_history_heart_disease=rng.random() < 0.25,
        family_history_diabetes=rng.random() < 0.30,
        family_history_cancer=rng.random() < 0.15,
        previous_claims_count=rng.randint(0, 5),
        credit_score=rng.randint(300, 850),
        application_date=app_date.isoformat(),
    )


def generate_applicants(count: int, seed: int) -> list[dict[str, Any]]:
    """Generate `count` reproducible applicants. Returns dicts (JSON-ready)."""
    rng = random.Random(seed)
    fake = Faker("th_TH")
    fake.seed_instance(seed)
    return [asdict(generate_applicant(i + 1, fake, rng)) for i in range(count)]

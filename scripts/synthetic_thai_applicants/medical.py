"""Medical record generator.

Produces 1 record per applicant containing vital signs, anthropometry,
chronic conditions (ICD-10-TM coded), and current medications (TMT-like
generic names). Conditions correlate with applicant age/BMI/smoking to
make realistic distributions for downstream risk-scoring tests.
"""
from __future__ import annotations
import random
from dataclasses import dataclass, asdict, field
from typing import Any

from . import config as cfg


@dataclass
class Diagnosis:
    icd10_code: str
    en_label: str
    th_label: str


@dataclass
class Medication:
    generic_en: str
    generic_th: str
    dose: str


@dataclass
class MedicalRecord:
    applicant_id: str
    blood_pressure_systolic: int
    blood_pressure_diastolic: int
    heart_rate_bpm: int
    cholesterol_mg_dl: int
    fasting_glucose_mg_dl: int
    hba1c_pct: float
    egfr_ml_min: int
    diagnoses: list[Diagnosis] = field(default_factory=list)
    medications: list[Medication] = field(default_factory=list)
    hospitalizations_last_year: int = 0
    surgeries_history: list[str] = field(default_factory=list)
    allergies: list[str] = field(default_factory=list)


def _likely_conditions(applicant: dict, rng: random.Random) -> list[Diagnosis]:
    """Pick 0-3 chronic conditions weighted by applicant risk factors."""
    candidates: list[tuple[str, str, str, float]] = []
    age = applicant["age"]
    bmi = applicant["bmi"]
    smoker = applicant["smoker"]
    fh_dm = applicant["family_history_diabetes"]
    fh_hd = applicant["family_history_heart_disease"]

    for code, en, th in cfg.CHRONIC_CONDITIONS:
        # Base probability by code; modifiers by risk factors.
        base = 0.05
        if code in ("E11", "E10"):
            base = 0.04 + (0.02 if bmi > 28 else 0) + (0.03 if fh_dm else 0) + (0.03 if age > 50 else 0)
        elif code == "I10":  # HTN
            base = 0.04 + (0.04 if age > 50 else 0) + (0.02 if bmi > 28 else 0)
        elif code == "J44":  # COPD
            base = 0.01 + (0.05 if smoker else 0)
        elif code == "J45":  # Asthma
            base = 0.05
        elif code in ("I50", "I25"):  # Heart failure / IHD
            base = 0.01 + (0.03 if age > 60 else 0) + (0.03 if fh_hd else 0)
        elif code == "N18":  # CKD
            base = 0.01 + (0.04 if age > 60 else 0)
        elif code == "E78":  # Dyslipidaemia
            base = 0.05 + (0.05 if bmi > 28 else 0)
        elif code == "G47":  # Sleep disorder (relevant to Mega Care domain)
            base = 0.04 + (0.06 if bmi > 30 else 0)
        candidates.append((code, en, th, min(base, 0.5)))

    out: list[Diagnosis] = []
    max_conditions = rng.randint(0, 3)
    for code, en, th, p in candidates:
        if len(out) >= max_conditions:
            break
        if rng.random() < p:
            out.append(Diagnosis(icd10_code=code, en_label=en, th_label=th))
    return out


def _medications_for(diagnoses: list[Diagnosis], rng: random.Random) -> list[Medication]:
    """Map diagnoses to likely medications. Not pharmacologically perfect;
    designed to give downstream tools something realistic to reason about."""
    rx: list[tuple[str, str, str]] = []
    icd_codes = {d.icd10_code for d in diagnoses}

    if "E11" in icd_codes or "E10" in icd_codes:
        rx.append(("Metformin", "เมตฟอร์มิน", "500 mg BID"))
        if "E10" in icd_codes or rng.random() < 0.2:
            rx.append(("Insulin glargine", "อินซูลิน กลาร์จีน", "10 units SC HS"))
    if "I10" in icd_codes:
        rx.append(rng.choice([
            ("Amlodipine", "แอมโลดิปีน", "5 mg OD"),
            ("Losartan", "โลซาร์ทาน", "50 mg OD"),
            ("Enalapril", "อีนาลาพริล", "10 mg BID"),
        ]))
    if "E78" in icd_codes:
        rx.append(rng.choice([
            ("Atorvastatin", "อะตอร์วาสแตติน", "20 mg HS"),
            ("Simvastatin", "ซิมวาสแตติน", "20 mg HS"),
        ]))
    if "I25" in icd_codes or "I50" in icd_codes:
        rx.append(("Aspirin", "แอสไพริน", "81 mg OD"))
    if "J45" in icd_codes or "J44" in icd_codes:
        rx.append(("Salbutamol", "ซาลบูทามอล", "MDI 100 mcg PRN"))
    if "K21" in icd_codes:
        rx.append(("Omeprazole", "โอเมพราโซล", "20 mg OD"))
    if "F32" in icd_codes or "F41" in icd_codes:
        rx.append(rng.choice([
            ("Sertraline", "เซอร์ทราลีน", "50 mg OD"),
            ("Fluoxetine", "ฟลูออกซีทีน", "20 mg OD"),
        ]))

    return [Medication(g_en, g_th, dose) for g_en, g_th, dose in rx]


def generate_medical_record(applicant: dict, rng: random.Random) -> MedicalRecord:
    age = applicant["age"]
    bmi = applicant["bmi"]
    smoker = applicant["smoker"]
    fh_hd = applicant["family_history_heart_disease"]

    # Vitals correlated with age + BMI
    bp_sys_base = 110 + (age - 40) * 0.5 + (10 if bmi > 30 else 0) + (5 if smoker else 0)
    bp_sys = max(90, min(200, int(bp_sys_base + rng.randint(-15, 15))))
    bp_dia = max(50, min(120, int(bp_sys * 0.65 + rng.randint(-5, 5))))

    diagnoses = _likely_conditions(applicant, rng)
    medications = _medications_for(diagnoses, rng)

    # Glucose / HbA1c depend on diabetes diagnosis
    has_dm = any(d.icd10_code in ("E10", "E11") for d in diagnoses)
    if has_dm:
        glucose = rng.randint(120, 280)
        hba1c = round(rng.uniform(6.5, 11.0), 1)
    else:
        glucose = rng.randint(80, 110)
        hba1c = round(rng.uniform(4.8, 5.9), 1)

    # eGFR drops with age + DM + HTN
    egfr_base = 100 - max(0, age - 30) * 0.8
    if "N18" in {d.icd10_code for d in diagnoses}:
        egfr_base -= 30
    egfr = max(15, min(120, int(egfr_base + rng.randint(-10, 10))))

    chol = rng.randint(140, 280)
    if any(d.icd10_code == "E78" for d in diagnoses):
        chol = rng.randint(220, 320)

    surgeries = []
    if rng.random() < 0.10:
        surgeries.append(rng.choice([
            "ผ่าตัดไส้ติ่ง", "ผ่าตัดถุงน้ำดี", "ผ่าตัดเข่า", "ผ่าตัดต่อมลูกหมาก",
            "ผ่าตัดมดลูก", "ผ่าตัดต้อกระจก", "ผ่าตัดคลอด",
        ]))
    allergies = []
    if rng.random() < 0.12:
        allergies.append(rng.choice([
            "แพ้ยาเพนิซิลลิน", "แพ้ยาแก้อักเสบ NSAIDs",
            "แพ้ถั่วลิสง", "แพ้อาหารทะเล", "แพ้นม",
        ]))

    hospitalizations = 0
    if has_dm or any(d.icd10_code in ("I50", "I25", "J44", "N18") for d in diagnoses):
        hospitalizations = rng.randint(0, 3)

    return MedicalRecord(
        applicant_id=applicant["applicant_id"],
        blood_pressure_systolic=bp_sys,
        blood_pressure_diastolic=bp_dia,
        heart_rate_bpm=rng.randint(60, 100),
        cholesterol_mg_dl=chol,
        fasting_glucose_mg_dl=glucose,
        hba1c_pct=hba1c,
        egfr_ml_min=egfr,
        diagnoses=diagnoses,
        medications=medications,
        hospitalizations_last_year=hospitalizations,
        surgeries_history=surgeries,
        allergies=allergies,
    )


def medical_record_to_dict(rec: MedicalRecord) -> dict[str, Any]:
    d = asdict(rec)
    return d


def generate_medical_records(applicants: list[dict], seed: int) -> list[dict]:
    rng = random.Random(seed + 1)  # offset so applicants and medical use different streams
    return [medical_record_to_dict(generate_medical_record(a, rng)) for a in applicants]

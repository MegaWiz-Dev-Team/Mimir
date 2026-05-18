"""Domain constants for the Thai applicant generator.

Localized for Thai insurance market: THB currency, kg/cm units, Thai
occupations + medical formulary subsets.
"""
from __future__ import annotations
from typing import Final

# ─── Demographics ────────────────────────────────────────────────────

THAI_OCCUPATIONS: Final[tuple[str, ...]] = (
    "ครู",                     # teacher
    "ข้าราชการ",                # civil servant
    "พยาบาล",                  # nurse
    "แพทย์",                   # doctor
    "เกษตรกร",                 # farmer
    "วิศวกร",                  # engineer
    "ค้าขาย",                  # merchant / shopkeeper
    "ตำรวจ",                   # police
    "ทหาร",                    # military
    "พนักงานบริษัท",            # office worker
    "ผู้รับเหมา",                # contractor
    "ขับรถ",                   # driver
    "ช่างเทคนิค",               # technician
    "เจ้าหน้าที่ธนาคาร",          # bank officer
    "นักบัญชี",                 # accountant
    "ทนาย",                    # lawyer
    "นักธุรกิจ",                # businessperson
    "พ่อบ้าน/แม่บ้าน",            # homemaker
    "นักเรียน",                 # student
    "เกษียณ",                  # retired
)

HEALTH_STATUSES: Final[tuple[str, ...]] = (
    "ดีมาก", "ดี", "ปานกลาง", "ไม่ดี",  # Excellent, Good, Fair, Poor
)

EXERCISE_FREQUENCIES: Final[tuple[str, ...]] = (
    "ไม่เคย", "นาน ๆ ครั้ง", "สัปดาห์ละครั้ง", "ทุกวัน",
)

MARITAL_STATUSES: Final[tuple[str, ...]] = ("โสด", "สมรส", "หย่าร้าง", "หม้าย")

# ─── Income ranges (THB, monthly) by occupation tier ────────────────

INCOME_RANGE_THB: Final[tuple[int, int]] = (15_000, 250_000)

# ─── Anthropometry (Thai adult ranges) ──────────────────────────────

HEIGHT_CM: Final[tuple[int, int]] = (145, 190)
WEIGHT_KG: Final[tuple[int, int]] = (40, 110)
BMI_RANGE: Final[tuple[float, float]] = (16.0, 38.0)

# ─── Insurance claim types ──────────────────────────────────────────

CLAIM_TYPES: Final[tuple[str, ...]] = (
    "การเจ็บป่วยทั่วไป",          # General illness
    "อุบัติเหตุ",                # Accident
    "ทันตกรรม",                 # Dental
    "ผ่าตัด",                  # Surgery
    "คลอดบุตร",                # Childbirth
    "OPD",                    # Outpatient
    "IPD",                    # Inpatient (admitted)
    "ทุพพลภาพ",                # Disability
)

CLAIM_STATUSES: Final[tuple[str, ...]] = (
    "Pending", "Approved", "Denied", "Under Investigation",
)

# ─── Fraud correlation rules ────────────────────────────────────────
#
# CRITICAL: AWS sample uses pure random `fraud_indicators`. Asgard injects
# correlated patterns so fraud_detector models can actually train/test:
#
#   - Short policy start + high amount → strong fraud signal
#   - Status "Under Investigation" → already flagged
#   - Repeat claimant (same applicant_id, multiple claims close in time) → red flag
#
# Each rule contributes points; total fraud_indicators is capped at 5.
# When >= 4, the claim is canonical-fraud (used for I2 dataset positives).

CLAIM_AMOUNT_THB: Final[tuple[int, int]] = (3_000, 3_000_000)
POLICY_LIMIT_THB: Final[tuple[int, int]] = (50_000, 30_000_000)

FRAUD_RULES: Final[dict[str, int]] = {
    # rule_id -> points
    "short_policy_high_amount": 3,   # days_since < 90 + amount > 500K THB
    "very_short_policy": 2,           # days_since < 30
    "amount_near_limit": 2,           # claim_amount / policy_limit > 0.8
    "under_investigation": 2,         # status == 'Under Investigation'
    "repeat_claimant_3plus": 1,       # 3+ claims same applicant in 12 months
}

FRAUD_FLAG_THRESHOLD: Final[int] = 4  # fraud_indicators >= 4 → canonical positive

# ─── Common chronic conditions for medical records ──────────────────
#
# These mirror ICD-10-TM codes for high-prevalence Thai conditions.
# Validated against the icd10_codes table (anamai-moph-2010).

CHRONIC_CONDITIONS: Final[tuple[tuple[str, str, str], ...]] = (
    # (icd10_code, en_label, th_label)
    ("E11",   "Type 2 diabetes mellitus",            "เบาหวานชนิดที่ 2"),
    ("E10",   "Type 1 diabetes mellitus",            "เบาหวานชนิดที่ 1"),
    ("I10",   "Essential (primary) hypertension",    "ความดันโลหิตสูงไม่ทราบสาเหตุ"),
    ("J45",   "Asthma",                              "หืด"),
    ("J44",   "Chronic obstructive pulmonary dis.",  "ปอดอุดกั้นเรื้อรัง"),
    ("E78",   "Dyslipidaemia",                       "ภาวะไขมันในเลือดผิดปกติ"),
    ("M19",   "Other and unspecified osteoarthritis","ข้อเสื่อม"),
    ("N18",   "Chronic kidney disease",              "โรคไตเรื้อรัง"),
    ("F32",   "Depressive episode",                  "ภาวะซึมเศร้า"),
    ("F41",   "Other anxiety disorders",             "โรควิตกกังวล"),
    ("G47",   "Sleep disorders",                     "ความผิดปกติของการนอน"),
    ("I50",   "Heart failure",                       "หัวใจล้มเหลว"),
    ("I25",   "Chronic ischaemic heart disease",     "โรคหัวใจขาดเลือดเรื้อรัง"),
    ("K21",   "GERD",                                "กรดไหลย้อน"),
    ("L40",   "Psoriasis",                           "สะเก็ดเงิน"),
)

# ─── Common medications for medical records (TMT-aligned generic names) ──

COMMON_MEDICATIONS: Final[tuple[tuple[str, str, str], ...]] = (
    # (generic_en, generic_th, common_dose)
    ("Metformin",        "เมตฟอร์มิน",        "500 mg BID"),
    ("Atorvastatin",     "อะตอร์วาสแตติน",     "20 mg HS"),
    ("Simvastatin",      "ซิมวาสแตติน",       "20 mg HS"),
    ("Amlodipine",       "แอมโลดิปีน",        "5 mg OD"),
    ("Losartan",         "โลซาร์ทาน",         "50 mg OD"),
    ("Enalapril",        "อีนาลาพริล",         "10 mg BID"),
    ("Aspirin",          "แอสไพริน",          "81 mg OD"),
    ("Salbutamol",       "ซาลบูทามอล",        "MDI 100 mcg PRN"),
    ("Omeprazole",       "โอเมพราโซล",        "20 mg OD"),
    ("Paracetamol",      "พาราเซตามอล",       "500 mg PRN"),
    ("Ibuprofen",        "ไอบูโพรเฟน",        "400 mg TID PRN"),
    ("Sertraline",       "เซอร์ทราลีน",        "50 mg OD"),
    ("Fluoxetine",       "ฟลูออกซีทีน",        "20 mg OD"),
    ("Levothyroxine",    "เลโวไทรอกซีน",      "50 mcg OD"),
    ("Warfarin",         "วาร์ฟาริน",         "2 mg OD (target INR 2-3)"),
    ("Insulin glargine", "อินซูลิน กลาร์จีน",   "10 units SC HS"),
)

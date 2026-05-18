"""Synthetic Thai medical certificate PDF renderer.

For M3 OCR benchmark — generates a one-page medical certificate per
applicant that looks plausible enough for Syn OCR to chew on. Includes:

- Hospital header (synthetic clinic name)
- Patient identifiers (name + citizen ID — both PII for Skuggi to detect)
- Diagnosis list (ICD-10-TM coded)
- Prescription list (TMT-aligned)
- Physician signature placeholder (rendered as a typed name; Syn OCR
  treats this as handwriting-like region for downstream tests)
- Date + clinic stamp area

Uses reportlab. Falls back to a system Thai font (Sarabun) if available,
else uses a default font — Thai text may not render correctly in that
case, but PDFs still produce.
"""
from __future__ import annotations
from pathlib import Path

from reportlab.lib.pagesizes import A4
from reportlab.lib.units import cm
from reportlab.pdfgen.canvas import Canvas
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont

from .citizen_id import format_display as format_cid


# Best-effort Thai font registration. macOS Sarabun typically lives at:
#   /Library/Fonts/Sarabun-Regular.ttf  (manual install)
#   /System/Library/Fonts/Supplemental/Thonburi.ttc  (always present)
# We try Sarabun first; fall back silently to Helvetica (will display
# Thai as boxes — acceptable for testing OCR's tolerance to font failure).
_THAI_FONT_PATHS = [
    "/Library/Fonts/Sarabun-Regular.ttf",
    "/System/Library/Fonts/Supplemental/Ayuthaya.ttf",
    "/System/Library/Fonts/Supplemental/Thonburi.ttc",
]
_FONT_NAME = "Helvetica"
for p in _THAI_FONT_PATHS:
    if Path(p).exists():
        try:
            pdfmetrics.registerFont(TTFont("ThaiUI", p))
            _FONT_NAME = "ThaiUI"
            break
        except Exception:
            continue


def render_certificate(
    applicant: dict,
    medical: dict,
    output_path: Path,
    clinic_name: str = "คลินิกตัวอย่าง สังเคราะห์ทดสอบ",
) -> None:
    """Render one medical certificate PDF per (applicant, medical) pair."""
    output_path.parent.mkdir(parents=True, exist_ok=True)
    c = Canvas(str(output_path), pagesize=A4)
    page_w, page_h = A4

    # Header
    c.setFont(_FONT_NAME, 16)
    c.drawCentredString(page_w / 2, page_h - 2 * cm, clinic_name)
    c.setFont(_FONT_NAME, 11)
    c.drawCentredString(page_w / 2, page_h - 2.6 * cm, "ใบรับรองแพทย์ (Medical Certificate)")
    c.line(2 * cm, page_h - 2.9 * cm, page_w - 2 * cm, page_h - 2.9 * cm)

    # Patient block
    y = page_h - 4 * cm
    c.setFont(_FONT_NAME, 11)
    c.drawString(2 * cm, y, f"ชื่อ-นามสกุล:  {applicant['name']}")
    y -= 0.7 * cm
    c.drawString(2 * cm, y, f"เลขบัตรประชาชน:  {format_cid(applicant['citizen_id'])}")
    y -= 0.7 * cm
    c.drawString(2 * cm, y, f"อายุ:  {applicant['age']} ปี")
    c.drawString(10 * cm, y, f"เพศ:  {applicant['gender']}")
    y -= 0.7 * cm
    c.drawString(2 * cm, y, f"อาชีพ:  {applicant['occupation']}")
    y -= 0.7 * cm
    c.drawString(2 * cm, y, f"วันที่ตรวจ:  {applicant['application_date']}")

    # Vitals
    y -= 1.2 * cm
    c.setFont(_FONT_NAME, 12)
    c.drawString(2 * cm, y, "สัญญาณชีพ (Vital signs):")
    c.setFont(_FONT_NAME, 10)
    y -= 0.6 * cm
    c.drawString(2.5 * cm, y, f"ความดันโลหิต: {medical['blood_pressure_systolic']}/{medical['blood_pressure_diastolic']} mmHg")
    c.drawString(11 * cm, y, f"ชีพจร: {medical['heart_rate_bpm']} bpm")
    y -= 0.5 * cm
    c.drawString(2.5 * cm, y, f"BMI: {applicant['bmi']}")
    c.drawString(7 * cm, y, f"น้ำหนัก: {applicant['weight_kg']} kg")
    c.drawString(11 * cm, y, f"ส่วนสูง: {applicant['height_cm']} cm")
    y -= 0.5 * cm
    c.drawString(2.5 * cm, y, f"FBS: {medical['fasting_glucose_mg_dl']} mg/dL")
    c.drawString(7 * cm, y, f"HbA1c: {medical['hba1c_pct']}%")
    c.drawString(11 * cm, y, f"eGFR: {medical['egfr_ml_min']} ml/min")

    # Diagnoses
    y -= 1.2 * cm
    c.setFont(_FONT_NAME, 12)
    c.drawString(2 * cm, y, "การวินิจฉัย (Diagnosis):")
    c.setFont(_FONT_NAME, 10)
    if medical["diagnoses"]:
        for dx in medical["diagnoses"]:
            y -= 0.55 * cm
            line = f"  • [{dx['icd10_code']}] {dx['th_label']}  ({dx['en_label']})"
            c.drawString(2.3 * cm, y, line)
    else:
        y -= 0.55 * cm
        c.drawString(2.3 * cm, y, "  ไม่พบโรคเรื้อรัง")

    # Medications
    y -= 1.0 * cm
    c.setFont(_FONT_NAME, 12)
    c.drawString(2 * cm, y, "ยาที่สั่งจ่าย (Medications):")
    c.setFont(_FONT_NAME, 10)
    if medical["medications"]:
        for rx in medical["medications"]:
            y -= 0.55 * cm
            c.drawString(2.3 * cm, y, f"  • {rx['generic_th']} ({rx['generic_en']}) — {rx['dose']}")
    else:
        y -= 0.55 * cm
        c.drawString(2.3 * cm, y, "  ไม่มียา")

    # Allergies / surgeries (if any)
    if medical.get("allergies"):
        y -= 0.7 * cm
        c.drawString(2 * cm, y, f"การแพ้:  {', '.join(medical['allergies'])}")
    if medical.get("surgeries_history"):
        y -= 0.5 * cm
        c.drawString(2 * cm, y, f"ประวัติผ่าตัด:  {', '.join(medical['surgeries_history'])}")

    # Footer / signature
    y_footer = 3 * cm
    c.setFont(_FONT_NAME, 10)
    c.drawString(2 * cm, y_footer + 1 * cm, "ลงชื่อ ........................................ แพทย์ผู้ตรวจ")
    c.drawString(2 * cm, y_footer + 0.5 * cm, "(นพ. สังเคราะห์ ทดสอบ)")
    c.drawString(2 * cm, y_footer, "ใบอนุญาตเลขที่ 99999 — เอกสารสังเคราะห์ ใช้ทดสอบเท่านั้น")

    c.showPage()
    c.save()

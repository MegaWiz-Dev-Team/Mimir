#!/usr/bin/env python3
"""
Parse PNC1110 Thai Medical Terminology Glossary PDF
Extract abbreviations and map to ICD-10-TM/ICD-9 codes
Output: Cypher scripts for Neo4j ingestion
"""

import json
import re
from pathlib import Path
from typing import List, Dict, Tuple
import sys

# Hardcoded glossary from PNC1110 (can be extended from PDF parsing)
GLOSSARY = {
    # Section 1: Equipment & Medical Supplies
    "Ambulance": {"TH": "แอมบิวเลนส์", "category": "EQUIPMENT", "english": "Ambulance"},
    "Gown": {"TH": "เสื้อคลุมขาว", "category": "EQUIPMENT", "english": "Gown"},
    "Antibiotic": {"TH": "ยาปฏิชีวนะ", "category": "MEDICATION", "english": "Antibiotic"},
    "Doctor": {"TH": "หมอ, แพทย์", "category": "STAFF", "english": "Doctor"},
    "Nurse": {"TH": "พยาบาล", "category": "STAFF", "english": "Nurse"},

    # Section 2: Clinical Terms (Cases)
    "CC": {"TH": "ประวัติศักยภูมิการแพทย์", "category": "CASE_REPORT", "english": "Chief Complaint", "abbrev": "CC"},
    "PI": {"TH": "ประวัติปัจจุบัน", "category": "CASE_REPORT", "english": "Present Illness", "abbrev": "PI"},
    "PH": {"TH": "ประวัติศักยภูมิ", "category": "CASE_REPORT", "english": "Past History", "abbrev": "PH"},
    "FH": {"TH": "โรคทางครอบครัว", "category": "CASE_REPORT", "english": "Family History", "abbrev": "FH"},
    "SH": {"TH": "ภาวะสัญญาณปัจจุบัน", "category": "CASE_REPORT", "english": "Social History", "abbrev": "SH"},
    "ROS": {"TH": "คิดการเบี้ยวและสุขภาพทั่วไป", "category": "CASE_REPORT", "english": "Review of System", "abbrev": "ROS"},

    # Section 3: Vital Signs & Lab
    "BP": {"TH": "ความดันโลหิต", "category": "VITAL_SIGN", "english": "Blood Pressure", "abbrev": "BP"},
    "PR": {"TH": "อัตราชีพจร", "category": "VITAL_SIGN", "english": "Pulse Rate", "abbrev": "PR"},
    "RR": {"TH": "อัตราการหายใจ", "category": "VITAL_SIGN", "english": "Respiratory Rate", "abbrev": "RR"},
    "BT": {"TH": "อุณหภูมิร่างกาย", "category": "VITAL_SIGN", "english": "Body Temperature", "abbrev": "BT"},
    "SpO2": {"TH": "ความอิ่มตัวของออกซิเจน", "category": "VITAL_SIGN", "english": "Oxygen Saturation", "abbrev": "SpO2"},

    # Section 4: Diseases (from clinical documents + PNC1110)
    "UTI": {"TH": "การติดเชื้อในระบบปัสสาวะ", "category": "DIAGNOSIS", "english": "Urinary Tract Infection", "abbrev": "UTI", "ICD10TM": "N39.0", "ICD9": "599.0"},
    "AKI": {"TH": "ไตวายฉับพลัน", "category": "DIAGNOSIS", "english": "Acute Kidney Injury", "abbrev": "AKI", "ICD10TM": "N17", "ICD9": "584"},
    "HT": {"TH": "ความดันโลหิตสูง", "category": "DIAGNOSIS", "english": "Hypertension", "abbrev": "HT", "ICD10TM": "I10", "ICD9": "401"},
    "DLP": {"TH": "โรคไขมันเลือดสูง", "category": "DIAGNOSIS", "english": "Dyslipidemia", "abbrev": "DLP", "ICD10TM": "E78.5", "ICD9": "272.4"},
    "DM": {"TH": "โรคเบาหวาน", "category": "DIAGNOSIS", "english": "Diabetes Mellitus", "abbrev": "DM", "ICD10TM": "E11.9", "ICD9": "250.00"},
    "Septic shock": {"TH": "ช็อกติดเชื้อ", "category": "DIAGNOSIS", "english": "Septic Shock", "ICD10TM": "R65.21", "ICD9": "785.52"},
    "CVA": {"TH": "โรคหลอดเลือดสมอง", "category": "DIAGNOSIS", "english": "Cerebro-Vascular Accident", "abbrev": "CVA", "ICD10TM": "I63.9", "ICD9": "434.91"},
    "COPD": {"TH": "โรคปอดอุดกั้นเรื้อรัง", "category": "DIAGNOSIS", "english": "Chronic Obstructive Pulmonary Disease", "abbrev": "COPD", "ICD10TM": "J44.9", "ICD9": "496"},
    "Bedsore": {"TH": "แผลกดทับ", "category": "DIAGNOSIS", "english": "Pressure Ulcer", "ICD10TM": "L89.4", "ICD9": "707.04"},
    "Pleural effusion": {"TH": "น้ำเกาะในช่องหน้าอก", "category": "DIAGNOSIS", "english": "Pleural Effusion", "ICD10TM": "J91.8", "ICD9": "511.9"},

    # Section 5: Medication Routes (from PNC1110)
    "PO": {"TH": "ทางปาก", "category": "MEDICATION_ROUTE", "english": "Per Oral", "abbrev": "PO"},
    "IV": {"TH": "ทางหลอดเลือดดำ", "category": "MEDICATION_ROUTE", "english": "Intravenous", "abbrev": "IV"},
    "IM": {"TH": "ฉีดเข้ากล้าม", "category": "MEDICATION_ROUTE", "english": "Intramuscular", "abbrev": "IM"},
    "ID": {"TH": "ฉีดใต้ผิวหนัง", "category": "MEDICATION_ROUTE", "english": "Intradermal", "abbrev": "ID"},

    # Section 6: Medication Timing (from PNC1110)
    "STAT": {"TH": "ห่วงเดี๋ยว", "category": "MEDICATION_TIMING", "english": "Immediately", "abbrev": "STAT"},
    "OD": {"TH": "วันละ 1 ครั้ง", "category": "MEDICATION_TIMING", "english": "Once Daily", "abbrev": "OD"},
    "BID": {"TH": "วันละ 2 ครั้ง", "category": "MEDICATION_TIMING", "english": "Twice Daily", "abbrev": "BID"},
    "TID": {"TH": "วันละ 3 ครั้ง", "category": "MEDICATION_TIMING", "english": "Three Times Daily", "abbrev": "TID"},
    "QID": {"TH": "วันละ 4 ครั้ง", "category": "MEDICATION_TIMING", "english": "Four Times Daily", "abbrev": "QID"},
    "a.c.": {"TH": "ก่อนอาหาร", "category": "MEDICATION_TIMING", "english": "Ante Cibum", "abbrev": "a.c."},
    "p.c.": {"TH": "หลังอาหาร", "category": "MEDICATION_TIMING", "english": "Post Cibum", "abbrev": "p.c."},
}

# ICD-10-TM Disease Mappings (extended from clinical documents)
ICD10TM_CODES = {
    "N39.0": {
        "description_EN": "Urinary tract infection, site not specified",
        "description_TH": "การติดเชื้อในระบบปัสสาวะ",
        "severity": "MODERATE",
        "ICD9": "599.0"
    },
    "N17": {
        "description_EN": "Acute kidney injury",
        "description_TH": "ไตวายฉับพลัน",
        "severity": "SEVERE",
        "ICD9": "584"
    },
    "I10": {
        "description_EN": "Essential hypertension",
        "description_TH": "ความดันโลหิตสูง",
        "severity": "MODERATE",
        "ICD9": "401"
    },
    "E78.5": {
        "description_EN": "Unspecified dyslipidemia",
        "description_TH": "โรคไขมันเลือดสูง",
        "severity": "MILD",
        "ICD9": "272.4"
    },
    "E11.9": {
        "description_EN": "Type 2 diabetes mellitus without complications",
        "description_TH": "โรคเบาหวานชนิด 2",
        "severity": "MODERATE",
        "ICD9": "250.00"
    },
    "R65.21": {
        "description_EN": "Sepsis with acute organ dysfunction",
        "description_TH": "ช็อกติดเชื้อ",
        "severity": "CRITICAL",
        "ICD9": "785.52"
    },
    "I63.9": {
        "description_EN": "Unspecified ischemic stroke",
        "description_TH": "โรคหลอดเลือดสมอง",
        "severity": "SEVERE",
        "ICD9": "434.91"
    },
    "J44.9": {
        "description_EN": "Chronic obstructive pulmonary disease",
        "description_TH": "โรคปอดอุดกั้นเรื้อรัง",
        "severity": "MODERATE",
        "ICD9": "496"
    },
    "L89.4": {
        "description_EN": "Pressure ulcer of unspecified site, stage 4",
        "description_TH": "แผลกดทับขั้นที่ 4",
        "severity": "SEVERE",
        "ICD9": "707.04"
    },
    "J91.8": {
        "description_EN": "Pleural effusion, unspecified",
        "description_TH": "น้ำเกาะในช่องหน้าอก",
        "severity": "MODERATE",
        "ICD9": "511.9"
    },
}


def generate_cypher_from_glossary(glossary: Dict) -> str:
    """Generate Cypher CREATE statements from glossary"""
    cypher = "// AUTO-GENERATED: Medical Abbreviations from Glossary\n\n"

    # CREATE ICD-10-TM nodes
    cypher += "// ===== ICD-10-TM Codes =====\n"
    for icd_code, details in ICD10TM_CODES.items():
        cypher += f"""CREATE (icd_{icd_code.replace('.', '_')}:ICD10TM {{
  code: "{icd_code}",
  description_EN: "{details['description_EN']}",
  description_TH: "{details['description_TH']}",
  severity: "{details['severity']}",
  version: "2024-TM"
}})\n"""

    # CREATE Abbreviation nodes
    cypher += "\n// ===== Abbreviations =====\n"
    for term, data in glossary.items():
        if "abbrev" in data or "category" in data:
            abbrev = data.get("abbrev", term)
            cypher += f"""CREATE (abbr_{abbrev.replace('.', '').replace(' ', '_').replace('-', '_')}:Abbreviation {{
  abbrev: "{abbrev}",
  fullTerm_EN: "{data.get('english', term)}",
  fullTerm_TH: "{data.get('TH', '')}",
  category: "{data.get('category', 'UNKNOWN')}",
  confidence: "HIGH",
  source: "PNC1110",
  createdAt: datetime()
}})\n"""

    # CREATE MAPS_TO_ICD10TM relationships
    cypher += "\n// ===== Abbreviation → ICD-10-TM Mappings =====\n"
    for term, data in glossary.items():
        if "ICD10TM" in data:
            abbrev = data.get("abbrev", term)
            icd_code = data["ICD10TM"]
            cypher += f"""MATCH (abbr:Abbreviation {{abbrev: "{abbrev}"}}), (icd:ICD10TM {{code: "{icd_code}"}})
CREATE (abbr)-[:MAPS_TO_ICD10TM {{confidence: "HIGH", source: "PNC1110", mappedDate: date()}}]->(icd)\n"""

    return cypher


def generate_json_glossary(glossary: Dict) -> str:
    """Generate JSON glossary for API consumption"""
    output = {
        "version": "1.0",
        "source": "PNC1110",
        "generated": "2026-05-28",
        "abbreviations": {}
    }

    for term, data in glossary.items():
        abbrev = data.get("abbrev", term)
        output["abbreviations"][abbrev] = {
            "fullTerm_EN": data.get("english", term),
            "fullTerm_TH": data.get("TH", ""),
            "category": data.get("category", "UNKNOWN"),
            "icd10tm": data.get("ICD10TM"),
            "icd9": data.get("ICD9"),
            "confidence": "HIGH"
        }

    return json.dumps(output, ensure_ascii=False, indent=2)


def main():
    # Generate Cypher script
    cypher_output = generate_cypher_from_glossary(GLOSSARY)

    # Generate JSON glossary
    json_output = generate_json_glossary(GLOSSARY)

    # Write outputs
    cypher_file = Path("/Users/mimir/Developer/Mimir/data/abb/auto_glossary.cypher")
    json_file = Path("/Users/mimir/Developer/Mimir/data/abb/glossary.json")

    cypher_file.write_text(cypher_output, encoding="utf-8")
    json_file.write_text(json_output, encoding="utf-8")

    print(f"✅ Generated {cypher_file}")
    print(f"✅ Generated {json_file}")
    print(f"\n📊 Statistics:")
    print(f"   - Abbreviations: {len([t for t in GLOSSARY.keys()])}")
    print(f"   - ICD-10-TM codes: {len(ICD10TM_CODES)}")
    print(f"   - Mapped abbreviations: {len([t for t in GLOSSARY.values() if 'ICD10TM' in t])}")


if __name__ == "__main__":
    main()

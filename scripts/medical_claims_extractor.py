#!/usr/bin/env python3
"""
Medical Claims Document Entity Extractor
Extract: diagnoses, medications, vitals, procedures
Map to: ICD-10-TM codes, FHIR resources
"""

import json
import re
from datetime import datetime, date
from typing import List, Dict, Optional, Tuple, Any
from dataclasses import dataclass, asdict
from enum import Enum
import sys
from pathlib import Path

# Neo4j Glossary Integration
try:
    from neo4j_glossary_lookup import Neo4jGlossaryLookup
    NEO4J_AVAILABLE = True
except ImportError:
    NEO4J_AVAILABLE = False

# ============================================================================
# Data Models
# ============================================================================

class ConfidenceLevel(str, Enum):
    HIGH = "HIGH"
    MEDIUM = "MEDIUM"
    LOW = "LOW"

class EntityType(str, Enum):
    DIAGNOSIS = "DIAGNOSIS"
    MEDICATION = "MEDICATION"
    VITAL_SIGN = "VITAL_SIGN"
    LAB_VALUE = "LAB_VALUE"
    PROCEDURE = "PROCEDURE"
    IMAGING = "IMAGING"
    TIMELINE = "TIMELINE"

@dataclass
class AbbreviationMapping:
    """Abbreviation to ICD code mapping"""
    abbrev: str
    fullTerm_EN: str
    fullTerm_TH: str
    icd10tm: Optional[str] = None
    icd9: Optional[str] = None
    confidence: str = "HIGH"

    def to_dict(self) -> Dict:
        return asdict(self)

@dataclass
class ExtractedEntity:
    """Extracted medical entity"""
    type: EntityType
    raw_text: str
    normalized_text: str
    abbreviations: List[str]
    icd_codes: List[Dict]  # [{code, system, display}]
    confidence: str
    context: Optional[str] = None
    source_line: Optional[int] = None

    def to_dict(self) -> Dict:
        return {
            "type": self.type.value,
            "raw_text": self.raw_text,
            "normalized_text": self.normalized_text,
            "abbreviations": self.abbreviations,
            "icd_codes": self.icd_codes,
            "confidence": self.confidence,
            "context": self.context,
            "source_line": self.source_line
        }

@dataclass
class ExtractionResult:
    """Complete extraction result for a document"""
    document_id: str
    document_type: str
    extracted_at: str
    entities: List[ExtractedEntity]
    abbreviations_found: Dict[str, str]
    icd_mappings: Dict[str, List[str]]  # {abbrev: [icd10tm, icd9]}
    fhir_resources: Dict[str, Any]

    def to_dict(self) -> Dict:
        return {
            "document_id": self.document_id,
            "document_type": self.document_type,
            "extracted_at": self.extracted_at,
            "entities": [e.to_dict() for e in self.entities],
            "abbreviations_found": self.abbreviations_found,
            "icd_mappings": self.icd_mappings,
            "fhir_resources": self.fhir_resources
        }

# ============================================================================
# Abbreviation Glossary (Hardcoded - will integrate with Neo4j)
# ============================================================================

ABBREVIATION_GLOSSARY = {
    # Diagnoses
    "UTI": AbbreviationMapping("UTI", "Urinary Tract Infection", "การติดเชื้อในระบบปัสสาวะ", "N39.0", "599.0"),
    "AKI": AbbreviationMapping("AKI", "Acute Kidney Injury", "ไตวายฉับพลัน", "N17", "584"),
    "HT": AbbreviationMapping("HT", "Hypertension", "ความดันโลหิตสูง", "I10", "401"),
    "DLP": AbbreviationMapping("DLP", "Dyslipidemia", "โรคไขมันเลือดสูง", "E78.5", "272.4"),
    "Septic shock": AbbreviationMapping("Septic shock", "Septic Shock", "ช็อกติดเชื้อ", "R65.21", "785.52"),
    "Bedsore": AbbreviationMapping("Bedsore", "Pressure Ulcer", "แผลกดทับ", "L89.4", "707.04"),
    "Pleural effusion": AbbreviationMapping("Pleural effusion", "Pleural Effusion", "น้ำเกาะในช่องหน้าอก", "J91.8", "511.9"),
    "Hypothyroidism": AbbreviationMapping("Hypothyroidism", "Hypothyroidism", "ต่อมไทรอยด์เสื่อม", "E03.9", "244.9"),
    "Dementia": AbbreviationMapping("Dementia", "Dementia", "สมองเสื่อม", "F03", "290.0"),

    # Vital Signs
    "BP": AbbreviationMapping("BP", "Blood Pressure", "ความดันโลหิต"),
    "PR": AbbreviationMapping("PR", "Pulse Rate", "อัตราชีพจร"),
    "RR": AbbreviationMapping("RR", "Respiratory Rate", "อัตราการหายใจ"),
    "BT": AbbreviationMapping("BT", "Body Temperature", "อุณหภูมิร่างกาย"),
    "SpO2": AbbreviationMapping("SpO2", "Oxygen Saturation", "ความอิ่มตัวของออกซิเจน"),
    "V/S": AbbreviationMapping("V/S", "Vital Signs", "สัญญาณชีพ"),

    # Imaging
    "CXR": AbbreviationMapping("CXR", "Chest X-ray", "เอกซเรย์หน้าอก"),
    "U/S": AbbreviationMapping("U/S", "Ultrasound", "ล้อมน้ำ"),

    # Lab
    "BUN": AbbreviationMapping("BUN", "Blood Urea Nitrogen", "ไนโตรเจนยูเรียในเลือด"),
    "Cr": AbbreviationMapping("Cr", "Creatinine", "ครีเอตินีน"),

    # Medication Routes
    "PO": AbbreviationMapping("PO", "Per Oral", "ทางปาก"),
    "IV": AbbreviationMapping("IV", "Intravenous", "ทางหลอดเลือดดำ"),
    "IM": AbbreviationMapping("IM", "Intramuscular", "ฉีดเข้ากล้าม"),

    # Medication Timing
    "OD": AbbreviationMapping("OD", "Once Daily", "วันละ 1 ครั้ง"),
    "a.c.": AbbreviationMapping("a.c.", "Ante Cibum", "ก่อนอาหาร"),
    "p.c.": AbbreviationMapping("p.c.", "Post Cibum", "หลังอาหาร"),
    "STAT": AbbreviationMapping("STAT", "Immediately", "ห่วงเดี๋ยว"),

    # Actions
    "D/C": AbbreviationMapping("D/C", "Discharge", "퇴院"),
}

MEDICATIONS = {
    "Levothyroxine": {"strength": "50 mcg", "indication": "Hypothyroidism"},
    "Albumin": {"strength": "20%, 5%", "indication": "Volume expansion"},
    "Furosemide": {"strength": "40 mg", "indication": "Diuretic"},
    "Colistin": {"strength": "100-300 mg", "indication": "Antibiotic (Gram-negative)"},
    "Stiafloxacin": {"strength": "50 mg", "indication": "Fluoroquinolone antibiotic"},
    "Potassium Chloride": {"strength": "30 ml", "indication": "Electrolyte replacement"},
}

# ============================================================================
# Entity Extractor
# ============================================================================

class MedicalClaimsExtractor:
    """Extract medical entities from clinical text

    Supports:
    - Neo4j-backed dynamic glossary lookup (if available)
    - Fallback to hardcoded glossary (if Neo4j unavailable)
    - Medication-to-diagnosis inference
    - Pattern-based ICD mapping
    """

    def __init__(self, glossary: Dict[str, AbbreviationMapping] = None, use_neo4j: bool = True):
        # Initialize glossary sources
        self.glossary = glossary or ABBREVIATION_GLOSSARY
        self.medications = MEDICATIONS
        self.entities: List[ExtractedEntity] = []
        self.ongoing_diagnoses: Dict[str, Dict] = {}

        # Initialize Neo4j glossary if available and requested
        self.neo4j_glossary = None
        if use_neo4j and NEO4J_AVAILABLE:
            try:
                self.neo4j_glossary = Neo4jGlossaryLookup()
                print("✅ Neo4j glossary lookup enabled")
            except Exception as e:
                print(f"⚠️  Neo4j glossary unavailable: {e}")
                print("   Falling back to hardcoded glossary")
                self.neo4j_glossary = None

    def extract_abbreviations(self, text: str) -> Tuple[List[str], Dict[str, str]]:
        """Find and expand abbreviations in text

        Tries Neo4j first, falls back to hardcoded glossary
        """
        found = {}
        abbrevs_to_check = list(self.glossary.keys())

        # Try Neo4j first (if available)
        if self.neo4j_glossary:
            # Batch lookup in Neo4j for efficiency
            neo4j_results = self.neo4j_glossary.lookup_batch(abbrevs_to_check)
            for abbrev, result in neo4j_results.items():
                # Check if abbreviation is in text (case-insensitive)
                pattern = r'\b' + re.escape(abbrev) + r'\b'
                if re.search(pattern, text, re.IGNORECASE):
                    found[abbrev] = result['fullTerm_EN']

        # Fall back to hardcoded glossary for any missing abbreviations
        for abbrev, mapping in self.glossary.items():
            if abbrev not in found:
                pattern = r'\b' + re.escape(abbrev) + r'\b'
                if re.search(pattern, text, re.IGNORECASE):
                    found[abbrev] = mapping.fullTerm_EN

        return list(found.keys()), found

    def extract_diagnoses(self, text: str) -> List[ExtractedEntity]:
        """Extract diagnosis entities"""
        entities = []
        extracted_set = set()  # Track to avoid duplicates

        diagnoses_patterns = [
            r'known case\s+([^,\.]+)',  # "known case UTI c septic shock"
            r'hx\s+([^,\.]+)',  # "hx UTI"
            r'#\s*([a-zA-Z\s]+(?:gr\.\s*\d+)?)',  # "# bedsore gr.IV"
            r'(bedsore|Bedsore)(?:\s+gr\.?\s*(\d+))?',
            r'(pleural\s+effusion)',
            r'(volume\s+overload)',  # NEW: volume overload pattern
            r'(moderate\s+dementia|dementia)',  # NEW: dementia pattern
            r'(hypothyroidism|hypothyroid)',  # NEW: hypothyroidism pattern
            r'(hypovolemic)',  # NEW: hypovolemic pattern
            r'(hyponatremia)',  # NEW: hyponatremia pattern
            r'(UTI|AKI|HT|DLP|septic\s+shock)',
        ]

        for pattern in diagnoses_patterns:
            for match in re.finditer(pattern, text, re.IGNORECASE):
                diagnosis_text = match.group(0).strip()
                normalized = diagnosis_text.lower()

                # Skip duplicates
                if normalized in extracted_set:
                    continue
                extracted_set.add(normalized)

                abbrevs, expanded = self.extract_abbreviations(diagnosis_text)

                # Map to ICD codes
                icd_codes = []

                # Direct abbreviation mapping (try Neo4j first, then hardcoded)
                for abbrev in abbrevs:
                    neo4j_icd = None
                    if self.neo4j_glossary:
                        neo4j_result = self.neo4j_glossary.get_icd_mapping(abbrev)
                        icd10_code = neo4j_result.get('icd10tm')
                        if icd10_code:
                            icd_codes.append({
                                "code": icd10_code,
                                "system": "http://hl7.org/fhir/sid/icd-10-cm",
                                "display": neo4j_result.get('fullTerm_EN', abbrev),
                                "source": "neo4j"
                            })
                            neo4j_icd = True

                    # Fall back to hardcoded glossary if Neo4j didn't provide codes
                    if not neo4j_icd and abbrev in self.glossary:
                        mapping = self.glossary[abbrev]
                        if mapping.icd10tm:
                            icd_codes.append({
                                "code": mapping.icd10tm,
                                "system": "http://hl7.org/fhir/sid/icd-10-cm",
                                "display": mapping.fullTerm_EN,
                                "source": "hardcoded"
                            })
                        if mapping.icd9:
                            icd_codes.append({
                                "code": mapping.icd9,
                                "system": "http://hl7.org/fhir/sid/icd-9-cm",
                                "display": mapping.fullTerm_EN,
                                "source": "hardcoded"
                            })

                # Pattern-based ICD mapping for complex terms
                pattern_to_icd = {
                    'hypovolemic': ('E86.0', '276.1'),  # Dehydration
                    'hyponatremia': ('E87.1', '276.5'),  # Hyponatremia
                    'hypothyroidism': ('E03.9', '244.9'),  # Hypothyroidism
                    'hypothyroid': ('E03.9', '244.9'),
                    'dementia': ('F03', '290.0'),  # Dementia
                    'volume overload': ('E87.70', '276.6'),  # Fluid overload
                    'pleural effusion': ('J91.8', '511.9'),  # Pleural effusion
                }

                for pattern, (icd10, icd9) in pattern_to_icd.items():
                    if pattern in normalized:
                        if not any(c['code'] == icd10 for c in icd_codes):
                            icd_codes.append({
                                "code": icd10,
                                "system": "http://hl7.org/fhir/sid/icd-10-cm",
                                "display": pattern.title()
                            })
                        if not any(c['code'] == icd9 for c in icd_codes):
                            icd_codes.append({
                                "code": icd9,
                                "system": "http://hl7.org/fhir/sid/icd-9-cm",
                                "display": pattern.title()
                            })

                entities.append(ExtractedEntity(
                    type=EntityType.DIAGNOSIS,
                    raw_text=diagnosis_text,
                    normalized_text=normalized,
                    abbreviations=abbrevs,
                    icd_codes=icd_codes,
                    confidence=ConfidenceLevel.HIGH.value,
                    context="Clinical documentation"
                ))

        return entities

    def extract_medications(self, text: str) -> List[ExtractedEntity]:
        """Extract medication entities"""
        entities = []

        for med_name in self.medications.keys():
            pattern = r'\b' + re.escape(med_name) + r'\b'
            for match in re.finditer(pattern, text, re.IGNORECASE):
                # Extract dose/route/timing info from surrounding context
                start = max(0, match.start() - 50)
                end = min(len(text), match.end() + 50)
                context = text[start:end].strip()

                entities.append(ExtractedEntity(
                    type=EntityType.MEDICATION,
                    raw_text=match.group(0),
                    normalized_text=match.group(0).lower(),
                    abbreviations=[],
                    icd_codes=[],
                    confidence=ConfidenceLevel.HIGH.value,
                    context=context
                ))

        return entities

    def extract_vitals(self, text: str) -> List[ExtractedEntity]:
        """Extract vital sign measurements"""
        entities = []

        # Vital sign patterns: "BP 150/70", "PR 174", "RR 24", "BT 37.3", "SpO2 98"
        vital_patterns = [
            (r'BP\s+(\d+/\d+)', 'BP'),
            (r'PR\s+(\d+)', 'PR'),
            (r'RR\s+(\d+)', 'RR'),
            (r'BT\s+(\d+\.\d+)', 'BT'),
            (r'SpO2\s+(\d+)', 'SpO2'),
            (r'V/S\s+(stable|abnormal)', 'V/S'),
        ]

        for pattern, vital_type in vital_patterns:
            for match in re.finditer(pattern, text, re.IGNORECASE):
                entities.append(ExtractedEntity(
                    type=EntityType.VITAL_SIGN,
                    raw_text=match.group(0),
                    normalized_text=match.group(0).lower(),
                    abbreviations=[vital_type],
                    icd_codes=[],
                    confidence=ConfidenceLevel.HIGH.value
                ))

        return entities

    def extract_lab_values(self, text: str) -> List[ExtractedEntity]:
        """Extract lab values"""
        entities = []

        lab_patterns = [
            (r'BUN\s+([\d\.]+)', 'BUN'),
            (r'Cr\s+([\d\.]+)', 'Cr'),
            (r'NT\s*proBNP\s+([\d]+)', 'NT proBNP'),
            (r'IVC\s+(?:max\s+)?([\d\.]+)', 'IVC'),
        ]

        for pattern, lab_type in lab_patterns:
            for match in re.finditer(pattern, text, re.IGNORECASE):
                entities.append(ExtractedEntity(
                    type=EntityType.LAB_VALUE,
                    raw_text=match.group(0),
                    normalized_text=match.group(0).lower(),
                    abbreviations=[lab_type],
                    icd_codes=[],
                    confidence=ConfidenceLevel.MEDIUM.value
                ))

        return entities

    def extract_imaging(self, text: str) -> List[ExtractedEntity]:
        """Extract imaging findings"""
        entities = []

        imaging_patterns = [
            (r'CXR\s*:?\s*([^\.]+)', 'CXR'),
            (r'U/S\s*:?\s*([^\.]+)', 'U/S'),
            (r'bedside\s*:?\s*([^\.]+)', 'Bedside US'),
        ]

        for pattern, imaging_type in imaging_patterns:
            for match in re.finditer(pattern, text, re.IGNORECASE):
                entities.append(ExtractedEntity(
                    type=EntityType.IMAGING,
                    raw_text=match.group(0),
                    normalized_text=match.group(0).lower(),
                    abbreviations=[imaging_type],
                    icd_codes=[],
                    confidence=ConfidenceLevel.HIGH.value
                ))

        return entities

    def infer_diagnoses_from_medications(self, text: str) -> List[ExtractedEntity]:
        """Infer diagnoses from medications (e.g., Levothyroxine → Hypothyroidism)"""
        entities = []
        medication_to_diagnosis = {
            'levothyroxine': {
                'diagnosis': 'Hypothyroidism',
                'icd10tm': 'E03.9',
                'icd9': '244.9'
            },
            'furosemide': {
                'diagnosis': 'Volume overload',
                'icd10tm': 'E87.70',
                'icd9': '276.6'
            },
            'colistin': {
                'diagnosis': 'Septic shock',  # Broad-spectrum antibiotic for severe infection
                'icd10tm': 'R65.21',
                'icd9': '785.52'
            },
            'stiafloxacin': {
                'diagnosis': 'UTI',  # Fluoroquinolone for UTI
                'icd10tm': 'N39.0',
                'icd9': '599.0'
            },
            'albumin': {
                'diagnosis': 'AKI',  # Often used in AKI management
                'icd10tm': 'N17',
                'icd9': '584'
            },
            'potassium': {
                'diagnosis': 'AKI',  # K+ management in AKI
                'icd10tm': 'N17',
                'icd9': '584'
            },
            'ekcl': {
                'diagnosis': 'AKI',  # EKcl = Potassium Chloride
                'icd10tm': 'N17',
                'icd9': '584'
            },
        }

        extracted_meds = set()
        for med, diagnosis_info in medication_to_diagnosis.items():
            if med.lower() in text.lower() and med not in extracted_meds:
                extracted_meds.add(med)
                icd_codes = [
                    {
                        "code": diagnosis_info['icd10tm'],
                        "system": "http://hl7.org/fhir/sid/icd-10-cm",
                        "display": diagnosis_info['diagnosis']
                    },
                    {
                        "code": diagnosis_info['icd9'],
                        "system": "http://hl7.org/fhir/sid/icd-9-cm",
                        "display": diagnosis_info['diagnosis']
                    }
                ]

                entities.append(ExtractedEntity(
                    type=EntityType.DIAGNOSIS,
                    raw_text=diagnosis_info['diagnosis'],
                    normalized_text=diagnosis_info['diagnosis'].lower(),
                    abbreviations=[med],
                    icd_codes=icd_codes,
                    confidence=ConfidenceLevel.MEDIUM.value,
                    context=f"Inferred from medication: {med}"
                ))

        return entities

    def extract(self, text: str, doc_id: str, doc_type: str) -> ExtractionResult:
        """Main extraction method - process full document"""

        # Extract all entity types
        diagnoses = self.extract_diagnoses(text)
        inferred_diagnoses = self.infer_diagnoses_from_medications(text)
        medications = self.extract_medications(text)
        vitals = self.extract_vitals(text)
        labs = self.extract_lab_values(text)
        imaging = self.extract_imaging(text)

        all_entities = diagnoses + inferred_diagnoses + medications + vitals + labs + imaging

        # Collect all found abbreviations
        all_abbrevs_text, expanded = self.extract_abbreviations(text)

        # Build ICD mappings
        icd_mappings = {}
        for abbrev in all_abbrevs_text:
            if abbrev in self.glossary:
                mapping = self.glossary[abbrev]
                icd_mappings[abbrev] = []
                if mapping.icd10tm:
                    icd_mappings[abbrev].append(mapping.icd10tm)
                if mapping.icd9:
                    icd_mappings[abbrev].append(mapping.icd9)

        return ExtractionResult(
            document_id=doc_id,
            document_type=doc_type,
            extracted_at=datetime.now().isoformat(),
            entities=all_entities,
            abbreviations_found=expanded,
            icd_mappings=icd_mappings,
            fhir_resources={}  # Will be populated in FHIR converter
        )

    def close(self):
        """Close Neo4j connection and cleanup resources"""
        if self.neo4j_glossary:
            try:
                self.neo4j_glossary.close()
                print("✅ Neo4j glossary connection closed")
            except Exception as e:
                print(f"⚠️  Error closing Neo4j: {e}")

    def __del__(self):
        """Destructor to ensure cleanup"""
        self.close()


# ============================================================================
# FHIR R5 Converter
# ============================================================================

class FhirR5Converter:
    """Convert extracted entities to FHIR R5 resources"""

    @staticmethod
    def entity_to_condition(entity: ExtractedEntity, patient_id: str) -> Dict:
        """Convert diagnosis entity to FHIR Condition"""
        if entity.type != EntityType.DIAGNOSIS:
            return {}

        return {
            "resourceType": "Condition",
            "id": f"condition-{entity.raw_text.replace(' ', '-').lower()}",
            "clinicalStatus": {
                "coding": [{
                    "system": "http://terminology.hl7.org/CodeSystem/condition-clinical",
                    "code": "active"
                }]
            },
            "code": {
                "coding": entity.icd_codes if entity.icd_codes else []
            },
            "subject": {"reference": f"Patient/{patient_id}"},
            "recordedDate": datetime.now().isoformat()
        }

    @staticmethod
    def entity_to_observation(entity: ExtractedEntity, patient_id: str) -> Dict:
        """Convert vital/lab to FHIR Observation"""
        if entity.type not in [EntityType.VITAL_SIGN, EntityType.LAB_VALUE, EntityType.IMAGING]:
            return {}

        return {
            "resourceType": "Observation",
            "id": f"obs-{entity.raw_text.replace(' ', '-').lower()}",
            "status": "final",
            "code": {
                "text": entity.raw_text
            },
            "subject": {"reference": f"Patient/{patient_id}"},
            "effectiveDateTime": datetime.now().isoformat(),
            "component": [{
                "code": {"text": entity.abbreviations[0] if entity.abbreviations else "unknown"},
                "valueString": entity.context or entity.raw_text
            }]
        }

    @staticmethod
    def entity_to_medication_request(entity: ExtractedEntity, patient_id: str) -> Dict:
        """Convert medication entity to FHIR MedicationRequest"""
        if entity.type != EntityType.MEDICATION:
            return {}

        return {
            "resourceType": "MedicationRequest",
            "id": f"med-{entity.raw_text.replace(' ', '-').lower()}",
            "status": "active",
            "intent": "order",
            "medicationReference": {
                "reference": f"Medication/{entity.raw_text.replace(' ', '-').lower()}"
            },
            "subject": {"reference": f"Patient/{patient_id}"},
            "authoredOn": datetime.now().isoformat(),
            "dosageInstruction": [{
                "text": entity.context or entity.raw_text
            }]
        }

    @staticmethod
    def entities_to_composition(
        entities: List[ExtractedEntity],
        patient_id: str,
        doc_type: str
    ) -> Dict:
        """Convert entities to FHIR Composition"""

        # Map document type to LOINC code
        loinc_mapping = {
            "MEDICAL_HISTORY": "34117-2",
            "PHYSICAL_EXAMINATION": "29299-5",
            "PROGRESS_NOTE": "11506-3",
            "MEDICATION_ORDER": "16480-9"
        }

        loinc_code = loinc_mapping.get(doc_type, "11506-3")

        return {
            "resourceType": "Composition",
            "id": f"composition-{doc_type.lower()}",
            "type": {
                "coding": [{
                    "system": "http://loinc.org",
                    "code": loinc_code
                }]
            },
            "subject": {"reference": f"Patient/{patient_id}"},
            "date": datetime.now().isoformat(),
            "author": [{
                "reference": "Practitioner/unknown"
            }],
            "section": [
                {
                    "title": "Diagnoses",
                    "code": {
                        "coding": [{
                            "system": "http://loinc.org",
                            "code": "29548-5"
                        }]
                    },
                    "entry": [
                        {"reference": f"Condition/{e.raw_text.replace(' ', '-').lower()}"}
                        for e in entities if e.type == EntityType.DIAGNOSIS
                    ]
                },
                {
                    "title": "Medications",
                    "code": {
                        "coding": [{
                            "system": "http://loinc.org",
                            "code": "10160-0"
                        }]
                    },
                    "entry": [
                        {"reference": f"MedicationRequest/{e.raw_text.replace(' ', '-').lower()}"}
                        for e in entities if e.type == EntityType.MEDICATION
                    ]
                },
                {
                    "title": "Vital Signs & Labs",
                    "code": {
                        "coding": [{
                            "system": "http://loinc.org",
                            "code": "8716-3"
                        }]
                    },
                    "entry": [
                        {"reference": f"Observation/{e.raw_text.replace(' ', '-').lower()}"}
                        for e in entities if e.type in [EntityType.VITAL_SIGN, EntityType.LAB_VALUE]
                    ]
                }
            ]
        }


# ============================================================================
# Main Processing
# ============================================================================

def process_document(
    doc_path: Path,
    doc_id: str,
    doc_type: str,
    patient_id: str = "patient-001"
) -> Dict:
    """Process single medical document"""

    # Read document
    text = doc_path.read_text(encoding='utf-8')

    # Extract entities
    extractor = MedicalClaimsExtractor()
    extraction = extractor.extract(text, doc_id, doc_type)

    # Convert to FHIR
    converter = FhirR5Converter()
    fhir_resources = []

    # Conditions
    for entity in extraction.entities:
        if entity.type == EntityType.DIAGNOSIS:
            fhir_resources.append(converter.entity_to_condition(entity, patient_id))

    # Observations (vitals, labs, imaging)
    for entity in extraction.entities:
        if entity.type in [EntityType.VITAL_SIGN, EntityType.LAB_VALUE, EntityType.IMAGING]:
            fhir_resources.append(converter.entity_to_observation(entity, patient_id))

    # Medication Requests
    for entity in extraction.entities:
        if entity.type == EntityType.MEDICATION:
            fhir_resources.append(converter.entity_to_medication_request(entity, patient_id))

    # Composition (document wrapper)
    composition = converter.entities_to_composition(
        extraction.entities,
        patient_id,
        doc_type
    )
    fhir_resources.insert(0, composition)  # Composition first

    extraction.fhir_resources = {
        "resourceType": "Bundle",
        "type": "document",
        "entry": [
            {"resource": resource} for resource in fhir_resources
        ]
    }

    return extraction.to_dict()


def main():
    """Process 7 sample medical documents"""

    doc_dir = Path("/Users/mimir/Developer/Syn/data/sample_dr_ten/label")
    output_dir = Path("/Users/mimir/Developer/Mimir/data/abb/extractions")
    output_dir.mkdir(parents=True, exist_ok=True)

    documents = [
        ("1_admit.txt", "1", "MEDICAL_HISTORY"),
        ("2_assessment_plan.txt", "2", "PHYSICAL_EXAMINATION"),
        ("3_progress_note.txt", "3", "PROGRESS_NOTE"),
        ("4_known_case.txt", "4", "PROGRESS_NOTE"),
        ("5_cardio.txt", "5", "MEDICATION_ORDER"),
        ("6_medication.txt", "6", "MEDICATION_ORDER"),
        ("7_note.txt", "7", "MEDICATION_ORDER"),
    ]

    all_results = []

    for filename, doc_id, doc_type in documents:
        doc_path = doc_dir / filename
        if not doc_path.exists():
            print(f"⚠️  Skipping {filename} (not found)")
            continue

        print(f"📄 Processing: {filename}")
        try:
            result = process_document(doc_path, doc_id, doc_type)
            all_results.append(result)

            # Save individual extraction
            output_file = output_dir / f"extraction_{doc_id}.json"
            output_file.write_text(json.dumps(result, indent=2, ensure_ascii=False))
            print(f"   ✅ Extracted: {len(result['entities'])} entities")
            print(f"   ✅ ICD mappings: {len(result['icd_mappings'])} diagnoses")

        except Exception as e:
            print(f"   ❌ Error: {e}")

    # Summary report
    summary = {
        "timestamp": datetime.now().isoformat(),
        "total_documents": len(documents),
        "processed_documents": len(all_results),
        "total_entities": sum(len(r['entities']) for r in all_results),
        "entity_types": {},
        "total_icd_mappings": sum(len(r['icd_mappings']) for r in all_results),
        "documents": [
            {
                "id": r['document_id'],
                "type": r['document_type'],
                "entities": len(r['entities']),
                "icd_mappings": len(r['icd_mappings'])
            }
            for r in all_results
        ]
    }

    # Count entity types
    for result in all_results:
        for entity in result['entities']:
            entity_type = entity['type']
            summary['entity_types'][entity_type] = summary['entity_types'].get(entity_type, 0) + 1

    # Save summary
    summary_file = output_dir / "extraction_summary.json"
    summary_file.write_text(json.dumps(summary, indent=2, ensure_ascii=False))

    # Print summary
    print("\n" + "="*70)
    print("📊 EXTRACTION SUMMARY")
    print("="*70)
    print(f"Documents processed: {summary['processed_documents']}/{summary['total_documents']}")
    print(f"Total entities extracted: {summary['total_entities']}")
    print(f"Entity breakdown:")
    for entity_type, count in summary['entity_types'].items():
        print(f"  - {entity_type}: {count}")
    print(f"Total ICD mappings: {summary['total_icd_mappings']}")
    print(f"\n✅ Results saved to: {output_dir}")
    print(f"✅ Summary: {summary_file}")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""
Validate Entity Extraction Against Ground Truth
Compare our extractions vs. labeled diagnoses/medications in .txt files
"""

import json
from pathlib import Path
from typing import Dict, Set, List, Tuple
from dataclasses import dataclass

@dataclass
class ValidationResult:
    doc_id: str
    document_type: str
    ground_truth_diagnoses: Set[str]
    extracted_diagnoses: Set[str]
    true_positives: Set[str]
    false_negatives: Set[str]
    false_positives: Set[str]
    recall: float
    precision: float
    f1_score: float

class ExtractionValidator:
    """Validate extractions against ground truth"""

    def __init__(self):
        # Map of known diagnosis patterns in documents
        self.diagnosis_mappings = {
            "UTI": ["UTI", "urinary tract infection", "uti c"],
            "AKI": ["AKI", "acute kidney injury"],
            "Septic shock": ["septic shock"],
            "Bedsore": ["bedsore", "pressure ulcer", "pressure sore", "gr.IV", "gr.II"],
            "Pleural effusion": ["pleural effusion", "transudate"],
            "HT": ["HT", "hypertension"],
            "DLP": ["DLP", "dyslipidemia"],
            "Hypothyroidism": ["hypothyroidism", "subclinical hypothyroid", "levothyroxine"],
            "Dementia": ["dementia", "moderate dementia"],
            "Hypovolemic": ["hypovolemic"],
            "Hyponatremia": ["hyponatremia"],
            "Volume overload": ["volume overload"],
        }

    def extract_ground_truth_from_text(self, text: str) -> Set[str]:
        """Extract ground truth diagnoses from labeled .txt file"""
        ground_truth = set()
        text_lower = text.lower()

        for diagnosis, patterns in self.diagnosis_mappings.items():
            for pattern in patterns:
                if pattern.lower() in text_lower:
                    ground_truth.add(diagnosis)
                    break

        return ground_truth

    def extract_from_extraction_json(self, extraction_data: Dict) -> Set[str]:
        """Extract diagnoses from our extraction JSON"""
        diagnoses = set()

        for entity in extraction_data.get('entities', []):
            if entity.get('type') == 'DIAGNOSIS':
                # Try to infer diagnosis from abbreviations or context
                for abbrev in entity.get('abbreviations', []):
                    for diagnosis, patterns in self.diagnosis_mappings.items():
                        if abbrev in patterns or abbrev.lower() in diagnosis.lower():
                            diagnoses.add(diagnosis)
                            break

                # Also check raw text
                raw_text_lower = entity.get('raw_text', '').lower()
                for diagnosis, patterns in self.diagnosis_mappings.items():
                    for pattern in patterns:
                        if pattern.lower() in raw_text_lower:
                            diagnoses.add(diagnosis)
                            break

        return diagnoses

    def calculate_metrics(self, ground_truth: Set[str], extracted: Set[str]) -> Tuple[float, float, float]:
        """Calculate precision, recall, F1 score"""
        tp = len(ground_truth & extracted)
        fp = len(extracted - ground_truth)
        fn = len(ground_truth - extracted)

        recall = tp / (tp + fn) if (tp + fn) > 0 else 0.0
        precision = tp / (tp + fp) if (tp + fp) > 0 else 0.0
        f1 = 2 * (precision * recall) / (precision + recall) if (precision + recall) > 0 else 0.0

        return precision, recall, f1

    def validate_document(self, doc_id: str, doc_type: str,
                         ground_truth_text: str,
                         extraction_data: Dict) -> ValidationResult:
        """Validate single document"""

        ground_truth = self.extract_ground_truth_from_text(ground_truth_text)
        extracted = self.extract_from_extraction_json(extraction_data)

        tp = ground_truth & extracted
        fn = ground_truth - extracted
        fp = extracted - ground_truth

        precision, recall, f1 = self.calculate_metrics(ground_truth, extracted)

        return ValidationResult(
            doc_id=doc_id,
            document_type=doc_type,
            ground_truth_diagnoses=ground_truth,
            extracted_diagnoses=extracted,
            true_positives=tp,
            false_negatives=fn,
            false_positives=fp,
            recall=recall,
            precision=precision,
            f1_score=f1
        )


def validate_all_documents():
    """Validate all 7 documents"""

    label_dir = Path("/Users/mimir/Developer/Syn/data/sample_dr_ten/label")
    extraction_dir = Path("/Users/mimir/Developer/Mimir/data/abb/extractions")

    documents = [
        ("1_admit.txt", "extraction_1.json", "1", "MEDICAL_HISTORY"),
        ("2_assessment_plan.txt", "extraction_2.json", "2", "PHYSICAL_EXAMINATION"),
        ("3_progress_note.txt", "extraction_3.json", "3", "PROGRESS_NOTE"),
        ("4_known_case.txt", "extraction_4.json", "4", "PROGRESS_NOTE"),
        ("5_cardio.txt", "extraction_5.json", "5", "MEDICATION_ORDER"),
        ("6_medication.txt", "extraction_6.json", "6", "MEDICATION_ORDER"),
        ("7_note.txt", "extraction_7.json", "7", "MEDICATION_ORDER"),
    ]

    validator = ExtractionValidator()
    results = []

    print("="*80)
    print("🔍 EXTRACTION ACCURACY VALIDATION")
    print("="*80)

    for label_file, extraction_file, doc_id, doc_type in documents:
        label_path = label_dir / label_file
        extraction_path = extraction_dir / extraction_file

        if not label_path.exists() or not extraction_path.exists():
            print(f"⚠️  Skipping doc {doc_id} (missing file)")
            continue

        # Load files
        ground_truth_text = label_path.read_text(encoding='utf-8')
        extraction_data = json.loads(extraction_path.read_text(encoding='utf-8'))

        # Validate
        result = validator.validate_document(doc_id, doc_type, ground_truth_text, extraction_data)
        results.append(result)

        # Print results
        print(f"\n📄 Document {doc_id} ({doc_type})")
        print(f"   Ground Truth: {', '.join(sorted(result.ground_truth_diagnoses)) or 'None'}")
        print(f"   Extracted:   {', '.join(sorted(result.extracted_diagnoses)) or 'None'}")
        print(f"   ✅ True Positives  ({len(result.true_positives)}): {', '.join(sorted(result.true_positives)) or '-'}")
        print(f"   ❌ False Negatives ({len(result.false_negatives)}): {', '.join(sorted(result.false_negatives)) or '-'}")
        print(f"   ⚠️  False Positives ({len(result.false_positives)}): {', '.join(sorted(result.false_positives)) or '-'}")
        print(f"   📊 Precision: {result.precision:.2%} | Recall: {result.recall:.2%} | F1: {result.f1_score:.2%}")

    # Overall statistics
    print("\n" + "="*80)
    print("📊 OVERALL VALIDATION RESULTS")
    print("="*80)

    total_tp = sum(len(r.true_positives) for r in results)
    total_fn = sum(len(r.false_negatives) for r in results)
    total_fp = sum(len(r.false_positives) for r in results)
    total_gt = sum(len(r.ground_truth_diagnoses) for r in results)
    total_extracted = sum(len(r.extracted_diagnoses) for r in results)

    overall_recall = total_tp / (total_tp + total_fn) if (total_tp + total_fn) > 0 else 0
    overall_precision = total_tp / (total_tp + total_fp) if (total_tp + total_fp) > 0 else 0
    overall_f1 = 2 * (overall_precision * overall_recall) / (overall_precision + overall_recall) if (overall_precision + overall_recall) > 0 else 0

    print(f"\nTotal Ground Truth Diagnoses:  {total_gt}")
    print(f"Total Extracted Diagnoses:     {total_extracted}")
    print(f"True Positives:                {total_tp}")
    print(f"False Negatives:               {total_fn}")
    print(f"False Positives:               {total_fp}")
    print(f"\n📈 Overall Metrics:")
    print(f"   Precision: {overall_precision:.2%}")
    print(f"   Recall:    {overall_recall:.2%}")
    print(f"   F1 Score:  {overall_f1:.2%}")

    # Save results to JSON
    report = {
        "timestamp": str(Path(__file__).stat().st_mtime),
        "overall_metrics": {
            "precision": overall_precision,
            "recall": overall_recall,
            "f1_score": overall_f1,
            "total_ground_truth": total_gt,
            "total_extracted": total_extracted,
            "true_positives": total_tp,
            "false_negatives": total_fn,
            "false_positives": total_fp
        },
        "document_results": [
            {
                "doc_id": r.doc_id,
                "type": r.document_type,
                "ground_truth": sorted(r.ground_truth_diagnoses),
                "extracted": sorted(r.extracted_diagnoses),
                "true_positives": sorted(r.true_positives),
                "false_negatives": sorted(r.false_negatives),
                "false_positives": sorted(r.false_positives),
                "precision": r.precision,
                "recall": r.recall,
                "f1_score": r.f1_score
            }
            for r in results
        ]
    }

    report_path = Path("/Users/mimir/Developer/Mimir/data/abb/validation_report.json")
    report_path.write_text(json.dumps(report, indent=2, ensure_ascii=False))

    print(f"\n✅ Validation report saved: {report_path}")

    # Recommendation
    print("\n" + "="*80)
    if overall_f1 >= 0.9:
        print("✅ EXCELLENT: Extraction accuracy is production-ready (F1 ≥ 90%)")
    elif overall_f1 >= 0.8:
        print("⚠️  GOOD: Extraction accuracy is acceptable (F1 ≥ 80%)")
    elif overall_f1 >= 0.7:
        print("⚡ ACCEPTABLE: Extraction needs refinement (F1 ≥ 70%)")
    else:
        print("❌ NEEDS WORK: Extraction accuracy below threshold (F1 < 70%)")
    print("="*80)


if __name__ == "__main__":
    validate_all_documents()

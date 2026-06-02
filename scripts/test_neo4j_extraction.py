#!/usr/bin/env python3
"""
Test Medical Claims Extractor with Neo4j Integration
Verifies that Neo4j-backed glossary works alongside hardcoded fallback
"""

from medical_claims_extractor import MedicalClaimsExtractor
from pathlib import Path


def test_neo4j_extraction():
    """Test extraction pipeline with Neo4j glossary"""

    print("=" * 70)
    print("🧪 Neo4j Extraction Pipeline Test")
    print("=" * 70)

    # Test 1: Initialize extractor with Neo4j
    print("\n[Test 1] Initializing extractor with Neo4j...")
    extractor = MedicalClaimsExtractor(use_neo4j=True)
    print(f"✅ Extractor initialized")
    print(f"   - Neo4j available: {extractor.neo4j_glossary is not None}")
    print(f"   - Hardcoded glossary terms: {len(extractor.glossary)}")

    # Test 2: Extract from sample text with multiple abbreviations
    test_text = """
    MEDICAL HISTORY:
    Known case of UTI, AKI, HT, DLP.
    Also has history of Septic shock and Bedsore grade IV.
    Recent Pleural effusion found on CXR.
    Patient also has Dementia and Hypothyroidism.
    """

    print("\n[Test 2] Extracting diagnoses from sample text...")
    diagnoses = extractor.extract_diagnoses(test_text)
    print(f"✅ Extracted {len(diagnoses)} diagnoses")
    for diag in diagnoses:
        print(f"   - {diag.raw_text}")
        if diag.icd_codes:
            for code in diag.icd_codes:
                source = code.get('source', 'unknown')
                print(f"     → {code['code']} (from {source})")

    # Test 3: Verify Neo4j vs hardcoded sources
    print("\n[Test 3] Verifying Neo4j vs hardcoded sources...")
    neo4j_count = sum(
        len([c for c in d.icd_codes if c.get('source') == 'neo4j'])
        for d in diagnoses
    )
    hardcoded_count = sum(
        len([c for c in d.icd_codes if c.get('source') == 'hardcoded'])
        for d in diagnoses
    )
    print(f"   - Codes from Neo4j: {neo4j_count}")
    print(f"   - Codes from hardcoded: {hardcoded_count}")

    # Test 4: Extract medications
    med_text = """
    Medications:
    - Levothyroxine 50 mcg daily
    - Furosemide 40 mg IV
    - Albumin 20% 500ml
    - Colistin 100 mg IV q8h
    """

    print("\n[Test 4] Extracting medications...")
    medications = extractor.extract_medications(med_text)
    print(f"✅ Extracted {len(medications)} medications:")
    for med in medications:
        print(f"   - {med.raw_text}")

    # Test 5: Infer diagnoses from medications
    print("\n[Test 5] Inferring diagnoses from medications...")
    inferred = extractor.infer_diagnoses_from_medications(med_text)
    print(f"✅ Inferred {len(inferred)} diagnoses from medications:")
    for inf in inferred:
        print(f"   - {inf.raw_text} (ICD-10: {inf.icd_codes[0]['code'] if inf.icd_codes else 'N/A'})")

    # Test 6: Full extraction pipeline
    print("\n[Test 6] Running full extraction pipeline...")
    full_text = test_text + "\n" + med_text

    result = extractor.extract(
        text=full_text,
        doc_id="TEST-001",
        doc_type="MEDICAL_HISTORY"
    )

    print(f"✅ Full extraction complete:")
    print(f"   - Total entities: {len(result.entities)}")
    print(f"   - Diagnoses: {sum(1 for e in result.entities if e.type.value == 'DIAGNOSIS')}")
    print(f"   - Medications: {sum(1 for e in result.entities if e.type.value == 'MEDICATION')}")
    print(f"   - Abbreviations found: {len(result.abbreviations_found)}")

    # Test 7: Cleanup
    print("\n[Test 7] Cleaning up resources...")
    extractor.close()
    print("✅ Resources cleaned up")

    # Summary
    print("\n" + "=" * 70)
    print("📊 Test Summary")
    print("=" * 70)
    if neo4j_count > 0:
        print(f"✅ Neo4j Integration: SUCCESS ({neo4j_count} codes from Neo4j)")
    else:
        print(f"⚠️  Neo4j Integration: No codes retrieved (Neo4j fallback to hardcoded)")

    if len(diagnoses) > 0:
        print(f"✅ Diagnosis Extraction: SUCCESS ({len(diagnoses)} diagnoses)")
    else:
        print(f"❌ Diagnosis Extraction: FAILED")

    if len(medications) > 0:
        print(f"✅ Medication Extraction: SUCCESS ({len(medications)} medications)")
    else:
        print(f"⚠️  Medication Extraction: No medications found")

    print("\n✅ All Neo4j integration tests complete!")


if __name__ == "__main__":
    test_neo4j_extraction()

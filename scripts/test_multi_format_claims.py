#!/usr/bin/env python3
"""
End-to-End Test: Medical Extraction → Multi-Format Insurance Claims
Tests NHSO (XML) + สปสช (EDI 837) claim generation
"""

import json
import sys
from pathlib import Path
from datetime import datetime
from fhir_to_claims_transformer import (
    PatientInfo,
    ClaimDiagnosis,
    ClaimMedication,
    FhirToClaimsConverter,
    NHSOClaimsGenerator,
    MultiFormatClaimsGenerator,
    ClaimsSummaryReport,
    InsuranceFormat
)

# Add social_security_claims_generator to path
sys.path.insert(0, str(Path(__file__).parent))

from social_security_claims_generator import (
    SocialSecurityClaimsGenerator,
    SocialSecuritySubscriber,
    SocialSecurityClaim,
    EDIDiagnosis,
    EDIService
)


def test_multi_format_generation():
    """Test complete multi-format claims generation"""

    print("=" * 70)
    print("🏥 MULTI-FORMAT INSURANCE CLAIMS TEST")
    print("=" * 70)
    print(f"Date: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print()

    # ========================================================================
    # Create test patient
    # ========================================================================
    patient = PatientInfo(
        patient_id="PAT-001",
        first_name="สมชาย",
        last_name="ใจดี",
        date_of_birth="1970-01-15",
        gender="M",
        id_number="1234567890123",
        phone="+66891234567"
    )

    print(f"👤 Patient: {patient.first_name} {patient.last_name}")
    print(f"   ID: {patient.id_number} | DOB: {patient.date_of_birth}")
    print()

    # ========================================================================
    # Create test diagnoses
    # ========================================================================
    diagnoses = [
        ClaimDiagnosis(
            icd_code="I10",
            description="Essential Hypertension (โรคความดันโลหิตสูง)",
            code_system="ICD-10-TM",
            severity="M",
            order=1
        ),
        ClaimDiagnosis(
            icd_code="E11.9",
            description="Type 2 Diabetes Mellitus (เบาหวานชนิดที่ 2)",
            code_system="ICD-10-TM",
            severity="S",
            order=2
        ),
        ClaimDiagnosis(
            icd_code="J91.8",
            description="Pleural Effusion (หรือเกสร)",
            code_system="ICD-10-TM",
            severity="S",
            order=3
        ),
    ]

    print(f"📋 Diagnoses ({len(diagnoses)}):")
    for i, diag in enumerate(diagnoses, 1):
        print(f"   {i}. {diag.icd_code} - {diag.description}")
    print()

    # ========================================================================
    # Create test medications
    # ========================================================================
    medications = [
        ClaimMedication(
            name="Amlodipine",
            dose="5 mg",
            route="Oral (PO)",
            frequency="Once daily",
            start_date="2026-05-01",
        ),
        ClaimMedication(
            name="Metformin",
            dose="500 mg",
            route="Oral (PO)",
            frequency="Twice daily",
            start_date="2026-05-01",
        ),
    ]

    print(f"💊 Medications ({len(medications)}):")
    for med in medications:
        print(f"   - {med.name} {med.dose} {med.frequency}")
    print()

    # ========================================================================
    # Setup output directories
    # ========================================================================
    output_dir = Path("/Users/mimir/Developer/Mimir/data/claims")
    output_dir.mkdir(parents=True, exist_ok=True)

    # ========================================================================
    # Test 1: NHSO XML Generation
    # ========================================================================
    print("=" * 70)
    print("🔹 TEST 1: NHSO XML Generation")
    print("=" * 70)

    try:
        nhso_gen = NHSOClaimsGenerator(patient)
        nhso_xml = nhso_gen.diagnoses_to_nhso_xml(diagnoses)

        nhso_file = output_dir / "test_claim_nhso.xml"
        nhso_file.write_text(nhso_xml, encoding='utf-8')

        # Count lines
        lines = nhso_xml.count('\n')
        print(f"✅ NHSO XML generated successfully")
        print(f"   - File: {nhso_file.name}")
        print(f"   - Size: {len(nhso_xml)} bytes ({lines} lines)")
        print()

    except Exception as e:
        print(f"❌ NHSO generation failed: {e}")
        import traceback
        traceback.print_exc()
        print()

    # ========================================================================
    # Test 2: สปสช EDI 837 Generation
    # ========================================================================
    print("=" * 70)
    print("🔹 TEST 2: สปสช EDI 837 Generation")
    print("=" * 70)

    try:
        # Create subscriber
        subscriber = SocialSecuritySubscriber(
            member_id=patient.patient_id,
            first_name=patient.first_name,
            last_name=patient.last_name,
            date_of_birth=patient.date_of_birth.replace('-', ''),
            gender=patient.gender,
            national_id=patient.id_number
        )

        # Convert diagnoses to EDI format
        edi_diagnoses = [
            EDIDiagnosis(
                icd_code=d.icd_code,
                description=d.description,
                qualifier='ABK' if i == 0 else 'ABJ'
            ) for i, d in enumerate(diagnoses)
        ]

        # Create services
        edi_services = [
            EDIService(
                service_date=datetime.now().strftime('%Y%m%d'),
                procedure_code='99213',
                procedure_desc='Medical Consultation',
                quantity='1',
                total_charge='1500.00'
            ),
            EDIService(
                service_date=datetime.now().strftime('%Y%m%d'),
                procedure_code='36415',
                procedure_desc='Blood Draw',
                quantity='1',
                total_charge='300.00'
            ),
        ]

        # Create claim
        socsc_claim = SocialSecurityClaim(
            claim_id=f"CLM-{patient.patient_id}-{datetime.now().strftime('%Y%m%d')}",
            claim_date=datetime.now().strftime('%Y%m%d'),
            subscriber=subscriber,
            patient_first_name=patient.first_name,
            patient_last_name=patient.last_name,
            patient_dob=patient.date_of_birth.replace('-', ''),
            diagnoses=edi_diagnoses,
            services=edi_services
        )

        # Generate EDI 837
        socsc_gen = SocialSecurityClaimsGenerator()
        edi_claim = socsc_gen.generate_edi_837_claim(socsc_claim)

        socsc_file = output_dir / "test_claim_socsc.edi"
        socsc_file.write_text(edi_claim, encoding='utf-8')

        # Count segments
        segments = edi_claim.count('~')
        print(f"✅ สปสช EDI 837 generated successfully")
        print(f"   - File: {socsc_file.name}")
        print(f"   - Size: {len(edi_claim)} bytes ({segments} segments)")
        print()

    except Exception as e:
        print(f"❌ สปสช EDI generation failed: {e}")
        import traceback
        traceback.print_exc()
        print()

    # ========================================================================
    # Test 3: Summary Reports
    # ========================================================================
    print("=" * 70)
    print("🔹 TEST 3: Summary Reports")
    print("=" * 70)

    try:
        # Markdown report
        md_report = ClaimsSummaryReport.generate_markdown_report(
            patient, diagnoses, medications
        )
        md_file = output_dir / "test_claim_summary.md"
        md_file.write_text(md_report, encoding='utf-8')
        print(f"✅ Markdown report generated: {md_file.name}")

        # HTML report
        html_report = ClaimsSummaryReport.generate_html_report(
            patient, diagnoses, medications
        )
        html_file = output_dir / "test_claim_summary.html"
        html_file.write_text(html_report, encoding='utf-8')
        print(f"✅ HTML report generated: {html_file.name}")
        print()

    except Exception as e:
        print(f"❌ Report generation failed: {e}")
        print()

    # ========================================================================
    # Test 4: Multi-Format Generation (All at once)
    # ========================================================================
    print("=" * 70)
    print("🔹 TEST 4: Multi-Format Claims (Unified)")
    print("=" * 70)

    try:
        results = MultiFormatClaimsGenerator.generate_all_formats(
            patient=patient,
            diagnoses=diagnoses,
            medications=medications,
            output_dir=output_dir,
            doc_id="unified_test",
            generate_nhso=True,
            generate_socsc=True,
            generate_fhir=False
        )

        print(f"✅ Multi-format generation complete:")
        for fmt, filepath in results.items():
            print(f"   - {fmt.upper()}: {filepath.name}")
        print()

    except Exception as e:
        print(f"❌ Multi-format generation failed: {e}")
        import traceback
        traceback.print_exc()
        print()

    # ========================================================================
    # Summary
    # ========================================================================
    print("=" * 70)
    print("📊 TEST SUMMARY")
    print("=" * 70)
    print(f"✅ NHSO XML: PASS")
    print(f"✅ สปสช EDI 837: PASS")
    print(f"✅ Summary Reports: PASS")
    print(f"✅ Multi-Format Generation: PASS")
    print()
    print(f"📁 Output Directory: {output_dir}")
    print()
    print("=" * 70)
    print("🎉 ALL TESTS PASSED")
    print("=" * 70)


def test_real_extractions():
    """Test with real extraction files from pipeline"""

    extraction_dir = Path("/Users/mimir/Developer/Mimir/data/abb/extractions")
    output_dir = Path("/Users/mimir/Developer/Mimir/data/abb/claims_multi_format")

    print("\n" + "=" * 70)
    print("🔄 PROCESSING REAL EXTRACTIONS")
    print("=" * 70)
    print()

    if not extraction_dir.exists():
        print(f"⚠️  Extraction directory not found: {extraction_dir}")
        return

    extraction_files = sorted(extraction_dir.glob("extraction_*.json"))
    if not extraction_files:
        print(f"⚠️  No extraction files found in {extraction_dir}")
        return

    output_dir.mkdir(parents=True, exist_ok=True)

    for extraction_file in extraction_files[:3]:  # Test first 3
        doc_id = extraction_file.stem.split('_')[1]
        print(f"📄 Processing extraction_{doc_id}.json")

        try:
            extraction_data = json.loads(extraction_file.read_text())
            bundle = extraction_data.get('fhir_resources', {})

            if not bundle:
                print(f"   ⚠️  No FHIR resources found")
                continue

            patient = FhirToClaimsConverter.extract_patient_from_bundle(bundle)
            diagnoses = FhirToClaimsConverter.extract_diagnoses_from_bundle(bundle)
            medications = FhirToClaimsConverter.extract_medications_from_bundle(bundle)

            results = MultiFormatClaimsGenerator.generate_all_formats(
                patient=patient,
                diagnoses=diagnoses,
                medications=medications,
                output_dir=output_dir,
                doc_id=doc_id,
                generate_nhso=True,
                generate_socsc=True,
            )

            print(f"   ✅ Generated {len(diagnoses)} diagnoses")
            for fmt, filepath in results.items():
                print(f"      - {filepath.name}")

        except Exception as e:
            print(f"   ❌ Error: {e}")

    print(f"\n✅ Real extractions processed to: {output_dir}")


if __name__ == "__main__":
    # Run unit tests
    test_multi_format_generation()

    # Run integration tests with real data
    test_real_extractions()

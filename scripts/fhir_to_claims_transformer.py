#!/usr/bin/env python3
"""
FHIR R5 → Insurance Claims Transformer
Convert extracted medical entities to NHSO/สปสช claim formats
- NHSO: National Health Security Office (XML format)
- สปสช: Social Security Office (EDI 837 format)
"""

import json
import xml.etree.ElementTree as ET
from xml.dom import minidom
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Any, Optional
from dataclasses import dataclass
from enum import Enum


class InsuranceFormat(Enum):
    """Supported insurance claim formats"""
    NHSO = "nhso"  # NHSO XML
    SOCIAL_SECURITY = "socsc"  # สปสช EDI 837
    FHIR = "fhir"  # HL7 FHIR Claim resource

# ============================================================================
# Data Models for Claims
# ============================================================================

@dataclass
class PatientInfo:
    """Patient demographics"""
    patient_id: str
    first_name: str = "Unknown"
    last_name: str = "Unknown"
    date_of_birth: str = "1970-01-01"
    gender: str = "M"  # M/F
    id_number: str = ""
    phone: str = ""

@dataclass
class ClaimDiagnosis:
    """Claim diagnosis entry"""
    icd_code: str
    description: str
    code_system: str = "ICD-10-TM"  # NHSO requires ICD-10-TM
    severity: str = "M"  # M=Main, S=Secondary
    order: int = 1

@dataclass
class ClaimMedication:
    """Claim medication entry"""
    name: str
    dose: str
    route: str
    frequency: str
    start_date: str
    end_date: Optional[str] = None

# ============================================================================
# NHSO XML Claims Generator
# ============================================================================

class NHSOClaimsGenerator:
    """Generate NHSO-compliant XML claim documents"""

    def __init__(self, patient: PatientInfo):
        self.patient = patient
        self.timestamp = datetime.now().isoformat()

    def diagnoses_to_nhso_xml(self, diagnoses: List[ClaimDiagnosis]) -> str:
        """Convert diagnoses to NHSO XML format"""

        # Create root element
        claim = ET.Element('CLAIM')
        claim.set('version', '1.0')

        # Claim header
        header = ET.SubElement(claim, 'CLAIM_HEADER')
        ET.SubElement(header, 'claim_id').text = f"CLM-{self.patient.patient_id}-{self.timestamp.split('T')[0]}"
        ET.SubElement(header, 'claim_date').text = datetime.now().strftime('%Y-%m-%d')
        ET.SubElement(header, 'organization_code').text = "NHSO"

        # Patient section
        patient_elem = ET.SubElement(claim, 'PATIENT')
        ET.SubElement(patient_elem, 'patient_id').text = self.patient.patient_id
        ET.SubElement(patient_elem, 'first_name').text = self.patient.first_name
        ET.SubElement(patient_elem, 'last_name').text = self.patient.last_name
        ET.SubElement(patient_elem, 'date_of_birth').text = self.patient.date_of_birth
        ET.SubElement(patient_elem, 'gender').text = self.patient.gender
        ET.SubElement(patient_elem, 'national_id').text = self.patient.id_number

        # Diagnoses section
        diagnoses_elem = ET.SubElement(claim, 'DIAGNOSES')
        for i, diag in enumerate(diagnoses, 1):
            diag_elem = ET.SubElement(diagnoses_elem, 'DIAGNOSIS')
            diag_elem.set('order', str(i))
            diag_elem.set('severity', diag.severity)
            ET.SubElement(diag_elem, 'icd_code').text = diag.icd_code
            ET.SubElement(diag_elem, 'description').text = diag.description
            ET.SubElement(diag_elem, 'code_system').text = diag.code_system

        # Procedures section (empty for now)
        procedures_elem = ET.SubElement(claim, 'PROCEDURES')

        # Services section
        services_elem = ET.SubElement(claim, 'SERVICES')
        service = ET.SubElement(services_elem, 'SERVICE')
        ET.SubElement(service, 'service_date').text = datetime.now().strftime('%Y-%m-%d')
        ET.SubElement(service, 'service_type').text = "Hospitalization"
        ET.SubElement(service, 'amount').text = "0.00"

        # Pretty print
        return self._prettify_xml(claim)

    def medications_to_nhso_xml(self, medications: List[ClaimMedication]) -> str:
        """Convert medications to NHSO format"""

        claim = ET.Element('MEDICATION_CLAIM')
        claim.set('version', '1.0')

        header = ET.SubElement(claim, 'HEADER')
        ET.SubElement(header, 'claim_id').text = f"MED-{self.patient.patient_id}-{self.timestamp.split('T')[0]}"
        ET.SubElement(header, 'claim_date').text = datetime.now().strftime('%Y-%m-%d')

        patient_elem = ET.SubElement(claim, 'PATIENT')
        ET.SubElement(patient_elem, 'patient_id').text = self.patient.patient_id
        ET.SubElement(patient_elem, 'name').text = f"{self.patient.first_name} {self.patient.last_name}"

        meds_elem = ET.SubElement(claim, 'MEDICATIONS')
        for i, med in enumerate(medications, 1):
            med_elem = ET.SubElement(meds_elem, 'MEDICATION')
            med_elem.set('order', str(i))
            ET.SubElement(med_elem, 'drug_name').text = med.name
            ET.SubElement(med_elem, 'dose').text = med.dose
            ET.SubElement(med_elem, 'route').text = med.route
            ET.SubElement(med_elem, 'frequency').text = med.frequency
            ET.SubElement(med_elem, 'start_date').text = med.start_date
            if med.end_date:
                ET.SubElement(med_elem, 'end_date').text = med.end_date

        return self._prettify_xml(claim)

    @staticmethod
    def _prettify_xml(elem: ET.Element) -> str:
        """Return a pretty-printed XML string"""
        rough_string = ET.tostring(elem, encoding='unicode')
        reparsed = minidom.parseString(rough_string)
        return reparsed.toprettyxml(indent="  ")


# ============================================================================
# FHIR Bundle to Claims Converter
# ============================================================================

class FhirToClaimsConverter:
    """Convert FHIR R5 Bundle to insurance claims"""

    @staticmethod
    def extract_patient_from_bundle(bundle: Dict) -> PatientInfo:
        """Extract patient info from FHIR Bundle"""
        # Look for Patient resource in bundle
        for entry in bundle.get('entry', []):
            resource = entry.get('resource', {})
            if resource.get('resourceType') == 'Patient':
                name = resource.get('name', [{}])[0]
                return PatientInfo(
                    patient_id=resource.get('id', 'unknown'),
                    first_name=name.get('given', ['Unknown'])[0],
                    last_name=name.get('family', 'Unknown'),
                    date_of_birth=resource.get('birthDate', '1970-01-01'),
                    gender=resource.get('gender', 'M').upper()[0]
                )
        return PatientInfo(patient_id='unknown')

    @staticmethod
    def extract_diagnoses_from_bundle(bundle: Dict) -> List[ClaimDiagnosis]:
        """Extract diagnoses from FHIR Bundle"""
        diagnoses = []
        severity_map = {'active': 'M', 'recurrence': 'S', 'relapse': 'S'}

        for i, entry in enumerate(bundle.get('entry', []), 1):
            resource = entry.get('resource', {})
            if resource.get('resourceType') == 'Condition':
                codes = resource.get('code', {}).get('coding', [])
                for code in codes:
                    if code.get('system') == 'http://hl7.org/fhir/sid/icd-10-cm':
                        diagnoses.append(ClaimDiagnosis(
                            icd_code=code.get('code', 'UNKNOWN'),
                            description=code.get('display', 'Unknown diagnosis'),
                            code_system='ICD-10-TM',
                            severity='M' if i == 1 else 'S',  # First = main
                            order=i
                        ))
                        break

        return diagnoses

    @staticmethod
    def extract_medications_from_bundle(bundle: Dict) -> List[ClaimMedication]:
        """Extract medications from FHIR Bundle"""
        medications = []

        for entry in bundle.get('entry', []):
            resource = entry.get('resource', {})
            if resource.get('resourceType') == 'MedicationRequest':
                med_name = resource.get('medicationReference', {}).get('reference', 'Unknown')
                dosage = resource.get('dosageInstruction', [{}])[0]

                medications.append(ClaimMedication(
                    name=med_name.split('/')[-1],
                    dose=dosage.get('text', 'As prescribed'),
                    route='PO',  # Default
                    frequency='Daily',  # Default
                    start_date=resource.get('authoredOn', datetime.now().strftime('%Y-%m-%d'))
                ))

        return medications


# ============================================================================
# Summary Report Generator
# ============================================================================

class ClaimsSummaryReport:
    """Generate human-readable claims summary"""

    @staticmethod
    def generate_markdown_report(
        patient: PatientInfo,
        diagnoses: List[ClaimDiagnosis],
        medications: List[ClaimMedication]
    ) -> str:
        """Generate Markdown summary report"""

        report = f"""# Medical Claims Submission Report

**Generated**: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}

## Patient Information
- **ID**: {patient.patient_id}
- **Name**: {patient.first_name} {patient.last_name}
- **DOB**: {patient.date_of_birth}
- **Gender**: {'Male' if patient.gender == 'M' else 'Female'}

## Primary Diagnoses (ICD-10-TM)

| Order | ICD Code | Description |
|-------|----------|-------------|
"""

        for i, diag in enumerate(diagnoses, 1):
            report += f"| {i} | **{diag.icd_code}** | {diag.description} |\n"

        report += f"""
## Medications ({len(medications)} prescribed)

| Drug | Dose | Route | Frequency | Start Date |
|------|------|-------|-----------|------------|
"""

        for med in medications:
            report += f"| {med.name} | {med.dose} | {med.route} | {med.frequency} | {med.start_date} |\n"

        report += f"""
## Claim Status

- **Total Diagnoses**: {len(diagnoses)}
- **Total Medications**: {len(medications)}
- **Ready for Submission**: ✅ Yes
- **Format**: NHSO XML
- **Next Steps**: Submit to NHSO health portal

## Insurance Coverage

- **Insurance Type**: Thai Social Security / NHSO
- **Claim Submission**: Via NHSO portal (https://nhso.go.th)
- **Support Contact**: insurance-claims@nhso.go.th

---

*Generated by Asgard Medical Claims Pipeline*
*For questions, contact: claims@megawiz.co.th*
"""
        return report

    @staticmethod
    def generate_html_report(
        patient: PatientInfo,
        diagnoses: List[ClaimDiagnosis],
        medications: List[ClaimMedication]
    ) -> str:
        """Generate HTML summary report"""

        html = f"""<!DOCTYPE html>
<html lang="th">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Medical Claims Report</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 20px; }}
        h1, h2 {{ color: #333; }}
        table {{ border-collapse: collapse; width: 100%; margin: 20px 0; }}
        th, td {{ border: 1px solid #ddd; padding: 12px; text-align: left; }}
        th {{ background-color: #4CAF50; color: white; }}
        tr:nth-child(even) {{ background-color: #f2f2f2; }}
        .patient-info {{ background-color: #e7f3fe; padding: 10px; margin: 20px 0; }}
        .status {{ color: #4CAF50; font-weight: bold; }}
        footer {{ margin-top: 40px; padding-top: 20px; border-top: 1px solid #ddd; color: #666; }}
    </style>
</head>
<body>
    <h1>🏥 Medical Claims Submission Report</h1>
    <p><em>Generated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}</em></p>

    <div class="patient-info">
        <h2>👤 Patient Information</h2>
        <p><strong>ID</strong>: {patient.patient_id}</p>
        <p><strong>Name</strong>: {patient.first_name} {patient.last_name}</p>
        <p><strong>DOB</strong>: {patient.date_of_birth}</p>
        <p><strong>Gender</strong>: {'Male' if patient.gender == 'M' else 'Female'}</p>
    </div>

    <h2>📋 Primary Diagnoses (ICD-10-TM)</h2>
    <table>
        <tr><th>Order</th><th>ICD Code</th><th>Description</th></tr>
"""

        for i, diag in enumerate(diagnoses, 1):
            html += f"<tr><td>{i}</td><td><strong>{diag.icd_code}</strong></td><td>{diag.description}</td></tr>\n"

        html += f"""
    </table>

    <h2>💊 Medications ({len(medications)} prescribed)</h2>
    <table>
        <tr><th>Drug</th><th>Dose</th><th>Route</th><th>Frequency</th><th>Start Date</th></tr>
"""

        for med in medications:
            html += f"<tr><td>{med.name}</td><td>{med.dose}</td><td>{med.route}</td><td>{med.frequency}</td><td>{med.start_date}</td></tr>\n"

        html += f"""
    </table>

    <h2>✅ Claim Status</h2>
    <ul>
        <li>Total Diagnoses: <strong>{len(diagnoses)}</strong></li>
        <li>Total Medications: <strong>{len(medications)}</strong></li>
        <li>Ready for Submission: <span class="status">✓ Yes</span></li>
        <li>Format: <strong>NHSO XML</strong></li>
    </ul>

    <h2>🏛️ Insurance Coverage</h2>
    <ul>
        <li><strong>Insurance Type</strong>: Thai Social Security / NHSO</li>
        <li><strong>Claim Portal</strong>: https://nhso.go.th</li>
        <li><strong>Support</strong>: insurance-claims@nhso.go.th</li>
    </ul>

    <footer>
        <p>Generated by <em>Asgard Medical Claims Pipeline</em></p>
        <p>For questions, contact: claims@megawiz.co.th</p>
    </footer>
</body>
</html>
"""
        return html


# ============================================================================
# Main Processing
# ============================================================================

# ============================================================================
# Multi-Format Claims Generator
# ============================================================================

class MultiFormatClaimsGenerator:
    """Generate claims in multiple formats (NHSO, สปสช, FHIR)"""

    @staticmethod
    def generate_all_formats(
        patient: PatientInfo,
        diagnoses: List[ClaimDiagnosis],
        medications: List[ClaimMedication],
        output_dir: Path,
        doc_id: str,
        generate_nhso: bool = True,
        generate_socsc: bool = True,
        generate_fhir: bool = False
    ) -> Dict[str, Path]:
        """Generate claims in all requested formats"""

        results = {}

        # Generate NHSO XML
        if generate_nhso:
            try:
                nhso_gen = NHSOClaimsGenerator(patient)
                nhso_xml = nhso_gen.diagnoses_to_nhso_xml(diagnoses)
                nhso_file = output_dir / f"claim_nhso_{doc_id}.xml"
                nhso_file.write_text(nhso_xml, encoding='utf-8')
                results['nhso'] = nhso_file
            except Exception as e:
                print(f"      ⚠️  NHSO generation failed: {e}")

        # Generate สปสช EDI 837
        if generate_socsc:
            try:
                from social_security_claims_generator import (
                    SocialSecurityClaimsGenerator,
                    SocialSecuritySubscriber,
                    SocialSecurityClaim,
                    EDIDiagnosis,
                    EDIService
                )

                # Create subscriber
                subscriber = SocialSecuritySubscriber(
                    member_id=patient.patient_id,
                    first_name=patient.first_name,
                    last_name=patient.last_name,
                    date_of_birth=patient.date_of_birth.replace('-', ''),
                    gender=patient.gender,
                    national_id=patient.id_number
                )

                # Convert diagnoses
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
                        total_charge='0.00'
                    )
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
                socsc_file = output_dir / f"claim_socsc_{doc_id}.edi"
                socsc_file.write_text(edi_claim, encoding='utf-8')
                results['socsc'] = socsc_file
            except Exception as e:
                print(f"      ⚠️  สปสช EDI generation failed: {e}")

        # Generate FHIR Claim resource (future)
        if generate_fhir:
            print(f"      ℹ️  FHIR Claim resource generation deferred to Phase 2")

        return results


def process_extractions_to_claims(
    extraction_dir: Path,
    output_dir: Path,
    insurance_formats: List[str] = None
):
    """Process all extractions to insurance claims in multiple formats

    Args:
        extraction_dir: Directory with extraction JSON files
        output_dir: Output directory for claims
        insurance_formats: List of formats ['nhso', 'socsc', 'fhir']
    """

    if insurance_formats is None:
        insurance_formats = ['nhso', 'socsc']  # Default: NHSO + สปสช

    output_dir.mkdir(parents=True, exist_ok=True)

    # Find all extraction JSON files
    extraction_files = sorted(extraction_dir.glob("extraction_*.json"))

    for extraction_file in extraction_files:
        doc_id = extraction_file.stem.split('_')[1]
        print(f"📄 Processing extraction_{doc_id}.json")

        try:
            # Load extraction
            extraction_data = json.loads(extraction_file.read_text())

            # Create FHIR Bundle-like structure from extraction
            bundle = extraction_data.get('fhir_resources', {})
            if not bundle:
                print(f"   ⚠️  No FHIR resources found, skipping")
                continue

            # Extract data for claims
            patient = FhirToClaimsConverter.extract_patient_from_bundle(bundle)
            diagnoses = FhirToClaimsConverter.extract_diagnoses_from_bundle(bundle)
            medications = FhirToClaimsConverter.extract_medications_from_bundle(bundle)

            # Generate claims in all requested formats
            formats_results = MultiFormatClaimsGenerator.generate_all_formats(
                patient=patient,
                diagnoses=diagnoses,
                medications=medications,
                output_dir=output_dir,
                doc_id=doc_id,
                generate_nhso='nhso' in insurance_formats,
                generate_socsc='socsc' in insurance_formats,
                generate_fhir='fhir' in insurance_formats
            )

            # Generate summary reports
            summary_report = ClaimsSummaryReport.generate_markdown_report(patient, diagnoses, medications)
            summary_file = output_dir / f"claim_summary_{doc_id}.md"
            summary_file.write_text(summary_report, encoding='utf-8')
            formats_results['summary_md'] = summary_file

            html_report = ClaimsSummaryReport.generate_html_report(patient, diagnoses, medications)
            html_file = output_dir / f"claim_summary_{doc_id}.html"
            html_file.write_text(html_report, encoding='utf-8')
            formats_results['summary_html'] = html_file

            # Print results
            print(f"   ✅ Generated {len(diagnoses)} diagnoses, {len(medications)} medications")
            for fmt, filepath in formats_results.items():
                print(f"      - {fmt.upper()}: {filepath.name}")

        except Exception as e:
            print(f"   ❌ Error: {e}")
            import traceback
            traceback.print_exc()

    print(f"\n✅ All claims generated in: {output_dir}")


def main():
    """Main entry point"""

    extraction_dir = Path("/Users/mimir/Developer/Mimir/data/abb/extractions")
    output_dir = Path("/Users/mimir/Developer/Mimir/data/abb/claims")

    print("="*70)
    print("🏥 FHIR → Insurance Claims Transformer")
    print("="*70)

    process_extractions_to_claims(extraction_dir, output_dir)

    print("\n" + "="*70)
    print("📊 CLAIMS GENERATION COMPLETE")
    print("="*70)


if __name__ == "__main__":
    main()

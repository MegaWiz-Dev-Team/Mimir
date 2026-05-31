#!/usr/bin/env python3
"""
สปสช (Social Security Office) Claims Generator
EDI 837 format for Thai Social Security Office claim submission
Maps FHIR R5 to EDI 837-I (Institutional) Healthcare Claim
"""

from datetime import datetime
from typing import List, Optional, Dict, Any
from dataclasses import dataclass, field
import json


# ============================================================================
# EDI 837 Data Models for สปสช
# ============================================================================

@dataclass
class EDIElement:
    """Single EDI element (data element)"""
    value: str

    def __str__(self):
        return self.value


@dataclass
class EDISegment:
    """EDI segment with elements separated by *"""
    tag: str  # ISA, GS, ST, etc.
    elements: List[str]

    def to_edi(self, segment_terminator: str = '~') -> str:
        """Convert to EDI format"""
        return self.tag + '*' + '*'.join(self.elements) + segment_terminator


@dataclass
class SocialSecuritySubscriber:
    """สปสช Subscriber (Social Security member) information"""
    member_id: str  # สปสช member ID (หมายเลขประวัติสมาชิก)
    first_name: str
    last_name: str
    date_of_birth: str  # YYYYMMDD
    gender: str  # M or F
    national_id: str  # ID card number
    employee_id: Optional[str] = None  # Employer ID
    workplace_name: Optional[str] = None
    relationship_to_patient: str = "Self"  # Self, Spouse, Child, Parent


@dataclass
class EDIDiagnosis:
    """EDI 837 Diagnosis format"""
    icd_code: str  # ICD-10-TM code
    description: str
    qualifier: str = "ABJ"  # ABJ = Principal Diagnosis
    severity: Optional[str] = None


@dataclass
class EDIService:
    """EDI 837 Service/Procedure line item"""
    service_date: str  # YYYYMMDD
    procedure_code: str  # CPT or Thai HFIS code
    procedure_desc: str
    quantity: str = "1"
    unit_price: str = "0"
    total_charge: str = "0"
    service_type_code: str = "30"  # 30=Professional Service


@dataclass
class SocialSecurityClaim:
    """Complete สปสช EDI 837 claim"""
    claim_id: str
    claim_date: str  # YYYYMMDD
    subscriber: SocialSecuritySubscriber
    patient_first_name: str
    patient_last_name: str
    patient_dob: str  # YYYYMMDD
    diagnoses: List[EDIDiagnosis] = field(default_factory=list)
    services: List[EDIService] = field(default_factory=list)
    facility_code: str = "999999"  # Thai facility code
    provider_npi: str = "9999999999"  # Provider ID


# ============================================================================
# EDI 837 Control Numbers
# ============================================================================

class EDIControlNumbers:
    """Manage EDI control/reference numbers"""

    def __init__(self):
        self.interchange_control = 1
        self.group_control = 1
        self.transaction_control = 1
        self.segment_count = 0

    def next_interchange(self) -> str:
        result = str(self.interchange_control).zfill(9)
        self.interchange_control += 1
        return result

    def next_group(self) -> str:
        result = str(self.group_control).zfill(5)
        self.group_control += 1
        return result

    def next_transaction(self) -> str:
        result = str(self.transaction_control).zfill(4)
        self.transaction_control += 1
        return result

    def increment_segment(self):
        self.segment_count += 1
        return str(self.segment_count).zfill(4)


# ============================================================================
# สปสช EDI 837 Generator
# ============================================================================

class SocialSecurityClaimsGenerator:
    """Generate EDI 837 claims for Thai Social Security Office (สปสช)"""

    # EDI Delimiters
    SEGMENT_TERMINATOR = "~"
    ELEMENT_SEPARATOR = "*"
    COMPONENT_SEPARATOR = ":"

    def __init__(self):
        self.control_numbers = EDIControlNumbers()
        self.submitter_id = "ASGARD001"  # Organization ID
        self.submitter_name = "Asgard Medical"
        self.receiver_id = "SOCSC001"  # สปสช receiver ID
        self.receiver_name = "SOCIAL SECURITY OFFICE"
        self.edi_version = "005010X222"  # Version for 837-I

    def generate_edi_837_claim(self, claim: SocialSecurityClaim) -> str:
        """Generate complete EDI 837 claim for สปสช"""

        segments = []

        # ISA: Interchange Control Header
        segments.append(self._segment_isa())

        # GS: Group Header
        segments.append(self._segment_gs())

        # ST: Transaction Set Header
        transaction_control = self.control_numbers.next_transaction()
        segments.append(self._segment_st(transaction_control))

        # BHT: Beginning of Hierarchical Transaction
        segments.append(self._segment_bht(claim.claim_date))

        # NM1: Submitter Name
        segments.append(self._segment_nm1_submitter())

        # NM1: Receiver (สปสช)
        segments.append(self._segment_nm1_receiver())

        # NM1: Subscriber (Employee/Member)
        segments.append(self._segment_nm1_subscriber(claim.subscriber))

        # NM1: Patient
        segments.append(self._segment_nm1_patient(claim))

        # HL: Hierarchical Level (Patient)
        segments.append(self._segment_hl_patient())

        # SBR: Subscriber Information
        segments.append(self._segment_sbr(claim.subscriber))

        # OI: Other Insurance
        segments.append(self._segment_oi())

        # NM1: Provider (Facility)
        segments.append(self._segment_nm1_provider())

        # CLM: Claim Information
        segments.append(self._segment_clm(claim))

        # DTP: Date - Service Date
        if claim.services:
            segments.append(self._segment_dtp_service(claim.services[0].service_date))

        # CL1: Claim Code Information
        segments.append(self._segment_cl1())

        # HI: Health Care Diagrams/Procedures
        segments.append(self._segment_hi(claim.diagnoses))

        # NTE: Notes
        segments.append(self._segment_nte())

        # LX: Service Line Number
        segments.append(self._segment_lx(claim.services))

        # Service Line Items (SVC)
        for service in claim.services:
            segments.append(self._segment_sv1(service))

        # SE: Transaction Set Trailer
        segments.append(self._segment_se(transaction_control, len(segments) + 1))

        # GE: Group Trailer
        segments.append(self._segment_ge(1))

        # IEA: Interchange Trailer
        segments.append(self._segment_iea(1))

        # Join with segment terminator
        return self.SEGMENT_TERMINATOR.join([seg for seg in segments if seg])

    # ========================================================================
    # EDI Segment Methods
    # ========================================================================

    def _segment_isa(self) -> str:
        """ISA: Interchange Control Header"""
        segments = [
            "00",  # Auth Info Qualifier
            "          ",  # Auth Info (10 spaces)
            "00",  # Security Info Qualifier
            "          ",  # Security Info (10 spaces)
            "ZZ",  # Interchange ID Qualifier (ZZ=mutually defined)
            "ASGARD001     ",  # Interchange Sender ID (15 chars)
            "ZZ",  # Interchange ID Qualifier
            "SOCSC001      ",  # Interchange Receiver ID (15 chars)
            datetime.now().strftime("%y%m%d"),  # Interchange Date
            datetime.now().strftime("%H%M"),  # Interchange Time
            "^",  # Repetition Separator
            self.edi_version,  # Interchange Control Version
            self.control_numbers.next_interchange(),  # Interchange Control Number
            "0",  # Ack Requested
            "T",  # Usage Indicator (T=test, P=production)
            ":"  # Component Element Separator
        ]
        return "ISA" + self.ELEMENT_SEPARATOR + self.ELEMENT_SEPARATOR.join(segments)

    def _segment_gs(self) -> str:
        """GS: Group Header"""
        segments = [
            "HC",  # Code identifying functional group (HC=Healthcare)
            self.submitter_id,  # Application Sender's Code
            self.receiver_id,  # Application Receiver's Code
            datetime.now().strftime("%Y%m%d"),  # Date
            datetime.now().strftime("%H%M%S"),  # Time
            self.control_numbers.next_group(),  # Group Control Number
            "X",  # Responsible Agency Code
            self.edi_version  # Version
        ]
        return "GS" + self.ELEMENT_SEPARATOR + self.ELEMENT_SEPARATOR.join(segments)

    def _segment_st(self, transaction_control: str) -> str:
        """ST: Transaction Set Header"""
        segments = [
            "837",  # Transaction Set Identifier (837=Healthcare Claim)
            transaction_control  # Transaction Set Control Number
        ]
        return "ST" + self.ELEMENT_SEPARATOR + self.ELEMENT_SEPARATOR.join(segments)

    def _segment_bht(self, claim_date: str) -> str:
        """BHT: Beginning of Hierarchical Transaction"""
        segments = [
            "0019",  # Hierarchical Structure Code
            "00",  # Billing/Service Period Type Code
            "0121",  # Claim Submission Reason Code
            claim_date[:4] + claim_date[4:6] + claim_date[6:8],  # Claim Date
            claim_date[:4] + claim_date[4:6] + claim_date[6:8],  # Claim Time
            "CH"  # Claim Type Code (CH=Chiropractic, use for all institutional)
        ]
        return "BHT" + self.ELEMENT_SEPARATOR + self.ELEMENT_SEPARATOR.join(segments)

    def _segment_nm1_submitter(self) -> str:
        """NM1: Submitter Name"""
        segments = [
            "41",  # Entity Identifier Code (41=Submitter)
            "2",  # Entity Type Qualifier (2=Organization)
            self.submitter_name,  # Name
            "",  # First Name
            "",  # Middle Name
            "",  # Name Prefix
            "",  # Name Suffix
            "",  # Identification Code Qualifier
            self.submitter_id,  # Identification Code
        ]
        return "NM1" + self.ELEMENT_SEPARATOR + self.ELEMENT_SEPARATOR.join(segments)

    def _segment_nm1_receiver(self) -> str:
        """NM1: Receiver (สปสช)"""
        segments = [
            "40",  # Entity Identifier Code (40=Receiver)
            "2",  # Entity Type Qualifier (2=Organization)
            self.receiver_name,  # Name
            "",  # First Name
            "",  # Middle Name
            "",  # Name Prefix
            "",  # Name Suffix
            "",  # Identification Code Qualifier
            self.receiver_id,  # Identification Code
        ]
        return "NM1" + self.ELEMENT_SEPARATOR + self.ELEMENT_SEPARATOR.join(segments)

    def _segment_nm1_subscriber(self, subscriber: SocialSecuritySubscriber) -> str:
        """NM1: Subscriber (สปสช Member)"""
        segments = [
            "IL",  # Entity Identifier Code (IL=Insured or Subscriber)
            "1",  # Entity Type Qualifier (1=Person)
            subscriber.last_name,  # Last Name
            subscriber.first_name,  # First Name
            "",  # Middle Name
            "",  # Name Prefix
            "",  # Name Suffix
            "34",  # Identification Code Qualifier (34=Social Security Number, adapt for Thai ID)
            subscriber.member_id,  # Identification Code (สปสช Member ID)
        ]
        return "NM1" + self.ELEMENT_SEPARATOR + self.ELEMENT_SEPARATOR.join(segments)

    def _segment_nm1_patient(self, claim: SocialSecurityClaim) -> str:
        """NM1: Patient Name"""
        segments = [
            "QC",  # Entity Identifier Code (QC=Patient)
            "1",  # Entity Type Qualifier (1=Person)
            claim.patient_last_name,  # Last Name
            claim.patient_first_name,  # First Name
            "",  # Middle Name
            "",  # Name Prefix
            "",  # Name Suffix
            "34",  # Identification Code Qualifier
            claim.claim_id,  # Use claim ID as patient reference
        ]
        return "NM1" + self.ELEMENT_SEPARATOR + self.ELEMENT_SEPARATOR.join(segments)

    def _segment_hl_patient(self) -> str:
        """HL: Hierarchical Level - Patient Level"""
        segments = [
            "1",  # Hierarchical ID Number
            "",  # Hierarchical Parent ID Number
            "22",  # Hierarchical Level Code (22=Patient)
            "0"  # Hierarchical Child Code
        ]
        return "HL" + self.ELEMENT_SEPARATOR + self.ELEMENT_SEPARATOR.join(segments)

    def _segment_sbr(self, subscriber: SocialSecuritySubscriber) -> str:
        """SBR: Subscriber Information"""
        segments = [
            "",  # Payer Responsibility Sequence (primary)
            "18",  # Individual Relationship Code (18=Self)
            "",  # Reference ID (Employee ID)
            "",  # Group or Policy Number
            "",  # Group or Policy Name
            "11",  # Insurance Type Code (11=Preferred Provider Organization)
            "",  # Coordination of Benefits Code
            "",  # Yes/No Condition or Response Code
            "",  # Employment Status Code
            ""  # Claim Filing Indicator Code
        ]
        return "SBR" + self.ELEMENT_SEPARATOR + self.ELEMENT_SEPARATOR.join(segments)

    def _segment_oi(self) -> str:
        """OI: Other Insurance"""
        segments = [
            "B",  # Claim Submission Reason Code
            "B",  # Benefit Assignment Code
            "N",  # Patient Signature Source Code
            "",  # Provider Agreement Code
            "N"  # Release of Information Code
        ]
        return "OI" + self.ELEMENT_SEPARATOR + self.ELEMENT_SEPARATOR.join(segments)

    def _segment_nm1_provider(self) -> str:
        """NM1: Provider/Facility Name"""
        segments = [
            "82",  # Entity Identifier Code (82=Rendering Provider)
            "2",  # Entity Type Qualifier (2=Organization)
            "ASGARD MEDICAL FACILITY",  # Organization Name
            "",  # First Name
            "",  # Middle Name
            "",  # Name Prefix
            "",  # Name Suffix
            "34",  # Identification Code Qualifier
            self.control_numbers.submitter_id if hasattr(self.control_numbers, 'submitter_id') else "9999999999",  # Provider NPI
        ]
        return "NM1" + self.ELEMENT_SEPARATOR + self.ELEMENT_SEPARATOR.join(segments)

    def _segment_clm(self, claim: SocialSecurityClaim) -> str:
        """CLM: Claim Information"""
        segments = [
            claim.claim_id,  # Claim Submission Number
            "11",  # Claim Status Code (11=Not Yet Submitted)
            "11",  # Claim Frequency Code (11=Original)
            "B",  # Facility Code Qualifier
            claim.facility_code,  # Facility Code
            "",  # Service Type Code
            "N",  # Provider or Supplier Signature Indicator
            "N",  # Patient Signature Source Code
            "N",  # Provider Accept Assignment Code
            "N",  # Beneficiary Signature Source Code
            "",  # Assign Benefits Code
            "11"  # Release of Information Code
        ]
        return "CLM" + self.ELEMENT_SEPARATOR + self.ELEMENT_SEPARATOR.join(segments)

    def _segment_dtp_service(self, service_date: str) -> str:
        """DTP: Date - Service Date Period"""
        segments = [
            "434",  # Date/Time Qualifier (434=Service Date)
            "D8",  # Date Period Format Qualifier (D8=CCYYMMDD)
            service_date[:4] + service_date[4:6] + service_date[6:8]  # Service Date
        ]
        return "DTP" + self.ELEMENT_SEPARATOR + self.ELEMENT_SEPARATOR.join(segments)

    def _segment_cl1(self) -> str:
        """CL1: Institutional Claim Code"""
        segments = [
            "11",  # Facility Type Code
            "C",  # Claim Frequency Code
            "1"  # Patient Status Code
        ]
        return "CL1" + self.ELEMENT_SEPARATOR + self.ELEMENT_SEPARATOR.join(segments)

    def _segment_hi(self, diagnoses: List[EDIDiagnosis]) -> str:
        """HI: Health Care Diagnosis/Procedures"""
        segments = ["HI"]

        for i, diag in enumerate(diagnoses):
            if i == 0:
                # Principal diagnosis
                code_type = "ABK"  # ABK=ICD-10-CM (use for ICD-10-TM)
            else:
                # Secondary diagnoses
                code_type = "ABJ"  # ABJ=ICD-10-CM

            segments.append(f"{code_type}{self.COMPONENT_SEPARATOR}{diag.icd_code}")

        return self.ELEMENT_SEPARATOR.join(segments)

    def _segment_nte(self, note: str = "Social Security Claim Submission") -> str:
        """NTE: Note/Special Instruction"""
        segments = [
            "NTE",
            "",  # Note Reference Code
            note  # Note Text
        ]
        return self.ELEMENT_SEPARATOR.join(segments)

    def _segment_lx(self, services: List[EDIService]) -> str:
        """LX: Service Line Number"""
        segments = [
            "LX",
            str(len(services))  # Line Item Count
        ]
        return self.ELEMENT_SEPARATOR.join(segments)

    def _segment_sv1(self, service: EDIService) -> str:
        """SV1: Service Line Detail (Professional Service)"""
        segments = [
            "SV1",
            f"HC{self.COMPONENT_SEPARATOR}{service.procedure_code}",  # Procedure Code
            service.total_charge,  # Line Item Charge Amount
            "UN",  # Unit or Basis for Measurement Code
            service.quantity,  # Service Unit Count
            "",  # Place of Service Code
            "",  # Service Type Code
            "",  # Procedure Modifier
            "",  # Description
            ""  # EPSDT Indicator
        ]
        return self.ELEMENT_SEPARATOR.join(segments)

    def _segment_se(self, transaction_control: str, segment_count: int) -> str:
        """SE: Transaction Set Trailer"""
        segments = [
            "SE",
            str(segment_count),  # Number of Included Segments
            transaction_control  # Transaction Set Control Number
        ]
        return self.ELEMENT_SEPARATOR.join(segments)

    def _segment_ge(self, group_count: int) -> str:
        """GE: Group Trailer"""
        segments = [
            "GE",
            str(group_count),  # Number of Transaction Sets
            self.control_numbers.group_control  # Group Control Number
        ]
        return self.ELEMENT_SEPARATOR.join([segments[0], segments[1], str(self.control_numbers.group_control - 1).zfill(5)])

    def _segment_iea(self, interchange_count: int) -> str:
        """IEA: Interchange Trailer"""
        segments = [
            "IEA",
            str(interchange_count),  # Number of Functional Groups
            self.control_numbers.interchange_control  # Interchange Control Number
        ]
        return self.ELEMENT_SEPARATOR.join([segments[0], segments[1], str(self.control_numbers.interchange_control - 1).zfill(9)])


# ============================================================================
# FHIR to Social Security Converter
# ============================================================================

class FhirToSocialSecurityConverter:
    """Convert FHIR R5 to สปสช EDI 837 format"""

    @staticmethod
    def extract_subscriber_from_bundle(bundle: Dict[str, Any]) -> SocialSecuritySubscriber:
        """Extract subscriber info from FHIR Bundle"""
        for entry in bundle.get('entry', []):
            resource = entry.get('resource', {})
            if resource.get('resourceType') == 'Patient':
                name = resource.get('name', [{}])[0]
                return SocialSecuritySubscriber(
                    member_id=resource.get('id', 'unknown'),
                    first_name=name.get('given', ['Unknown'])[0],
                    last_name=name.get('family', 'Unknown'),
                    date_of_birth=resource.get('birthDate', '19700101').replace('-', ''),
                    gender=resource.get('gender', 'M').upper()[0],
                    national_id=resource.get('identifier', [{}])[0].get('value', ''),
                )
        return SocialSecuritySubscriber(
            member_id='unknown',
            first_name='Unknown',
            last_name='Unknown',
            date_of_birth='19700101',
            gender='M',
            national_id=''
        )

    @staticmethod
    def extract_diagnoses_from_bundle(bundle: Dict[str, Any]) -> List[EDIDiagnosis]:
        """Extract diagnoses from FHIR Bundle"""
        diagnoses = []

        for i, entry in enumerate(bundle.get('entry', []), 1):
            resource = entry.get('resource', {})
            if resource.get('resourceType') == 'Condition':
                codes = resource.get('code', {}).get('coding', [])
                for code in codes:
                    qualifier = 'ABK' if i == 1 else 'ABJ'  # First=principal, rest=secondary
                    diagnoses.append(EDIDiagnosis(
                        icd_code=code.get('code', 'UNKNOWN'),
                        description=code.get('display', 'Unknown'),
                        qualifier=qualifier
                    ))
                    break

        return diagnoses

    @staticmethod
    def extract_services_from_bundle(bundle: Dict[str, Any]) -> List[EDIService]:
        """Extract services from FHIR Bundle"""
        services = []

        for entry in bundle.get('entry', []):
            resource = entry.get('resource', {})
            if resource.get('resourceType') == 'Procedure':
                services.append(EDIService(
                    service_date=resource.get('performedDateTime', '').replace('-', '')[:8] or datetime.now().strftime('%Y%m%d'),
                    procedure_code=resource.get('code', {}).get('coding', [{}])[0].get('code', '99999'),
                    procedure_desc=resource.get('code', {}).get('coding', [{}])[0].get('display', 'Medical Procedure'),
                ))

        if not services:
            # Default service if none found
            services.append(EDIService(
                service_date=datetime.now().strftime('%Y%m%d'),
                procedure_code='99213',  # Office visit
                procedure_desc='Office Visit - Established Patient',
            ))

        return services


# ============================================================================
# Main Processing
# ============================================================================

def main():
    """Test สปสช EDI 837 generation"""

    # Create sample subscriber
    subscriber = SocialSecuritySubscriber(
        member_id="SSO123456789",
        first_name="สมชาย",
        last_name="ใจดี",
        date_of_birth="19700115",
        gender="M",
        national_id="1234567890123",
        employee_id="EMP-001",
        workplace_name="Bangkok Hospital"
    )

    # Create sample claim
    claim = SocialSecurityClaim(
        claim_id="CLM-001-20260528",
        claim_date=datetime.now().strftime('%Y%m%d'),
        subscriber=subscriber,
        patient_first_name="สมชาย",
        patient_last_name="ใจดี",
        patient_dob="19700115",
        diagnoses=[
            EDIDiagnosis(icd_code="I10", description="Essential Hypertension", qualifier="ABK"),
            EDIDiagnosis(icd_code="E11.9", description="Type 2 Diabetes Mellitus", qualifier="ABJ"),
        ],
        services=[
            EDIService(
                service_date="20260528",
                procedure_code="99213",
                procedure_desc="Office Visit",
                quantity="1",
                total_charge="500.00"
            ),
            EDIService(
                service_date="20260528",
                procedure_code="36415",
                procedure_desc="Routine Blood Draw",
                quantity="1",
                total_charge="200.00"
            ),
        ]
    )

    # Generate EDI 837
    generator = SocialSecurityClaimsGenerator()
    edi_claim = generator.generate_edi_837_claim(claim)

    # Output
    print("=" * 70)
    print("สปสช (Social Security Office) EDI 837 Claim")
    print("=" * 70)
    print(edi_claim)
    print("=" * 70)

    # Save to file
    output_file = Path("/Users/mimir/Developer/Mimir/data/claims/socsc_claim_sample.edi")
    output_file.parent.mkdir(parents=True, exist_ok=True)
    output_file.write_text(edi_claim, encoding='utf-8')
    print(f"\n✅ Saved to: {output_file}")


if __name__ == "__main__":
    from pathlib import Path
    main()

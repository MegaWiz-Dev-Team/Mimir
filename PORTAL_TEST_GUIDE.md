# 🧪 NHSO + สปสช Portal Testing Guide

**Objective**: Validate claim formats with real insurance portals before production launch  
**Status**: Ready for testing phase  
**Timeline**: 1-2 weeks (portal access dependent)

---

## Prerequisites

### NHSO (National Health Security Office)

**Get Access**:
- 📧 Contact: NHSO IT/EDI team
- 🔗 Portal: https://claim.nhso.go.th (or similar)
- 📋 Required: 
  - Hospital/Clinic registration code
  - User credentials
  - Test environment access

**Test Portal URL**: (Confirm with NHSO)
```
https://claim-test.nhso.go.th/
or
https://portal.nhso.go.th/test
```

### สปสช (Social Security Office)

**Get Access**:
- 📧 Contact: สปสช EDI/Claims team
- 🔗 Portal: https://www.sso.go.th (check for claims portal)
- 📋 Required:
  - Organization ID
  - EDI submission credentials
  - Test account

**Test Portal URL**: (Confirm with สปสช)
```
https://claim-test.sso.go.th/
or
EDI over SFTP/API endpoint
```

---

## Test Data Package

### Sample Claims Generated ✅

Located: `/Users/mimir/Developer/Mimir/data/claims/`

```
✅ test_claim_nhso.xml
   - Patient: สมชาย ใจดี (ID: PAT-001)
   - Diagnoses: I10, E11.9, J91.8 (ICD-10-TM codes)
   - Format: XML, well-formed, all required fields

✅ test_claim_socsc.edi
   - Member: สมชาย ใจดี (ID: PAT-001, Member: SSO123456789)
   - Services: 2 line items (99213, 36415)
   - Format: EDI 837-I, X12 5010 compliant

✅ Real extraction claims (3 documents)
   - claim_nhso_1.xml, claim_nhso_2.xml, claim_nhso_3.xml
   - claim_socsc_1.edi, claim_socsc_2.edi, claim_socsc_3.edi
   - Actual medical data from test documents
```

---

## NHSO XML Validation Checklist

### Before Portal Submission

- [ ] **File Format**
  - [ ] Valid XML (parseable by XML validators)
  - [ ] UTF-8 encoding
  - [ ] Proper XML declaration: `<?xml version="1.0" ?>`

- [ ] **Root Element**
  - [ ] `<CLAIM version="1.0">` present
  - [ ] All required child elements present

- [ ] **CLAIM_HEADER**
  - [ ] `<claim_id>` - format: `CLM-[patient_id]-[YYYY-MM-DD]`
  - [ ] `<claim_date>` - format: `YYYY-MM-DD`
  - [ ] `<organization_code>` - value: `NHSO`

- [ ] **PATIENT**
  - [ ] `<patient_id>` - unique identifier
  - [ ] `<first_name>` - Thai characters OK
  - [ ] `<last_name>` - Thai characters OK
  - [ ] `<date_of_birth>` - format: `YYYY-MM-DD`
  - [ ] `<gender>` - value: `M` or `F`
  - [ ] `<national_id>` - 13-digit Thai ID (check digit validation)

- [ ] **DIAGNOSES**
  - [ ] At least 1 `<DIAGNOSIS>` element
  - [ ] `<icd_code>` - valid ICD-10-TM code (e.g., `I10`, `E11.9`)
  - [ ] `<description>` - non-empty
  - [ ] `<code_system>` - value: `ICD-10-TM`
  - [ ] `<severity>` - first diagnosis: `M`, others: `S`
  - [ ] `order` attribute - sequential (1, 2, 3...)

- [ ] **SERVICES**
  - [ ] At least 1 `<SERVICE>` element
  - [ ] `<service_date>` - format: `YYYY-MM-DD`
  - [ ] `<service_type>` - value present
  - [ ] `<amount>` - numeric value (can be 0.00 for testing)

### Portal Submission Steps

```
1. Login to NHSO test portal
   → Username: [hospital credentials]
   → Password: [portal password]

2. Navigate to: Claims → New Claim → Upload XML

3. Select file: test_claim_nhso.xml (or real extraction file)

4. Verify preview:
   ✓ Patient name matches
   ✓ Diagnoses display correctly
   ✓ Dates are correct

5. Submit claim

6. Check response:
   ✓ Success: "Claim submitted successfully"
   ✓ Error: Check error message (format issue? ICD code invalid?)
   ✓ Validation: Any warnings about data?

7. Capture confirmation number
```

### Expected Response (Success)

```
✅ Status: SUBMITTED
📋 Confirmation Number: [auto-generated]
📊 Claim ID: CLM-PAT-001-2026-05-28
💰 Amount: TBD
⏰ Timestamp: [submission time]
```

### Common NHSO Errors & Fixes

| Error | Cause | Fix |
|-------|-------|-----|
| `Invalid XML format` | Malformed XML | Validate with XML parser first |
| `ICD code not found` | Invalid ICD-10-TM code | Check against ICD-10-TM list |
| `Patient ID missing` | Empty patient_id | Ensure all patient fields populated |
| `Duplicate claim` | Same claim_id submitted twice | Use unique claim_id per submission |
| `Date format invalid` | Wrong date format | Ensure YYYY-MM-DD format |

---

## สปสช EDI 837 Validation Checklist

### Before Portal Submission

- [ ] **File Format**
  - [ ] EDI text file (not binary)
  - [ ] Line terminator: `~` (tilde)
  - [ ] Element separator: `*` (asterisk)
  - [ ] Component separator: `:` (colon)
  - [ ] No spaces around delimiters

- [ ] **Envelope Segments**
  - [ ] `ISA` segment present (Interchange header)
    - [ ] ISA01-ISA08: Standard header fields
    - [ ] ISA09: Repetition separator (^)
    - [ ] ISA16: Component separator (:)
  - [ ] `GS` segment present (Group header)
    - [ ] GS01: `HC` (Healthcare)
    - [ ] GS06: Group control number (sequential)
  - [ ] `ST` segment present (Transaction set)
    - [ ] ST01: `837` (Healthcare Claim)
    - [ ] ST02: Transaction control number

- [ ] **Claim Data Segments**
  - [ ] `BHT` segment (Claim begin)
    - [ ] BHT03: `0121` (Original claim)
    - [ ] BHT04-05: Service date (YYYYMMDD)
  - [ ] `NM1` segments (Names)
    - [ ] NM1*41 = Submitter (hospital)
    - [ ] NM1*40 = Receiver (สปสช)
    - [ ] NM1*IL = Subscriber/Member
    - [ ] NM1*QC = Patient
    - [ ] NM1*82 = Provider
  - [ ] `CLM` segment (Claim info)
    - [ ] CLM01: Claim ID
    - [ ] CLM02: `11` (Not yet submitted)
    - [ ] CLM11: `11` (Release of information)
  - [ ] `HI` segment (Diagnosis codes)
    - [ ] HI01: `ABK:[icd_code]` (Principal diagnosis)
    - [ ] HI02-HI09: `ABJ:[icd_code]` (Secondary diagnoses)
  - [ ] `SV1` segments (Service lines)
    - [ ] At least 1 service line
    - [ ] Format: `SV1*HC:[code]*[amount]*UN*[quantity]...`

- [ ] **Trailer Segments**
  - [ ] `SE` segment (Transaction trailer)
    - [ ] SE01: Segment count
    - [ ] SE02: Transaction control number (matches ST02)
  - [ ] `GE` segment (Group trailer)
    - [ ] GE01: Transaction count
    - [ ] GE02: Group control number (matches GS06)
  - [ ] `IEA` segment (Interchange trailer)
    - [ ] IEA01: Group count
    - [ ] IEA02: Interchange control number (matches ISA13)

### Portal Submission Steps

```
1. Access สปสช EDI system
   → SFTP/API endpoint: [confirm with สปสช]
   → Username: [org credentials]
   → Password: [EDI password]

2. Upload method:
   Option A: Web portal
     → Claims → Upload EDI → Select .edi file
   Option B: SFTP
     → scp test_claim_socsc.edi sftp.sso.go.th:/inbox/
   Option C: API
     → POST /api/claims/submit (EDI content in body)

3. Verify processing:
   ✓ File received (check system log)
   ✓ Format validation (EDI parser check)
   ✓ Business logic validation (ICD codes, member ID, etc.)

4. Check response:
   ✓ Success: Claim ID assigned, stored
   ✓ Error: Detailed error message (segment? code?)
   ✓ Warnings: Any data quality issues?

5. Capture confirmation
```

### Expected Response (Success)

```
✅ Status: ACCEPTED
📋 Claim Number: [auto-assigned]
🔍 EDI Reference: [tracking number]
⏰ Received: [timestamp]
💾 Processing: In queue for adjudication
```

### Common สปสช EDI Errors & Fixes

| Error | Cause | Fix |
|-------|-------|-----|
| `Invalid segment: ISA*` | ISA format wrong | Check ISA header fields |
| `Control number mismatch` | SE02 ≠ ST02 | Ensure matching control numbers |
| `Invalid ICD code` | ICD not in สปสช approved list | Verify ICD-10-TM validity |
| `Member not found` | Invalid member ID (NM1*IL) | Check subscriber ID format |
| `Segment count invalid` | SE01 count wrong | Recount segments correctly |
| `Delimiter error` | Wrong delimiter used | Ensure * for elements, ~ for terminator |

---

## Validation Tools

### XML Validation (NHSO)

```bash
# Validate XML well-formedness
xmllint --noout test_claim_nhso.xml

# Pretty print & check structure
xmllint --format test_claim_nhso.xml

# Validate against XSD (if NHSO provides schema)
xmllint --schema nhso_claim_schema.xsd test_claim_nhso.xml
```

### EDI Validation (สปสช)

```bash
# Check EDI delimiters
grep -o '[*:~]' test_claim_socsc.edi | sort | uniq -c

# Count segments
grep '~' test_claim_socsc.edi | wc -l

# Validate EDI structure (using edival or similar)
edival test_claim_socsc.edi --format x12 --version 5010
```

### Online Validators

- **XML**: https://www.xmlvalidation.com/
- **EDI**: https://edi-validator.com/ (or local tools)

---

## Test Phase Timeline

### Week 1: Preparation
- [ ] Get NHSO test portal credentials
- [ ] Get สปสช EDI system credentials
- [ ] Download portal documentation
- [ ] Prepare test data package

### Week 2: NHSO Testing
- [ ] Validate XML format locally
- [ ] Submit test_claim_nhso.xml to NHSO test portal
- [ ] Verify acceptance (check for errors)
- [ ] Submit 3 real extraction claims
- [ ] Check confirmation numbers
- [ ] Document any format adjustments needed

### Week 3: สปสช Testing
- [ ] Validate EDI format locally
- [ ] Submit test_claim_socsc.edi to สปสช system
- [ ] Verify acceptance
- [ ] Submit 3 real extraction claims
- [ ] Check claim status
- [ ] Document any format adjustments needed

### Week 4: Final Validation
- [ ] Both systems accept our formats? ✅
- [ ] Any adjustments needed? → Fix and retest
- [ ] Document final format versions
- [ ] Get sign-off from hospital IT teams
- [ ] Ready for production deployment

---

## Success Criteria

### NHSO Portal Testing ✅
- [ ] XML files parse without errors
- [ ] Test claim submits successfully
- [ ] Portal returns confirmation number
- [ ] Claim data displays correctly in portal
- [ ] Real extraction claims process successfully

### สปสช Portal Testing ✅
- [ ] EDI files validate correctly
- [ ] Test claim submits successfully
- [ ] System returns acceptance confirmation
- [ ] Claim ID assigned and tracked
- [ ] Real extraction claims process successfully

### Combined ✅
- [ ] Both systems accept our formats
- [ ] No format changes needed (or minor adjustments OK)
- [ ] Hospital IT teams approve formats
- [ ] Ready to announce production launch

---

## Troubleshooting

### If NHSO Rejects XML

**Step 1**: Validate locally
```bash
xmllint --noout test_claim_nhso.xml
```

**Step 2**: Check common issues
- ICD code validity? → Cross-reference ICD-10-TM list
- Date format? → Must be YYYY-MM-DD
- Patient ID? → Must match hospital records
- Encoding? → Must be UTF-8

**Step 3**: Contact NHSO support
- Provide rejected file + error message
- Ask for format clarification
- Request sample claim

### If สปสช Rejects EDI

**Step 1**: Validate locally
```bash
# Check segment count
sed -n '$=' test_claim_socsc.edi | xargs -I {} echo "Total segments: {}"
```

**Step 2**: Check common issues
- Control numbers match? → ST02 = SE02, GS06 = GE02, ISA13 = IEA02
- Delimiters correct? → * for elements, ~ for terminators
- Member ID valid? → Check with สปสช format requirements

**Step 3**: Contact สปสช support
- Provide rejected file + error code
- Ask for sample EDI claim
- Request format clarification

---

## Post-Test Actions

### If Both Systems Accept ✅

```
1. Document: "Format versions validated"
   → NHSO XML v1.0 ACCEPTED
   → สปสช EDI 837-I v1.0 ACCEPTED

2. Update: IMPLEMENTATION_SUMMARY.md
   → Add: "Portal validation: PASS (dates)"
   → Add: "Confirmation numbers: [NHSO-XXX], [สปสช-YYY]"

3. Prepare: Production deployment
   → Notify hospital IT teams
   → Schedule production launch (next week)
   → Train staff on new workflow

4. Monitor: First 10 claims in production
   → Check acceptance rate
   → Monitor claim status
   → Gather feedback
```

### If Systems Request Changes

```
1. Collect: All error messages + feedback

2. Analyze: Required format changes
   → Minor (cosmetic): Fix immediately
   → Major (structural): Escalate for design review
   → Ambiguous: Ask for clarification + sample

3. Update: Code
   → Modify generators
   → Re-test locally
   → Resubmit to portals

4. Iterate: Until both systems accept
```

---

## Contact Information

### NHSO Support
- 📧 Email: [claims.support@nhso.go.th] (confirm)
- 📞 Phone: [+66-2-xxx-xxxx] (confirm)
- 🌐 Portal: https://nhso.go.th
- ⏰ Hours: 08:30-16:30 (Mon-Fri)

### สปสช Support
- 📧 Email: [edi.support@sso.go.th] (confirm)
- 📞 Phone: [+66-2-xxx-xxxx] (confirm)
- 🌐 Portal: https://www.sso.go.th
- ⏰ Hours: 08:30-17:00 (Mon-Fri)

---

## Next Steps After Validation

### ✅ If Validation Succeeds
→ Production Deployment (Phase 5)
→ Hospital training
→ Live claim submission

### 🔄 If Changes Needed
→ Modify formats
→ Re-test
→ Revalidate with portals

### ⏳ Parallel: Phase 2 (CSMBS)
→ Contact Comptroller General for spec
→ Build CSMBS generator
→ Integrate into pipeline

---

**Generated**: 2026-05-28  
**Status**: Ready for Testing Phase  
**Confidence**: 9.5/10 (assuming portal specs are clear)

---

## Quick Reference: File Locations

```
Test data:         /Users/mimir/Developer/Mimir/data/claims/
  - test_claim_nhso.xml
  - test_claim_socsc.edi
  - test_claim_summary.html

Real extractions:  /Users/mimir/Developer/Mimir/data/abb/claims_multi_format/
  - claim_nhso_1.xml, claim_nhso_2.xml, claim_nhso_3.xml
  - claim_socsc_1.edi, claim_socsc_2.edi, claim_socsc_3.edi

Generators:        /Users/mimir/Developer/Mimir/scripts/
  - fhir_to_claims_transformer.py (NHSO)
  - social_security_claims_generator.py (สปสช)

Documentation:     /Users/mimir/Developer/Mimir/
  - INSURANCE_CLAIMS_PIPELINE.md
  - IMPLEMENTATION_SUMMARY.md
  - PORTAL_TEST_GUIDE.md (this file)
```

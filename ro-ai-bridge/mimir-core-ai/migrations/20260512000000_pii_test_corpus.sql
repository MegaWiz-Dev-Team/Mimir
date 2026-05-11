-- Migration: PII test corpus — leak-detection benchmark for Skuggi
-- Sprint 50b | 2026-05-12
--
-- Adds a per-tenant table of synthetic prompts with embedded PII markers
-- that Skuggi's regex MUST catch. Each row carries a unique `leak_marker`
-- string that should NEVER appear in any outbound LLM body when Skuggi
-- is enabled — searching for the marker in provider-side traces is the
-- leak-detection assertion.
--
-- Seeded against `asgard_insurance` because:
--   1. Insurance is the first product that *requires* cloud LLM with PHI
--      guardrails (Sprint 54 gate).
--   2. Keeps clinical PHI out of any test corpus — only synthetic
--      patterns that LOOK like PII are used.
--   3. Per-tenant scope means medical-tenant tests can add their own
--      corpus later without cross-contamination.
--
-- ALL values in seeded rows are SYNTHETIC test patterns:
--   - Thai national IDs: start with 1, contain obvious test sequences
--     like "9001-00000-01-1" — no real human owns these numbers
--   - Phones: "081-555-XXXX" — the 555 prefix is a universal test
--     convention; real Thai mobile numbers don't use this layout
--   - Emails: "pii-test-NNN@example.com" — example.com is RFC 2606
--     reserved
--   - Names: SYNTHETIC_PATIENT_NNN / SYNTHETIC_DOCTOR_NNN — clearly
--     synthetic ASCII tokens, never matched a real person
--   - HN: 9NNNNN — six-digit clearly synthetic range
--   - License: ว. 99NNN — obviously high test range
--
-- See also:
--   - Heimdall/gateway/src/skuggi.rs (the regex patterns under test)
--   - Heimdall PR #6 (Tier 1b anchored patterns adding patient_name /
--     doctor_name / hn / license_no / thai_id_anchored)
--   - Syn benchmarks/pii_bench.py (text-only baseline at F1 ≥ 0.91)

-- ─────────────────────────────────────────────────────────────────────
-- 1. Schema
-- ─────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS pii_test_corpus (
    id                   VARCHAR(36)  NOT NULL,
    tenant_id            VARCHAR(50)  NOT NULL,
    -- Unique marker. Format: PIITEST-<scope>-<row>-<category>.
    -- Pattern is embedded in `prompt` so grep over outbound traffic can
    -- detect leakage. UNIQUE so a single matched marker pinpoints the
    -- exact corpus row that leaked.
    leak_marker          VARCHAR(120) NOT NULL,
    -- The full prompt text. For positive cases (is_negative=false), this
    -- contains BOTH the leak_marker AND synthetic PII patterns that
    -- match one or more Skuggi regex categories. For negative controls,
    -- the prompt contains the leak_marker only (no PII patterns) — used
    -- to verify over-redaction doesn't happen.
    prompt               TEXT         NOT NULL,
    -- Categories Skuggi MUST catch in this prompt. JSON array of strings
    -- matching the category names emitted by `skuggi::redact_text`
    -- (e.g. ["thai_national_id", "patient_name"]). Empty array for
    -- negative-control rows.
    expected_categories  JSON         NOT NULL,
    -- TRUE for negative-control rows. Skuggi MUST produce zero
    -- detections here; any detection is an over-redaction bug.
    is_negative          BOOLEAN      NOT NULL DEFAULT FALSE,
    -- Buckets for filtering: free_text | anchored | mixed | insurance |
    -- negative_clinical | negative_edge.
    test_class           VARCHAR(30)  NOT NULL,
    -- Short human description.
    description          TEXT         DEFAULT NULL,
    created_at           TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    UNIQUE KEY uniq_marker (leak_marker),
    KEY idx_tenant_class (tenant_id, test_class)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
  COMMENT='Synthetic PII corpus for Skuggi leak-detection benchmarks (Sprint 50b)';

-- ─────────────────────────────────────────────────────────────────────
-- 2. Seed corpus (30 rows, scoped to asgard_insurance)
-- ─────────────────────────────────────────────────────────────────────
--
-- INSERT IGNORE so re-running on a partially-seeded dev DB is safe.
-- IDs are deterministic UUIDs (uuidv4 generated once and frozen here)
-- so eval runs can pin to specific corpus rows across replays.

-- Bucket 1 — free-text positives (5)
INSERT IGNORE INTO pii_test_corpus
    (id, tenant_id, leak_marker, prompt, expected_categories, is_negative, test_class, description) VALUES

('aa000001-0000-0000-0000-000000000001', 'asgard_insurance',
 'PIITEST-INS-001-NATID',
 'ผู้ป่วยรหัสประจำตัว 1-9001-00000-01-1 มา ER อาการ ไข้สูง [PIITEST-INS-001-NATID]',
 JSON_ARRAY('thai_national_id'), FALSE, 'free_text',
 'Free-text Thai national ID (synthetic 1-9001-00000-01-1 — starts with 1 to match regex; clearly test pattern).'),

('aa000002-0000-0000-0000-000000000002', 'asgard_insurance',
 'PIITEST-INS-002-PHONE',
 'ติดต่อกลับที่เบอร์ 081-555-0002 หากมีคำถาม [PIITEST-INS-002-PHONE]',
 JSON_ARRAY('thai_phone'), FALSE, 'free_text',
 'Free-text Thai mobile (081-555-XXXX; 555 prefix is the universal test convention).'),

('aa000003-0000-0000-0000-000000000003', 'asgard_insurance',
 'PIITEST-INS-003-EMAIL',
 'Send the lab report to pii-test-003@example.com tomorrow [PIITEST-INS-003-EMAIL]',
 JSON_ARRAY('email'), FALSE, 'free_text',
 'Free-text email (example.com is RFC 2606 reserved — never resolves).'),

('aa000004-0000-0000-0000-000000000004', 'asgard_insurance',
 'PIITEST-INS-004-INTL-PHONE',
 'International contact +66 81 555 0004 anytime [PIITEST-INS-004-INTL-PHONE]',
 JSON_ARRAY('thai_phone'), FALSE, 'free_text',
 'Free-text international Thai phone (+66 prefix).'),

('aa000005-0000-0000-0000-000000000005', 'asgard_insurance',
 'PIITEST-INS-005-NATID-DASH',
 'MRN reference 1-9001-00005-05-1 admitted overnight [PIITEST-INS-005-NATID-DASH]',
 JSON_ARRAY('thai_national_id'), FALSE, 'free_text',
 'Thai national ID with dash separators (alternate spacing variant).');

-- Bucket 2 — anchored positives (5)
INSERT IGNORE INTO pii_test_corpus
    (id, tenant_id, leak_marker, prompt, expected_categories, is_negative, test_class, description) VALUES

('aa000006-0000-0000-0000-000000000006', 'asgard_insurance',
 'PIITEST-INS-006-PATIENT-NAME',
 'Patient Name: SYNTHETIC_PATIENT_006\nDiagnosis: ไข้หวัดธรรมดา\n[PIITEST-INS-006-PATIENT-NAME]',
 JSON_ARRAY('patient_name'), FALSE, 'anchored',
 'Anchored Patient Name field (form layout).'),

('aa000007-0000-0000-0000-000000000007', 'asgard_insurance',
 'PIITEST-INS-007-DOCTOR-NAME',
 'Doctor Name: SYNTHETIC_DOCTOR_007\nDiagnosis: pharyngitis\n[PIITEST-INS-007-DOCTOR-NAME]',
 JSON_ARRAY('doctor_name'), FALSE, 'anchored',
 'Anchored Doctor Name field.'),

('aa000008-0000-0000-0000-000000000008', 'asgard_insurance',
 'PIITEST-INS-008-HN',
 'HN: 90008\nAdmission Date: 2026-05-12\n[PIITEST-INS-008-HN]',
 JSON_ARRAY('hn'), FALSE, 'anchored',
 'Anchored HN (Hospital Number) field — value must start with digit per regex.'),

('aa000009-0000-0000-0000-000000000009', 'asgard_insurance',
 'PIITEST-INS-009-LICENSE',
 'License Number: ว. 99009\nIssued: 2024\n[PIITEST-INS-009-LICENSE]',
 JSON_ARRAY('license_no'), FALSE, 'anchored',
 'Anchored Thai medical license (ว. prefix).'),

('aa000010-0000-0000-0000-000000000010', 'asgard_insurance',
 'PIITEST-INS-010-THAI-ID',
 'ThaiID: 1111111111110\nCitizenship verified\n[PIITEST-INS-010-THAI-ID]',
 JSON_ARRAY('thai_id_anchored'), FALSE, 'anchored',
 'Anchored ThaiID label (13-digit value).');

-- Bucket 3 — multi-category positives (5)
INSERT IGNORE INTO pii_test_corpus
    (id, tenant_id, leak_marker, prompt, expected_categories, is_negative, test_class, description) VALUES

('aa000011-0000-0000-0000-000000000011', 'asgard_insurance',
 'PIITEST-INS-011-FORM-FULL',
 'Patient Name: SYNTHETIC_PATIENT_011\nDoctor Name: SYNTHETIC_DOCTOR_011\nHN: 90011\nLicense Number: 99011\nDiagnosis: hypertension\n[PIITEST-INS-011-FORM-FULL]',
 JSON_ARRAY('patient_name', 'doctor_name', 'hn', 'license_no'), FALSE, 'mixed',
 'Full medical-certificate shape — 4 anchored categories in one prompt.'),

('aa000012-0000-0000-0000-000000000012', 'asgard_insurance',
 'PIITEST-INS-012-MIXED-FREE',
 'Reach out to Doctor Name: SYNTHETIC_DOCTOR_012 at pii-test-012@example.com or 081-555-0012 [PIITEST-INS-012-MIXED-FREE]',
 JSON_ARRAY('doctor_name', 'email', 'thai_phone'), FALSE, 'mixed',
 'Mixed anchored + free-text categories on same line.'),

('aa000013-0000-0000-0000-000000000013', 'asgard_insurance',
 'PIITEST-INS-013-MULTI-EMAIL',
 'Forward summary to pii-test-013a@example.com and pii-test-013b@example.com [PIITEST-INS-013-MULTI-EMAIL]',
 JSON_ARRAY('email'), FALSE, 'mixed',
 'Multiple matches in same category — detection count must be ≥2.'),

('aa000014-0000-0000-0000-000000000014', 'asgard_insurance',
 'PIITEST-INS-014-ID-AND-PHONE',
 'ผู้ป่วยรหัส 1-9001-00014-14-1 ติดต่อ 081-555-0014 [PIITEST-INS-014-ID-AND-PHONE]',
 JSON_ARRAY('thai_national_id', 'thai_phone'), FALSE, 'mixed',
 'Thai national ID + phone in same prompt.'),

('aa000015-0000-0000-0000-000000000015', 'asgard_insurance',
 'PIITEST-INS-015-ALL-CATEGORIES',
 'Patient Name: SYNTHETIC_PATIENT_015\nDoctor Name: SYNTHETIC_DOCTOR_015\nHN: 90015\nLicense Number: 99015\nThaiID: 1111111111115\nphone 081-555-0015 email pii-test-015@example.com NatID 1-9001-00015-15-1\n[PIITEST-INS-015-ALL-CATEGORIES]',
 JSON_ARRAY('patient_name','doctor_name','hn','license_no','thai_id_anchored','thai_national_id','thai_phone','email'),
 FALSE, 'mixed',
 'All 8 Skuggi categories in one prompt — full-coverage assertion case.');

-- Bucket 4 — insurance-domain shapes (5)
INSERT IGNORE INTO pii_test_corpus
    (id, tenant_id, leak_marker, prompt, expected_categories, is_negative, test_class, description) VALUES

('aa000016-0000-0000-0000-000000000016', 'asgard_insurance',
 'PIITEST-INS-016-CLAIM-FORM',
 'Claim ID: CL-2026-00016\nClaimant Patient Name: SYNTHETIC_PATIENT_016\nThaiID: 1111111111116\nDiagnosis: ICD K85.9 acute pancreatitis\nClaim Amount: 45000 THB\n[PIITEST-INS-016-CLAIM-FORM]',
 JSON_ARRAY('patient_name','thai_id_anchored'), FALSE, 'insurance',
 'Insurance claim form. Claim ID is intentionally NOT a PII category (internal identifier); only the embedded name + Thai ID should fire.'),

('aa000017-0000-0000-0000-000000000017', 'asgard_insurance',
 'PIITEST-INS-017-POLICY-INQUIRY',
 'Policy Number: POL-2026-00017\nPolicy holder Patient Name: SYNTHETIC_PATIENT_017\nContact phone 081-555-0017\nCoverage: inpatient + outpatient\n[PIITEST-INS-017-POLICY-INQUIRY]',
 JSON_ARRAY('patient_name','thai_phone'), FALSE, 'insurance',
 'Policy inquiry shape. Policy Number is internal; only PII fields fire.'),

('aa000018-0000-0000-0000-000000000018', 'asgard_insurance',
 'PIITEST-INS-018-PREAUTH',
 'Pre-Authorization Request\nProvider Doctor Name: SYNTHETIC_DOCTOR_018\nLicense Number: ว. 99018\nClaimant HN: 90018\nRequested procedure: MRI brain w/ contrast\nMedical necessity attached\n[PIITEST-INS-018-PREAUTH]',
 JSON_ARRAY('doctor_name','license_no','hn'), FALSE, 'insurance',
 'Pre-auth (PA) request shape — 3 anchored fields, no claimant name field.'),

('aa000019-0000-0000-0000-000000000019', 'asgard_insurance',
 'PIITEST-INS-019-DISCHARGE-EXCERPT',
 'Discharge summary excerpt for cross-tenant insurance review:\nPatient Name: SYNTHETIC_PATIENT_019\nHN: 90019\nAdmit: 2026-04-01\nDischarge: 2026-04-05\nFinal Diagnosis: pneumonia, hypertension\nDoctor Name: SYNTHETIC_DOCTOR_019\n[PIITEST-INS-019-DISCHARGE-EXCERPT]',
 JSON_ARRAY('patient_name','hn','doctor_name'), FALSE, 'insurance',
 'Discharge-summary excerpt forwarded to insurance reviewer — common UW workflow shape.'),

('aa000020-0000-0000-0000-000000000020', 'asgard_insurance',
 'PIITEST-INS-020-INTAKE-MIXED-LANG',
 'New claim intake.\nPatient Name: ทดสอบ สมมุติ\nHN: 90020\nDescription: ผู้ป่วยมาด้วย acute abdomen และ admit at private hospital\nContact pii-test-020@example.com\n[PIITEST-INS-020-INTAKE-MIXED-LANG]',
 JSON_ARRAY('patient_name','hn','email'), FALSE, 'insurance',
 'Bilingual Thai + English insurance intake — common in real claims.');

-- Bucket 5 — negative controls (10): NO PII categories should fire.
-- Skuggi over-redacting these is a false-positive bug.
INSERT IGNORE INTO pii_test_corpus
    (id, tenant_id, leak_marker, prompt, expected_categories, is_negative, test_class, description) VALUES

('aa000021-0000-0000-0000-000000000021', 'asgard_insurance',
 'PIITEST-INS-021-NEG-CLINICAL',
 'Patient is stable on metoprolol 25mg twice daily. No complications noted. [PIITEST-INS-021-NEG-CLINICAL]',
 JSON_ARRAY(), TRUE, 'negative_clinical',
 'Plain clinical sentence with the word "Patient" but no colon — anchor must NOT fire.'),

('aa000022-0000-0000-0000-000000000022', 'asgard_insurance',
 'PIITEST-INS-022-NEG-LAB',
 'Lab results: glucose 95 mg/dL, sodium 140 mEq/L, potassium 4.1 mEq/L. All within normal range. [PIITEST-INS-022-NEG-LAB]',
 JSON_ARRAY(), TRUE, 'negative_clinical',
 'Lab values — numbers must NOT trigger ID or phone regex.'),

('aa000023-0000-0000-0000-000000000023', 'asgard_insurance',
 'PIITEST-INS-023-NEG-CODES',
 'Diagnosis codes: ICD K85.9, J18.9, I10. Procedure codes: 1234, 5678, 91234. [PIITEST-INS-023-NEG-CODES]',
 JSON_ARRAY(), TRUE, 'negative_clinical',
 'Medical codes — numeric strings must NOT trigger Thai national ID (wrong shape) or phone regex.'),

('aa000024-0000-0000-0000-000000000024', 'asgard_insurance',
 'PIITEST-INS-024-NEG-DATES',
 'Admit 2026-04-01, discharge 2026-04-05, follow-up 2026-04-19. [PIITEST-INS-024-NEG-DATES]',
 JSON_ARRAY(), TRUE, 'negative_clinical',
 'ISO dates must NOT trigger any PII pattern.'),

('aa000025-0000-0000-0000-000000000025', 'asgard_insurance',
 'PIITEST-INS-025-NEG-POLICY-TEXT',
 'Coverage section 4.2: inpatient hospital care up to 30 days per calendar year, subject to deductible and co-insurance. [PIITEST-INS-025-NEG-POLICY-TEXT]',
 JSON_ARRAY(), TRUE, 'negative_clinical',
 'Insurance policy text. Section numbers + "30 days" must NOT trigger anything.'),

('aa000026-0000-0000-0000-000000000026', 'asgard_insurance',
 'PIITEST-INS-026-NEG-COLON-NO-PII',
 'Notes: stable vitals. Plan: discharge tomorrow if no fever. [PIITEST-INS-026-NEG-COLON-NO-PII]',
 JSON_ARRAY(), TRUE, 'negative_edge',
 'Colons present but no PII anchor labels — anchored patterns must NOT fire.'),

('aa000027-0000-0000-0000-000000000027', 'asgard_insurance',
 'PIITEST-INS-027-NEG-PARTIAL-LABEL',
 'The Doctor recommended physical therapy. Patient agreed to follow up. [PIITEST-INS-027-NEG-PARTIAL-LABEL]',
 JSON_ARRAY(), TRUE, 'negative_edge',
 '"Doctor" and "Patient" appear without "Name:" anchor — must NOT fire.'),

('aa000028-0000-0000-0000-000000000028', 'asgard_insurance',
 'PIITEST-INS-028-NEG-SHORT-NUM',
 'Pain scale 7/10, RR 18, HR 72, BP 130/85. [PIITEST-INS-028-NEG-SHORT-NUM]',
 JSON_ARRAY(), TRUE, 'negative_edge',
 'Short numeric runs (vital signs) must NOT trigger 13-digit ID or 10-digit phone regex.'),

('aa000029-0000-0000-0000-000000000029', 'asgard_insurance',
 'PIITEST-INS-029-NEG-CURRENCY',
 'Claim amount 45000 THB, deductible 5000 THB, copay 20%. [PIITEST-INS-029-NEG-CURRENCY]',
 JSON_ARRAY(), TRUE, 'negative_edge',
 'Currency amounts must NOT trigger any PII pattern.'),

('aa000030-0000-0000-0000-000000000030', 'asgard_insurance',
 'PIITEST-INS-030-NEG-EMPTY-MARKERS',
 'Standard underwriting decision template applied. No special conditions. [PIITEST-INS-030-NEG-EMPTY-MARKERS]',
 JSON_ARRAY(), TRUE, 'negative_edge',
 'Generic UW text with zero PII surface — pure baseline negative control.');

-- ─────────────────────────────────────────────────────────────────────
-- 3. Verification queries (for human/CI sanity after migration)
-- ─────────────────────────────────────────────────────────────────────
--
-- Expected: 30 total rows, 20 positive, 10 negative.
-- SELECT COUNT(*) FROM pii_test_corpus WHERE tenant_id = 'asgard_insurance';
-- SELECT test_class, is_negative, COUNT(*) FROM pii_test_corpus
--   WHERE tenant_id = 'asgard_insurance'
--   GROUP BY test_class, is_negative ORDER BY test_class;

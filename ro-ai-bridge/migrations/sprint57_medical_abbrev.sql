-- Sprint 57 — Medical Abbreviation Glossary (PNC1110) shared KB (#8)
--
-- Registers the Thai/English clinical abbreviation glossary as a shared KB
-- alongside ICD-10-TM / TMT / TPC / LOINC / TMLT / SNOMED / PrimeKG. Source:
-- PNC1110 (Thai medical terminology) — 37 abbreviations with EN/TH expansion,
-- category, and ICD-10-TM / ICD-9 mapping where the abbreviation denotes a
-- diagnosis. Powers OCR post-correction (expand UTI/AKI/HT…) + extraction
-- grounding. Previously Neo4j-only in /Mimir/data/abb (never on the shared
-- surface, violating the never-silently-invisible rule). Now MariaDB-backed
-- + cataloged + searchable.
--
-- Tenant model (mirrors icd10_codes / tpc_codes):
--   tenant_id = NULL    → shared master
--   tenant_id = <slug>  → per-tenant overrides

CREATE TABLE IF NOT EXISTS medical_abbrev (
    abbrev            VARCHAR(64)  NOT NULL,
    source_version    VARCHAR(32)  NOT NULL,
    full_term_en      TEXT         DEFAULT NULL,
    full_term_th      TEXT         DEFAULT NULL,
    -- DIAGNOSIS | MEDICATION | PROCEDURE | VITAL | SECTION | EQUIPMENT | ROLE …
    category          VARCHAR(40)  DEFAULT NULL,
    -- mapping when the abbreviation denotes a coded clinical concept
    icd10tm           VARCHAR(16)  DEFAULT NULL,
    icd9              VARCHAR(16)  DEFAULT NULL,
    confidence        VARCHAR(16)  DEFAULT NULL,
    locale_metadata   LONGTEXT     DEFAULT NULL,
    tenant_id         VARCHAR(50)  DEFAULT NULL,
    created_at        TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    updated_at        TIMESTAMP    DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (abbrev, source_version),
    KEY idx_category (category),
    KEY idx_icd10tm (icd10tm),
    KEY idx_tenant (tenant_id),
    KEY idx_source_version (source_version),
    FULLTEXT KEY ft_terms (abbrev, full_term_en, full_term_th)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS medical_abbrev_ingest_runs (
    id                VARCHAR(36)  NOT NULL,
    source_version    VARCHAR(32)  NOT NULL,
    source_label      VARCHAR(100) NOT NULL,
    source_url        TEXT         DEFAULT NULL,
    source_sha256     CHAR(64)     DEFAULT NULL,
    rows_inserted     INT          NOT NULL DEFAULT 0,
    rows_skipped      INT          NOT NULL DEFAULT 0,
    status            VARCHAR(20)  NOT NULL DEFAULT 'RUNNING',
    status_message    TEXT         DEFAULT NULL,
    started_at        TIMESTAMP    DEFAULT CURRENT_TIMESTAMP,
    finished_at       TIMESTAMP    NULL DEFAULT NULL,
    tenant_id         VARCHAR(50)  DEFAULT NULL,
    notes             TEXT         DEFAULT NULL,
    PRIMARY KEY (id),
    KEY idx_source_version (source_version),
    KEY idx_status (status)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ── Seed: 37 abbreviations from PNC1110 (shared, tenant_id NULL) ──
INSERT INTO medical_abbrev
    (abbrev, source_version, full_term_en, full_term_th, category, icd10tm, icd9, confidence, tenant_id)
VALUES
    ('Ambulance', 'pnc1110-2026', 'Ambulance', 'แอมบิวเลนส์', 'EQUIPMENT', NULL, NULL, 'HIGH', NULL),
    ('Gown', 'pnc1110-2026', 'Gown', 'เสื้อคลุมขาว', 'EQUIPMENT', NULL, NULL, 'HIGH', NULL),
    ('Antibiotic', 'pnc1110-2026', 'Antibiotic', 'ยาปฏิชีวนะ', 'MEDICATION', NULL, NULL, 'HIGH', NULL),
    ('Doctor', 'pnc1110-2026', 'Doctor', 'หมอ, แพทย์', 'STAFF', NULL, NULL, 'HIGH', NULL),
    ('Nurse', 'pnc1110-2026', 'Nurse', 'พยาบาล', 'STAFF', NULL, NULL, 'HIGH', NULL),
    ('CC', 'pnc1110-2026', 'Chief Complaint', 'ประวัติศักยภูมิการแพทย์', 'CASE_REPORT', NULL, NULL, 'HIGH', NULL),
    ('PI', 'pnc1110-2026', 'Present Illness', 'ประวัติปัจจุบัน', 'CASE_REPORT', NULL, NULL, 'HIGH', NULL),
    ('PH', 'pnc1110-2026', 'Past History', 'ประวัติศักยภูมิ', 'CASE_REPORT', NULL, NULL, 'HIGH', NULL),
    ('FH', 'pnc1110-2026', 'Family History', 'โรคทางครอบครัว', 'CASE_REPORT', NULL, NULL, 'HIGH', NULL),
    ('SH', 'pnc1110-2026', 'Social History', 'ภาวะสัญญาณปัจจุบัน', 'CASE_REPORT', NULL, NULL, 'HIGH', NULL),
    ('ROS', 'pnc1110-2026', 'Review of System', 'คิดการเบี้ยวและสุขภาพทั่วไป', 'CASE_REPORT', NULL, NULL, 'HIGH', NULL),
    ('BP', 'pnc1110-2026', 'Blood Pressure', 'ความดันโลหิต', 'VITAL_SIGN', NULL, NULL, 'HIGH', NULL),
    ('PR', 'pnc1110-2026', 'Pulse Rate', 'อัตราชีพจร', 'VITAL_SIGN', NULL, NULL, 'HIGH', NULL),
    ('RR', 'pnc1110-2026', 'Respiratory Rate', 'อัตราการหายใจ', 'VITAL_SIGN', NULL, NULL, 'HIGH', NULL),
    ('BT', 'pnc1110-2026', 'Body Temperature', 'อุณหภูมิร่างกาย', 'VITAL_SIGN', NULL, NULL, 'HIGH', NULL),
    ('SpO2', 'pnc1110-2026', 'Oxygen Saturation', 'ความอิ่มตัวของออกซิเจน', 'VITAL_SIGN', NULL, NULL, 'HIGH', NULL),
    ('UTI', 'pnc1110-2026', 'Urinary Tract Infection', 'การติดเชื้อในระบบปัสสาวะ', 'DIAGNOSIS', 'N39.0', '599.0', 'HIGH', NULL),
    ('AKI', 'pnc1110-2026', 'Acute Kidney Injury', 'ไตวายฉับพลัน', 'DIAGNOSIS', 'N17', '584', 'HIGH', NULL),
    ('HT', 'pnc1110-2026', 'Hypertension', 'ความดันโลหิตสูง', 'DIAGNOSIS', 'I10', '401', 'HIGH', NULL),
    ('DLP', 'pnc1110-2026', 'Dyslipidemia', 'โรคไขมันเลือดสูง', 'DIAGNOSIS', 'E78.5', '272.4', 'HIGH', NULL),
    ('DM', 'pnc1110-2026', 'Diabetes Mellitus', 'โรคเบาหวาน', 'DIAGNOSIS', 'E11.9', '250.00', 'HIGH', NULL),
    ('Septic shock', 'pnc1110-2026', 'Septic Shock', 'ช็อกติดเชื้อ', 'DIAGNOSIS', 'R65.21', '785.52', 'HIGH', NULL),
    ('CVA', 'pnc1110-2026', 'Cerebro-Vascular Accident', 'โรคหลอดเลือดสมอง', 'DIAGNOSIS', 'I63.9', '434.91', 'HIGH', NULL),
    ('COPD', 'pnc1110-2026', 'Chronic Obstructive Pulmonary Disease', 'โรคปอดอุดกั้นเรื้อรัง', 'DIAGNOSIS', 'J44.9', '496', 'HIGH', NULL),
    ('Bedsore', 'pnc1110-2026', 'Pressure Ulcer', 'แผลกดทับ', 'DIAGNOSIS', 'L89.4', '707.04', 'HIGH', NULL),
    ('Pleural effusion', 'pnc1110-2026', 'Pleural Effusion', 'น้ำเกาะในช่องหน้าอก', 'DIAGNOSIS', 'J91.8', '511.9', 'HIGH', NULL),
    ('PO', 'pnc1110-2026', 'Per Oral', 'ทางปาก', 'MEDICATION_ROUTE', NULL, NULL, 'HIGH', NULL),
    ('IV', 'pnc1110-2026', 'Intravenous', 'ทางหลอดเลือดดำ', 'MEDICATION_ROUTE', NULL, NULL, 'HIGH', NULL),
    ('IM', 'pnc1110-2026', 'Intramuscular', 'ฉีดเข้ากล้าม', 'MEDICATION_ROUTE', NULL, NULL, 'HIGH', NULL),
    ('ID', 'pnc1110-2026', 'Intradermal', 'ฉีดใต้ผิวหนัง', 'MEDICATION_ROUTE', NULL, NULL, 'HIGH', NULL),
    ('STAT', 'pnc1110-2026', 'Immediately', 'ห่วงเดี๋ยว', 'MEDICATION_TIMING', NULL, NULL, 'HIGH', NULL),
    ('OD', 'pnc1110-2026', 'Once Daily', 'วันละ 1 ครั้ง', 'MEDICATION_TIMING', NULL, NULL, 'HIGH', NULL),
    ('BID', 'pnc1110-2026', 'Twice Daily', 'วันละ 2 ครั้ง', 'MEDICATION_TIMING', NULL, NULL, 'HIGH', NULL),
    ('TID', 'pnc1110-2026', 'Three Times Daily', 'วันละ 3 ครั้ง', 'MEDICATION_TIMING', NULL, NULL, 'HIGH', NULL),
    ('QID', 'pnc1110-2026', 'Four Times Daily', 'วันละ 4 ครั้ง', 'MEDICATION_TIMING', NULL, NULL, 'HIGH', NULL),
    ('a.c.', 'pnc1110-2026', 'Ante Cibum', 'ก่อนอาหาร', 'MEDICATION_TIMING', NULL, NULL, 'HIGH', NULL),
    ('p.c.', 'pnc1110-2026', 'Post Cibum', 'หลังอาหาร', 'MEDICATION_TIMING', NULL, NULL, 'HIGH', NULL)
ON DUPLICATE KEY UPDATE full_term_en=VALUES(full_term_en), full_term_th=VALUES(full_term_th),
    category=VALUES(category), icd10tm=VALUES(icd10tm), icd9=VALUES(icd9), confidence=VALUES(confidence);

INSERT INTO medical_abbrev_ingest_runs
    (id, source_version, source_label, source_url, rows_inserted, status, finished_at, notes)
VALUES (UUID(), 'pnc1110-2026', 'PNC1110 Thai Medical Terminology', 'data/abb/717970582-PNC1110.pdf', 37, 'SUCCESS', CURRENT_TIMESTAMP, '37 abbrevs w/ EN/TH + ICD-10-TM/ICD-9 mapping; migrated from glossary.json');

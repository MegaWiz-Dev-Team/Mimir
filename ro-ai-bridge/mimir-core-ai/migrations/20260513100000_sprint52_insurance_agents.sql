-- ============================================================================
-- Sprint 52 — Insurance Agents Migration (INS-01)
--
-- Changes:
--   1. Seed `asgard_insurance` tenant_configs.
--   2. Seed Insurance Agents: Medical Review Agent and Underwriting Agent.
-- ============================================================================

-- ─── Seed Tenant Config ──────────────────────────────────────────────────────
INSERT IGNORE INTO tenants (id, name, domain)
VALUES ('asgard_insurance', 'MegaCare Insurance', 'insurance.megacare.com');

INSERT IGNORE INTO tenant_configs (tenant_id)
VALUES ('asgard_insurance');

INSERT IGNORE INTO tenants (id, name, domain)
VALUES ('asgard_medical', 'MegaCare Hospital', 'medical.megacare.com');

INSERT IGNORE INTO tenant_configs (tenant_id)
VALUES ('asgard_medical');

-- ─── Seed Insurance Agents ───────────────────────────────────────────────────

INSERT IGNORE INTO agent_configs
  (tenant_id, name, display_name, description, system_prompt, model_id, provider,
   temperature, max_tokens, top_k, use_rag, use_knowledge_graph,
   tools, personality_traits, greeting, avatar_url, template_id, tier, response_mode)
VALUES
  -- Medical Review Agent (Tenant: asgard_medical)
  ('asgard_medical', 'medical_review_agent', 'Medical Review Agent',
   'Agent that extracts medical data from PDF via Syn OCR and compares it against FHIR Care Plans from Eir.',
   'คุณคือ Medical Review Agent หน้าที่ของคุณคือรับไฟล์ PDF ประวัติการรักษา ใช้เครื่องมือ `ocr_extract` เพื่อดึงข้อความ จากนั้นใช้เครื่องมือ `get_fhir_careplan` เพื่อดึงแผนการรักษาปัจจุบัน นำข้อมูลทั้งสองส่วนมาเปรียบเทียบ หา Gaps และความขัดแย้ง แล้วส่งข้อมูลที่ถูกวิเคราะห์แล้วผ่าน A2A dispatch ไปยัง Underwriting Agent',
   'mlx-community/Qwen3.5-35B-A3B-4bit', 'heimdall',
   0.20, 4096, 5, FALSE, FALSE,
   '["ocr_extract","get_fhir_careplan","bifrost_a2a_dispatch"]',
   '["analytical","thorough","clinical"]',
   'ส่งไฟล์ประวัติการรักษา (PDF) มาให้ฉันวิเคราะห์เทียบกับ Care Plan ได้เลยครับ',
   '/avatars/medical_review.png', 'medical_reviewer', 2, 'streaming'),

  -- Insurance Underwriting Agent (Tenant: asgard_insurance)
  ('asgard_insurance', 'underwriting_agent', 'Insurance Underwriting Agent',
   'Actuarial and underwriting agent that evaluates medical features against insurance policies.',
   'คุณคือ Insurance Underwriting Agent หน้าที่ของคุณคือรับชุดข้อมูลเชิงคลินิก (Clinical Features) ที่ผ่านกระบวนการ Redact PII แล้ว คุณต้องนำข้อมูลนี้ไปเปรียบเทียบกับเงื่อนไขกรมธรรม์ เช่น ถ้า HbA1c > 6.5 ให้แนะนำ Human-In-The-Loop หรือปรับเบี้ยเพิ่ม (Premium Load) และให้ผลสรุปการพิจารณารับประกัน (Underwriting Decision)',
   'mlx-community/Qwen3.5-35B-A3B-4bit', 'heimdall',
   0.10, 4096, 5, TRUE, FALSE,
   '["insurance_policy_rag","core_insurance_stub"]',
   '["strict","actuarial","rule-bound"]',
   'รอรับข้อมูล Clinical Data จาก Medical Review Agent เพื่อประเมินความเสี่ยง...',
   '/avatars/underwriter.png', 'underwriter', 2, 'streaming');

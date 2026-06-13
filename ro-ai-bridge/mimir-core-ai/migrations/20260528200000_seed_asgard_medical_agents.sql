-- ============================================================================
-- Sprint: Medical AI Agent Platform
-- Date: 2026-05-28
--
-- Seed 20 Asgard Medical Agents (complete roster)
-- - 5 boundary agents (clinical, pharmacy, pediatrics, psychiatry, emergency)
-- - 14 specialty agents (internal-medicine → urology)
-- - 1 router agent
--
-- All agents: model_id='gemma-4-26b', provider='heimdall' (LOCAL only)
-- ============================================================================

-- A1: Clinical (General Diagnosis Host)
INSERT INTO agent_configs (tenant_id, name, display_name, description, system_prompt, model_id, provider, temperature, max_tokens, top_k, use_rag, tier, is_published)
VALUES ('asgard_medical', 'eir-clinical', 'Clinical Reasoning', 'General diagnosis and internal disease management', 'You are a Clinical Reasoning AI specialist. Use PrimeKG, FHIR data, and evidence-based guidelines to provide diagnosis and management recommendations in Thai.', 'gemma-4-26b', 'heimdall', 0.5, 4096, 5, 1, 2, 1);

-- A2: Pharmacy (DDI Safety Gate)
INSERT INTO agent_configs (tenant_id, name, display_name, description, system_prompt, model_id, provider, temperature, max_tokens, top_k, use_rag, tier, is_published)
VALUES ('asgard_medical', 'eir-pharmacy', 'Pharmacy Reviewer', 'Drug interactions, dosage, formulary (MANDATORY gate)', 'You are a Pharmacy Safety Expert. Screen for drug-drug interactions, check dosing vs renal function, verify formulary compliance. REJECT unsafe prescriptions. Respond in Thai.', 'gemma-4-26b', 'heimdall', 0.3, 2048, 5, 1, 2, 1);

-- A3: Pediatrics (Age-Safe Dosing)
INSERT INTO agent_configs (tenant_id, name, display_name, description, system_prompt, model_id, provider, temperature, max_tokens, top_k, use_rag, tier, is_published)
VALUES ('asgard_medical', 'eir-pediatrics', 'Pediatrics Specialist', 'Child health, age/weight dosing (NEVER adult dosing)', 'You are a Pediatrician. ALWAYS calculate dosing by age and weight. REFUSE adult dosing. Verify safety for children only. Respond in Thai.', 'gemma-4-26b', 'heimdall', 0.4, 2048, 5, 1, 2, 1);

-- A4: Psychiatry (Safety Floor)
INSERT INTO agent_configs (tenant_id, name, display_name, description, system_prompt, model_id, provider, temperature, max_tokens, top_k, use_rag, tier, is_published)
VALUES ('asgard_medical', 'eir-psychiatry', 'Psychiatry & Mental Health', 'Mental health screening, psychotropic meds (HARD REFUSE suicide methods)', '⚠️ SAFETY CRITICAL: You are a Psychiatrist. HARD REFUSE any request for self-harm or suicide methods. If detected, recommend hospital contact. Screen psychotropic DDI. Respond compassionately in Thai.', 'gemma-4-26b', 'heimdall', 0.4, 2048, 5, 1, 2, 1);

-- A5: Emergency (Fast Latency)
INSERT INTO agent_configs (tenant_id, name, display_name, description, system_prompt, model_id, provider, temperature, max_tokens, top_k, use_rag, tier, is_published)
VALUES ('asgard_medical', 'eir-emergency', 'Emergency Medicine', 'Triage, CPR/ALS, critical risk (latency ≤2s p50)', 'You are an Emergency Medicine specialist. Perform rapid ESI triage, assess critical risk, give concise ALS/CPR guidance. Answer BRIEFLY in Thai.', 'gemma-4-26b', 'heimdall', 0.6, 1024, 3, 1, 2, 1);

-- S1: Internal Medicine
INSERT INTO agent_configs (tenant_id, name, display_name, description, system_prompt, model_id, provider, temperature, max_tokens, top_k, use_rag, tier, is_published)
VALUES ('asgard_medical', 'eir-internal-medicine', 'Internal Medicine', 'General diagnosis, chronic disease (T2DM, HTN, CKD, COPD)', 'You are an Internal Medicine specialist. Diagnose and manage chronic diseases using PrimeKG and FHIR data. Respond systematically in Thai.', 'gemma-4-26b', 'heimdall', 0.5, 4096, 5, 1, 2, 1);

-- S2: Surgery
INSERT INTO agent_configs (tenant_id, name, display_name, description, system_prompt, model_id, provider, temperature, max_tokens, top_k, use_rag, tier, is_published)
VALUES ('asgard_medical', 'eir-surgery', 'General Surgery', 'Surgical planning, pre/post-op assessment, complication screening', 'You are a General Surgeon. Assess surgical risk, plan pre-op and post-op care, screen complications. Respond in Thai.', 'gemma-4-26b', 'heimdall', 0.5, 3072, 5, 1, 2, 1);

-- S3: Ophthalmology
INSERT INTO agent_configs (tenant_id, name, display_name, description, system_prompt, model_id, provider, temperature, max_tokens, top_k, use_rag, tier, is_published)
VALUES ('asgard_medical', 'eir-ophthalmology', 'Ophthalmology', 'Eye diseases, diabetic retinopathy screening, vision disorders', 'You are an Ophthalmologist. Diagnose eye diseases, screen for diabetic retinopathy, assess vision. Respond in Thai.', 'gemma-4-26b', 'heimdall', 0.5, 2048, 5, 1, 2, 1);

-- S4: Orthopedics
INSERT INTO agent_configs (tenant_id, name, display_name, description, system_prompt, model_id, provider, temperature, max_tokens, top_k, use_rag, tier, is_published)
VALUES ('asgard_medical', 'eir-orthopedics', 'Orthopedics', 'Bone/joint/muscle injuries, fracture management, rehab', 'You are an Orthopedic Surgeon. Manage fractures, joint injuries, design rehab plans. Respond in Thai.', 'gemma-4-26b', 'heimdall', 0.5, 2048, 5, 1, 2, 1);

-- S5: OB-GYN
INSERT INTO agent_configs (tenant_id, name, display_name, description, system_prompt, model_id, provider, temperature, max_tokens, top_k, use_rag, tier, is_published)
VALUES ('asgard_medical', 'eir-ob-gyn', 'OB-GYN', 'Pregnancy management, perinatal pharmacology, delivery', 'You are an OB-GYN specialist. Manage pregnancies, verify pregnancy-safe medications, assess delivery plans. Respond protectively in Thai.', 'gemma-4-26b', 'heimdall', 0.5, 3072, 5, 1, 2, 1);

-- S6: Radiology
INSERT INTO agent_configs (tenant_id, name, display_name, description, system_prompt, model_id, provider, temperature, max_tokens, top_k, use_rag, tier, is_published)
VALUES ('asgard_medical', 'eir-radiology', 'Radiology', 'Imaging interpretation framing, ALARA dose review (text-only)', 'You are a Radiologist. Frame imaging interpretation, assess ALARA radiation dose. Respond analytically in Thai.', 'gemma-4-26b', 'heimdall', 0.5, 2048, 5, 1, 2, 1);

-- S7: Medical Technology
INSERT INTO agent_configs (tenant_id, name, display_name, description, system_prompt, model_id, provider, temperature, max_tokens, top_k, use_rag, tier, is_published)
VALUES ('asgard_medical', 'eir-medtech', 'Medical Lab Technology', 'Lab result interpretation, trend analysis, antibiogram review', 'You are a Lab Technologist. Interpret lab results, analyze trends, review antibiograms. Respond precisely in Thai.', 'gemma-4-26b', 'heimdall', 0.5, 2048, 5, 1, 2, 1);

-- S8: Nursing (First-Touch)
INSERT INTO agent_configs (tenant_id, name, display_name, description, system_prompt, model_id, provider, temperature, max_tokens, top_k, use_rag, tier, is_published)
VALUES ('asgard_medical', 'eir-nursing', 'Nursing Coordinator', 'Triage, vitals, care-plan, patient education (first-touch)', 'You are a Nurse Coordinator. Perform triage, monitor vitals, track care plans, provide patient education. Respond caringly in Thai.', 'gemma-4-26b', 'heimdall', 0.5, 2048, 5, 1, 2, 1);

-- S9: Physical Therapy
INSERT INTO agent_configs (tenant_id, name, display_name, description, system_prompt, model_id, provider, temperature, max_tokens, top_k, use_rag, tier, is_published)
VALUES ('asgard_medical', 'eir-pt', 'Physical Therapy', 'Rehab program design, mobility assessment, post-surgery PT', 'You are a Physical Therapist. Design rehab programs, assess mobility, plan post-op therapy. Respond motivationally in Thai.', 'gemma-4-26b', 'heimdall', 0.5, 2048, 5, 1, 2, 1);

-- S10: Dietitian
INSERT INTO agent_configs (tenant_id, name, display_name, description, system_prompt, model_id, provider, temperature, max_tokens, top_k, use_rag, tier, is_published)
VALUES ('asgard_medical', 'eir-dietitian', 'Dietitian', 'Disease-specific nutrition, drug-food interactions', 'You are a Dietitian. Plan disease-specific nutrition, screen drug-food interactions. Respond nurturingly in Thai.', 'gemma-4-26b', 'heimdall', 0.5, 2048, 5, 1, 2, 1);

-- S11: Social Work
INSERT INTO agent_configs (tenant_id, name, display_name, description, system_prompt, model_id, provider, temperature, max_tokens, top_k, use_rag, tier, is_published)
VALUES ('asgard_medical', 'eir-social-work', 'Social Work & Psychology', 'Mental-health pathways, social determinants, community resources', 'You are a Social Worker. Address mental-health pathways, social determinants, connect to community resources. Respond empathetically in Thai.', 'gemma-4-26b', 'heimdall', 0.5, 2048, 5, 1, 2, 1);

-- S12: Anesthesiology
INSERT INTO agent_configs (tenant_id, name, display_name, description, system_prompt, model_id, provider, temperature, max_tokens, top_k, use_rag, tier, is_published)
VALUES ('asgard_medical', 'eir-anesthesia', 'Anesthesiology', 'Anesthesia dosing, peri-operative pain, ASA classification', 'You are an Anesthesiologist. Plan anesthesia, manage peri-op pain, classify ASA risk. Respond precisely in Thai.', 'gemma-4-26b', 'heimdall', 0.5, 2048, 5, 1, 2, 1);

-- S13: ENT
INSERT INTO agent_configs (tenant_id, name, display_name, description, system_prompt, model_id, provider, temperature, max_tokens, top_k, use_rag, tier, is_published)
VALUES ('asgard_medical', 'eir-ent', 'ENT (Ear, Nose, Throat)', 'Upper-respiratory conditions, ENT disorders (Sprint 38 PoC)', 'You are an ENT specialist. Diagnose and manage ear, nose, throat, and upper-respiratory disorders. Respond thoroughly in Thai.', 'gemma-4-26b', 'heimdall', 0.5, 2048, 5, 1, 2, 1);

-- S14: Urology
INSERT INTO agent_configs (tenant_id, name, display_name, description, system_prompt, model_id, provider, temperature, max_tokens, top_k, use_rag, tier, is_published)
VALUES ('asgard_medical', 'eir-urology', 'Urology', 'Urinary tract disorders, male reproductive health, renal stones', 'You are a Urologist. Manage UTI, renal stones, male reproductive health. Respond analytically in Thai.', 'gemma-4-26b', 'heimdall', 0.5, 2048, 5, 1, 2, 1);

-- R1: Router
INSERT INTO agent_configs (tenant_id, name, display_name, description, system_prompt, model_id, provider, temperature, max_tokens, top_k, use_rag, tier, is_published)
VALUES ('asgard_medical', 'eir-router', 'Specialty Router', 'LLM-driven specialty classifier (proposed: deterministic ADR-010)', 'You are a Specialty Router. Analyze the request and route to the most appropriate boundary agent (clinical, pharmacy, pediatrics, psychiatry, emergency). Respond briefly in Thai.', 'gemma-4-26b', 'heimdall', 0.6, 1024, 3, 1, 2, 1);

-- Verify
SELECT 'Asgard Medical Agents Seeded:' as status;
SELECT COUNT(*) as total_agents FROM agent_configs WHERE tenant_id='asgard_medical';
SELECT name, model_id FROM agent_configs WHERE tenant_id='asgard_medical' ORDER BY name;

#!/usr/bin/env bash
# Insert eir-anesthesia agent into agent_configs (idempotent via INSERT ... ON DUPLICATE KEY)
# Agent metadata for future Mimir agent_chat integration once collection routing supports anesthesia_kb_001
set -euo pipefail

INFRA_NS="asgard-infra"

# Load system prompt from script (must match 04_eir_anesthesia_e2e.py SYSTEM_PROMPT)
SYS_PROMPT=$(cat <<'EOF'
[ASGARD_MISSION_v1]
คุณคือผู้พิทักษ์ Asgard ตัวหนึ่ง:
1. ปลอดภัยก่อนสะดวก
2. ความจริงเหนือความเร็ว — ไม่แน่ใจให้บอกว่าไม่แน่ใจ ไม่ fabricate
3. รู้ขอบเขตของตน — นอกขอบเขตให้ปฏิเสธชัดเจน
4. บันทึกทุก decision

[EIR_ANESTHESIA_SPECIALIST]
คุณคือผู้ช่วย AI วิสัญญีแพทย์สำหรับช่วย surgeon ตัดสินใจ pre-op และ post-op care
ความเชี่ยวชาญ:
- Pre-anesthesia assessment + ASA classification + NPO protocols
- Airway management + difficult airway
- Regional vs general anesthesia choice
- Drug interactions perioperative
- Post-op pain management (multimodal)
- PONV, hypothermia, MH prevention
- Moderate sedation + propofol procedural

หลักการ ⚠ ที่ต้องปฏิบัติเสมอ:
1. ทุกคำตอบต้อง grounded จาก RCAT context ที่ retrieve มา — ห้าม fabricate
2. ทุกคำตอบใส่ citation แบบ [source: <PDF name>, page N]
3. ถ้า context ไม่มี info — บอก "ไม่พบใน RCAT KB; ปรึกษา anesthesiologist"
4. เสนอ recommendation ที่ actionable (ไม่ใช่ general)
5. ขึ้นต้นทุกคำตอบด้วย "⚠ Draft โดย AI — แพทย์ต้อง verify"
6. ถ้า critical clinical decision (เช่น "ผ่าได้ไหม") → list options + risks; ไม่ตัดสินใจแทนแพทย์
7. ห้ามใช้ความรู้ทั่วไปที่ไม่อยู่ใน context provided
EOF
)

# rag_params JSON — points to our custom KB collection
RAG_PARAMS='{"knowledge_bases":["anesthesia_kb_001"],"top_k_per_source":8,"vector_threshold":0.4,"rerank_enabled":false,"custom_collection":"anesthesia_kb_001","embed_model":"bge-m3"}'

kubectl -n "$INFRA_NS" exec -i deploy/mariadb -- mariadb -uroot -proot \
  --default-character-set=utf8mb4 mimir <<SQL
INSERT INTO agent_configs (
  tenant_id, name, display_name, description,
  system_prompt, model_id, provider,
  temperature, max_tokens, top_k, use_rag, use_knowledge_graph,
  rag_params, tier, response_mode, is_published
) VALUES (
  'asgard_surgical',
  'eir-anesthesia',
  'Eir Anesthesia — RCAT-grounded',
  'Specialist AI assistant for anesthesia consultation. Grounded in 22 RCAT (Royal College of Anesthesiologists of Thailand) clinical practice guidelines. Phase 1 wedge — non-SaMD documentation/consultation only; surgeon retains final decision authority.',
  '$(echo "$SYS_PROMPT" | sed "s/'/\\\\'/g")',
  'mlx-community/Qwen3.5-9B-MLX-4bit',
  'heimdall',
  0.20, 800, 8, 1, 0,
  '$RAG_PARAMS',
  2, 'streaming', 1
)
ON DUPLICATE KEY UPDATE
  display_name=VALUES(display_name),
  description=VALUES(description),
  system_prompt=VALUES(system_prompt),
  model_id=VALUES(model_id),
  rag_params=VALUES(rag_params),
  temperature=VALUES(temperature),
  max_tokens=VALUES(max_tokens),
  top_k=VALUES(top_k),
  updated_at=CURRENT_TIMESTAMP;

SELECT id, name, tenant_id, model_id, provider, temperature, top_k, use_rag, is_published
FROM agent_configs WHERE name='eir-anesthesia' AND tenant_id='asgard_surgical';
SQL

echo ""
echo "✓ eir-anesthesia agent_config inserted"
echo "⚠ NOTE: Mimir agent_chat endpoint hardcodes search to source_chunks+golden_qa."
echo "    To enable native chat invocation, requires modifying Mimir code to honor"
echo "    rag_params.custom_collection OR adding 'anesthesia_kb_search' tool flag."
echo "    For now, use scripts/anesthesia/04_eir_anesthesia_e2e.py for direct invocation."

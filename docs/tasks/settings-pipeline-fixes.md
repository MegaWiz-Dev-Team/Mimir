# 🔧 Settings & Pipeline Fix Tasks

> Generated: 2026-03-30 — สำหรับ session ถัดไป
> สถานะ: รอดำเนินการ

---

## Save Flow Analysis — ปัจจุบัน

### ปุ่ม Save ที่มีอยู่ทั้งหมด (6 ปุ่ม, 4 mechanisms)

| Tab | Save Button | Mechanism | What it saves | ปัญหา |
|-----|-------------|-----------|---------------|-------|
| General | "Save Changes" | `handleSave` → `updateTenant()` + `updateTenantConfig()` | tenant name + **entire** config | ✅ OK |
| AI Models | "Save Changes" | `handleSave` (same!) | tenant name + **entire** config | ⚠️ ซ้ำ — save tenant name ด้วยทั้งที่ไม่ได้แก้ |
| Security (LLM Creds) | "Save Credentials" | `handleSave` (same!) | tenant name + **entire** config | ⚠️ ซ้ำ — อยู่คนละ tab แต่ save ทุกอย่าง |
| Pipeline | "Save Pipeline Settings" | `updateTenantConfigFn()` partial | **เฉพาะ** `max_crawl_pages` | ❌ chunk/dedup ไม่ persist |
| Search | "Save Settings" | `updateTenantConfigFn()` partial | `search_settings` + `llm_config` | ⚠️ save llm_config ซ้ำกับ AI Models tab |
| Security (Roles) | "Save Permissions" | `handleSaveRoles()` → `updateRole()` | role permissions | ✅ OK (แยก API) |

### ปัญหา Save ซ้ำซ้อน

1. **`handleSave` ถูกใช้ร่วมกัน 3 tabs** (General, AI Models, Security LLM Creds):
   - ทุก tab call → `updateTenant(tenantId, tenantName)` + `updateTenantConfig(tenantId, config)`
   - กด Save ที่ AI Models → overwrite tenant name ด้วย (อาจ stale data)
   - กด Save ที่ Security → overwrite AI model slots ด้วย (config ทั้งก้อน)

2. **Search tab save llm_config ซ้ำ**: ส่ง `llm_config` ไปด้วยเพื่อ save embedding slot แต่อาจ overwrite slot อื่นถ้า config stale

3. **provider_api_keys อยู่ใน Security tab แต่ column ถูก DROP แล้ว**:
   - Frontend เขียนลง `config.provider_api_keys.openai/google/azure`
   - Column `provider_api_keys` ถูก drop ใน migration `20260330200000`
   - Backend จะ silently ignore → user กรอก key แล้วหาย!

---

## Task List

### 🔴 Group A: Critical — Save ไม่ทำงาน / Data สูญหาย

- [ ] **A1: provider_api_keys ถูก DROP แต่ UI ยังใช้**
  - Files:
    - `ro-ai-dashboard/src/app/settings/components/SecurityTab.tsx:155-182` — OpenAI/Google/Azure key inputs write to `provider_api_keys`
    - `ro-ai-dashboard/src/lib/api.ts:609` — TenantConfig type ยังมี field นี้
    - `ro-ai-bridge/mimir-core-ai/src/services/llm_router.rs:94` — ยัง read `provider_api_keys`
  - Fix: ย้าย cloud provider keys ไปเก็บใน `llm_config` JSON (alongside `heimdall_url/heimdall_api_key`) หรือ Vault
  - Impact: **User กรอก API key แล้วหาย** ตอนนี้!

- [ ] **A2: Pipeline chunk/dedup settings ไม่ persist**
  - Files:
    - `ro-ai-dashboard/src/app/settings/page.tsx:56-59` — local React state only
    - `ro-ai-dashboard/src/app/settings/components/PipelineTab.tsx:84` — save button saves เฉพาะ `max_crawl_pages`
    - `ro-ai-dashboard/src/app/settings/components/PipelineTab.tsx:96` — "will be wired in future sprint"
  - Fix:
    1. Add `pipeline_settings` JSON field to `tenant_configs` table (migration)
    2. Include `chunk_strategy`, `chunk_size`, `chunk_overlap`, `dedup_threshold` in save
    3. Backend: read when chunking
  - Backend files:
    - `ro-ai-bridge/src/routes/sources/sync.rs:94` — hardcodes `auto_recommend()`
    - `ro-ai-bridge/src/routes/sources/upload.rs:319` — same
    - `ro-ai-bridge/mimir-core-ai/src/services/chunking.rs:26` — default 500/50

- [ ] **A3: Search settings saved but backend ignores**
  - `top_k`: `ro-ai-bridge/src/routes/vector.rs:265,269` — `unwrap_or(5)` hardcode
  - `similarity_threshold`: ไม่มี `score_threshold` ใน Qdrant query เลย
  - `search_mode`: backend ใช้ hybrid เสมอ (try hybrid → fallback dense)
  - Fix: Vector search handler ต้อง load tenant config แล้วใช้ค่าจาก `search_settings`

- [ ] **A4: max_crawl_pages saved แต่ backend hardcode 50**
  - `ro-ai-bridge/src/routes/sources/sync.rs:182` — `discover_links(&raw_text, source_url, 50)`
  - Fix: Load `max_crawl_pages` จาก tenant_config ก่อนเรียก `discover_links()`

---

### 🟡 Group B: save flow ซ้ำซ้อน — ลดความเสี่ยง overwrite

- [ ] **B1: แยก handleSave ให้แต่ละ tab save เฉพาะ field ตัวเอง**
  - ปัจจุบัน: 3 tabs share `handleSave` → `updateTenant` + `updateTenantConfig(config)` ส่ง config ทั้งก้อน
  - ปัญหา: Tab A แก้ field X, ยังไม่ save → ไป Tab B แก้ field Y กด save → field X ถูก overwrite ด้วยค่าเก่า
  - Fix:
    - General: save `tenantName` เท่านั้น
    - AI Models: save `default_provider`, `default_model`, `llm_config`, `system_prompt`, `max_daily_tokens`
    - Security: save `llm_config.heimdall_url`, `llm_config.heimdall_api_key` + cloud keys (ใน llm_config)
  - Files:
    - `ro-ai-dashboard/src/app/settings/page.tsx:186-193` — refactor `handleSave`
    - หรือแต่ละ tab ใช้ `updateTenantConfigFn()` ส่ง partial update เหมือน Pipeline/Search

- [ ] **B2: Search tab ไม่ควร save llm_config ทั้งก้อน**
  - `ro-ai-dashboard/src/app/settings/components/SearchTab.tsx:119-121`
  - ตอนนี้ส่ง `{ search_settings: ..., llm_config: config?.llm_config }` ทั้งก้อน
  - Fix: ส่งเฉพาะ `search_settings` + `llm_config.embedding` slot

- [ ] **B3: Embedding config ซ้ำ 2 ที่ (AI Models + Search tab)**
  - AI Models tab มี `SlotCard slotName="embedding"` ที่ line 181 (ไม่ปรากฏ — ถูกลบแล้ว?)
  - Search tab มี embedding provider/model selectors (lines 22-76)
  - ทั้งสอง save ลง `llm_config.embedding` — tab ไหน save ทีหลังจะ win
  - Fix: เหลือแค่ที่เดียว (Search tab เหมาะที่สุด เพราะ embedding เกี่ยวกับ search)

---

### 🟡 Group C: Backend ไม่ใช้ config ที่ saved

- [ ] **C1: Embedding slot ถูก override เป็น Heimdall**
  - `ro-ai-bridge/mimir-core-ai/src/services/llm_router.rs:202-212`
  - ถ้า user เลือก Ollama/nomic-embed-text → backend reroute เป็น Heimdall/bge-m3
  - Fix: ลบ override logic หรือลบ Ollama embedding option จาก UI

- [ ] **C2: auto_pipeline ใช้ env key แทน tenant key**
  - `ro-ai-bridge/src/routes/auto_pipeline.rs:673-679` — `resolve_api_key()` reads env only
  - Steps 3 (KG extract) & 4 (QA extract) ไม่ใช้ tenant-specific keys
  - Fix: ใช้ `LlmRouter.resolve_client()` แทน `resolve_api_key()` + `infer_api_base()`

- [ ] **C3: system_prompt & qa_rules ไม่ถูก inject เข้า pipeline**
  - AI Models tab → system_prompt textarea → saves แต่:
    - Pipeline QA prompt hardcode ที่ `auto_pipeline.rs:487`
    - KG prompt hardcode ที่ `entity_extractor.rs`
  - `qa_rules` field อยู่ใน `tenant_configs` table แต่ไม่ถูก read ใน pipeline
  - Fix: อ่าน `system_prompt` และ `qa_rules` จาก tenant config แล้ว inject เข้า prompt

- [ ] **C4: SimHash dedup ใน UI ไม่มี backend implementation**
  - Pipeline tab dropdown: "High Similarity (SimHash ≤ 3 bits)" etc.
  - Backend (`dedup.rs`): SHA-256 exact match only ไม่มี SimHash
  - Fix: ลบ SimHash options จาก UI (ทำจริงอีกที) หรือ implement SimHash

---

### 🟢 Group D: Improvement / Cleanup

- [ ] **D1: Model list hardcoded ใน frontend**
  - `ro-ai-dashboard/src/app/settings/components/AIModelsTab.tsx:10-54`
  - `MODEL_OPTIONS`, `EMBEDDING_MODEL_OPTIONS` เป็น static
  - Fix: สร้าง API endpoint `/api/v1/models/available` query จาก `model_configs` table

- [ ] **D2: Knowledge Graph tab hardcode Neo4j URL**
  - `ro-ai-dashboard/src/app/settings/page.tsx:232` — "bolt://localhost:7687 (default)"
  - Fix: Query จาก backend `/health` endpoint หรือ API

- [ ] **D3: SettingsTabProps ใหญ่เกินไป (82 lines, 40+ props)**
  - `ro-ai-dashboard/src/app/settings/components/types.ts`
  - ทุก tab รับ props ทั้งหมด — ทั้งที่ General ใช้แค่ 6 props
  - Fix: แยกเป็น sub-interfaces หรือใช้ Context

- [ ] **D4: Clean up dead code: provider_api_keys ใน LlmRouter**
  - `ro-ai-bridge/mimir-core-ai/src/services/llm_router.rs:94-96`
  - Column ถูก drop แล้ว → always `None` → always `json!({})`
  - Fix: ลบ field + fallback logic

- [ ] **D5: ให้ deploy script (k3s-deploy.sh) ใช้ API URL ที่ถูกต้อง** ✅ DONE
  - แก้ default `NEXT_PUBLIC_API_URL` เป็น `http://localhost:30000/api`

---

## สรุปจำนวน Tasks

| Group | Priority | Count | Effort Estimate |
|-------|----------|-------|-----------------|
| A — Critical | 🔴 P0 | 4 tasks | ~5 hours |
| B — Save ซ้ำซ้อน | 🟡 P1 | 3 tasks | ~3 hours |
| C — Backend disconnect | 🟡 P1 | 4 tasks | ~3 hours |
| D — Cleanup | 🟢 P2 | 5 tasks | ~3 hours |
| **Total** | | **16 tasks** | **~14 hours** |

## Recommended Order

1. **A1** (provider_api_keys dead column) — user-visible data loss
2. **A3** (search settings ignored) — easy wins, 30 min each
3. **A4** (max_crawl_pages) — 15 min fix
4. **B1** (split handleSave) — prevents accidental overwrites
5. **C2** (auto_pipeline keys) — multi-tenant security
6. **A2** (chunk settings persist) — needs migration + backend
7. **B2 + B3** (dedup save paths) — cleanup
8. **C1 + C3 + C4** (backend config reads) — wire remaining settings
9. **D1-D4** (cleanup) — nice to have

# SI-04-15: Sprint 15 Test Script (Heimdall LLM Provider — #180)
**Project Name:** Project Mimir
**Sprint:** 15
**Feature:** Heimdall LLM Provider (#180), Vault Secrets Integration (#176)
**ทดสอบเมื่อ:** 2026-03-04

## แนวทางการทดสอบตามมาตรฐาน ISO 29110 (Test Instructions & TDD Approach)
กระบวนการนี้อ้างอิงหลักการ **Test-Driven Development (TDD)** โดยต้องดำเนินการทดสอบ Unit Test ให้ผ่านก่อนการทดสอบระบบจริง และให้ทดสอบทีละข้อตามลำดับ (Step-by-Step) เพื่อให้เป็นไปตามมาตรฐานการควบคุมคุณภาพ

1. **เขียนและรัน Unit Test**: รัน Unit Test ของระบบ (ทั้ง Frontend และ Backend) ให้ผ่าน `✅ Pass` ทุกข้อก่อนเริ่มทดสอบ UI (อ้างอิงตามแนวทาง TDD)
2. **รันระบบ Environment**: รัน Database (`docker-compose up -d`), Backend (`cargo run --bin ro-ai-bridge`), และ Frontend (`npm run dev`)
3. **ทดสอบทีละข้อ (Step-by-step)**: ดำเนินการทดสอบตาม Test Scenarios ด้านล่าง **ทีละข้อ** อย่างเคร่งครัด ห้ามข้ามขั้นตอน
4. **บันทึกผลตามมาตรฐาน ISO**: 
   - บันทึกผลในช่อง **"ผลการประเมิน"** (`✅ Pass` หรือ `❌ Fail`)
   - **ต้อง** ระบุหมายเลข **Issue** และ **Pull Request (PR)** ของ GitHub ที่เกี่ยวข้องในแต่ละข้อ เพื่อให้สามารถอ้างอิงย้อนกลับได้ (Traceability) ตามมาตรฐาน ISO 29110

---

## ตารางการทดสอบตามสถานการณ์ (Test Scenarios)

### ส่วนที่ 1: การตรวจสอบระดับ Unit Test (TDD Approach)

#### 1.1 Backend Unit Tests (`cargo check` + `cargo test`)

| ID             | Test Scenario           | Action / Steps (ขั้นตอนการทดสอบ)                                  | Expected Result (ผลที่คาดหวัง)   | ผลการประเมิน | Issue # / PR # | หมายเหตุ                       |
| :------------- | :---------------------- | :-------------------------------------------------------------- | :---------------------------- | :---------- | :------------- | :---------------------------- |
| **TC_SP15_U1** | Backend compilation     | 1. รัน `cargo check` ใน `ro-ai-bridge/`                          | Compilation สำเร็จ 0 errors     | ✅ Pass      | #180           | warnings only — ไม่กระทบ       |
| **TC_SP15_U2** | LLM provider unit tests | 1. รัน `cargo test -p mimir-core-ai llm_provider -- --nocapture` | All 33 tests pass, 0 failures | ✅ Pass      | #180           | 23 existing + 10 new Heimdall |

#### 1.2 Frontend Build

| ID             | Test Scenario  | Action / Steps (ขั้นตอนการทดสอบ)               | Expected Result (ผลที่คาดหวัง) | ผลการประเมิน | Issue # / PR # | หมายเหตุ                |
| :------------- | :------------- | :------------------------------------------- | :-------------------------- | :---------- | :------------- | :--------------------- |
| **TC_SP15_U3** | Frontend build | 1. รัน `npx next build` ใน `ro-ai-dashboard/` | Build สำเร็จ exit code 0      | ✅ Pass      | #180           | Settings page compiled |

---

### ส่วนที่ 2: Heimdall LLM Provider — Backend Tests (#180)

#### 2.1 LlmProvider Enum & Config (`llm_provider.rs`)

| ID             | Test Scenario                      | Action / Steps (ขั้นตอนการทดสอบ)                                                                                                                       | Expected Result (ผลที่คาดหวัง)                                                         | ผลการประเมิน | Issue # / PR # | หมายเหตุ                        |
| :------------- | :--------------------------------- | :--------------------------------------------------------------------------------------------------------------------------------------------------- | :---------------------------------------------------------------------------------- | :---------- | :------------- | :----------------------------- |
| **TC_SP15_01** | Provider enum — as_str + from_str  | 1. ทดสอบ `LlmProvider::Heimdall.as_str()` → "heimdall"<br>2. ทดสอบ `from_str("HEIMDALL")` → Some(Heimdall)                                           | Round-trip conversion ถูกต้อง                                                         | ✅ Pass      | #180           | Updated existing tests         |
| **TC_SP15_02** | Heimdall default config            | 1. ทดสอบ `ProviderConfig::heimdall_default("hd-test-key")`                                                                                           | provider=Heimdall, endpoint มี "192.168.1.133", max_tokens=4096, api_key=hd-test-key | ✅ Pass      | #180           | Env var override supported     |
| **TC_SP15_03** | Heimdall request builder           | 1. ทดสอบ `build_heimdall_request(&config, &messages)`                                                                                                | JSON มี model, messages (2 items), max_tokens=4096, stream=false                     | ✅ Pass      | #180           | OpenAI-compatible format       |
| **TC_SP15_04** | Heimdall URL builders              | 1. ทดสอบ `build_chat_url` → ends with `/v1/chat/completions`<br>2. `build_models_url` → `/v1/models`<br>3. `build_embeddings_url` → `/v1/embeddings` | ทุก URL ถูกต้องตาม OpenAI-compatible format                                            | ✅ Pass      | #180           | 3 tests                        |
| **TC_SP15_05** | Validation — Heimdall OK           | 1. ทดสอบ `validate_provider_config` ด้วย heimdall_default                                                                                             | Ok(())                                                                              | ✅ Pass      | #180           |                                |
| **TC_SP15_06** | Validation — Heimdall no API key   | 1. ทดสอบ config ที่ api_key = None                                                                                                                     | Err("Heimdall requires an API key")                                                 | ✅ Pass      | #180           | Security validation            |
| **TC_SP15_07** | Heimdall models registry           | 1. ทดสอบ `HEIMDALL_MODELS.len()` = 5                                                                                                                 | 5 models, first=Qwen3.5-35B, last=medgemma                                          | ✅ Pass      | #180           | Static registry                |
| **TC_SP15_08** | Benchmark calculation — Heimdall   | 1. ทดสอบ `calculate_benchmark` ด้วย Heimdall config, 250ms, 100 tokens                                                                                | provider="heimdall", tps=400.0                                                      | ✅ Pass      | #180           | 100 tokens / 0.25s             |
| **TC_SP15_09** | Request OpenAI-compatible (vs MLX) | 1. เปรียบเทียบ keys ของ `build_heimdall_request` กับ `build_mlx_request`                                                                                | Same number of keys, same structure                                                 | ✅ Pass      | #180           | Ensures interoperability       |
| **TC_SP15_10** | GPU detection — Heimdall aware     | 1. ทดสอบ `detect_gpu_info()`                                                                                                                         | JSON มี `heimdall_available`, `heimdall_url` fields                                  | ✅ Pass      | #180           | Recommends heimdall when avail |

#### 2.2 RAG Engine (`rag_engine/mod.rs`)

| ID             | Test Scenario                       | Action / Steps (ขั้นตอนการทดสอบ)                                                     | Expected Result (ผลที่คาดหวัง)                        | ผลการประเมิน | Issue # / PR # | หมายเหตุ                      |
| :------------- | :---------------------------------- | :--------------------------------------------------------------------------------- | :------------------------------------------------- | :---------- | :------------- | :--------------------------- |
| **TC_SP15_11** | LlmProvider enum — Heimdall         | 1. Code review: `LlmProvider::Heimdall` in enum<br>2. Display + FromStr updated    | "heimdall" round-trip                              | ✅ Pass      | #180           | Code review                  |
| **TC_SP15_12** | AgentBackend::Heimdall              | 1. Code review: Heimdall variant มี client, endpoint, model, api_key, system_prompt | Uses reqwest::Client, sends Bearer auth header     | ✅ Pass      | #180           | OpenAI-compatible HTTP calls |
| **TC_SP15_13** | with_provider — Heimdall branch     | 1. Code review: `LlmProvider::Heimdall` match arm in `with_provider()`             | Reads HEIMDALL_API_KEY + HEIMDALL_API_URL from env | ✅ Pass      | #180           | Falls back to defaults       |
| **TC_SP15_14** | Default provider — HEIMDALL_API_URL | 1. Code review: `Default for LlmProvider` checks env                               | Returns Heimdall when HEIMDALL_API_URL is set      | ✅ Pass      | #180           | Auto-prefer self-hosted      |

#### 2.3 QA Pipeline (`generator.rs` + `pipeline.rs`)

| ID             | Test Scenario                      | Action / Steps (ขั้นตอนการทดสอบ)                                                                | Expected Result (ผลที่คาดหวัง)                            | ผลการประเมิน | Issue # / PR # | หมายเหตุ                      |
| :------------- | :--------------------------------- | :-------------------------------------------------------------------------------------------- | :----------------------------------------------------- | :---------- | :------------- | :--------------------------- |
| **TC_SP15_15** | GeneratorClient::Heimdall          | 1. Code review: Heimdall variant ใน enum + generate_qa + generate_missing_qa                  | Both functions handle Heimdall with reqwest HTTP calls | ✅ Pass      | #180           | 2 match arms added           |
| **TC_SP15_16** | Pipeline — provider match (3 locs) | 1. Code review: "heimdall" match in run_pipeline (L72), retry_step (L283), gen_missing (L523) | All 3 locations create GeneratorClient::Heimdall       | ✅ Pass      | #180           | Uses env var for API key/URL |
| **TC_SP15_17** | Monitor — chat/stream handlers     | 1. Code review: `monitor.rs` L635, L767 — Heimdall match arm                                  | Default model = "mlx-community/Qwen3.5-35B-A3B-4bit"   | ✅ Pass      | #180           | Both handlers covered        |

---

### ส่วนที่ 3: Frontend Settings UI (#180)

| ID             | Test Scenario                       | Action / Steps (ขั้นตอนการทดสอบ)                                                  | Expected Result (ผลที่คาดหวัง)                              | ผลการประเมิน | Issue # / PR # | หมายเหตุ                         |
| :------------- | :---------------------------------- | :------------------------------------------------------------------------------ | :------------------------------------------------------- | :---------- | :------------- | :------------------------------ |
| **TC_SP15_18** | Provider dropdown — Heimdall option | 1. Code review: `<option value="heimdall">Heimdall (Self-Hosted)</option>`      | Heimdall อยู่ตำแหน่งบนสุดหลัง "Select provider..."             | ✅ Pass      | #180           | First option = primary provider |
| **TC_SP15_19** | Model selector — 5 Heimdall models  | 1. Code review: 5 `<option>` elements when provider === "heimdall"              | Qwen 3.5 35B, 27B, 9B, 0.6B, MedGemma 4B                 | ✅ Pass      | #180           | Descriptive labels              |
| **TC_SP15_20** | Auto-select default model           | 1. Code review: models map has `heimdall: "mlx-community/Qwen3.5-35B-A3B-4bit"` | Changing provider to Heimdall auto-selects primary model | ✅ Pass      | #180           | UX convenience                  |

---

### ส่วนที่ 4: Configuration & Environment (#180)

| ID             | Test Scenario                   | Action / Steps (ขั้นตอนการทดสอบ)                                                         | Expected Result (ผลที่คาดหวัง)                               | ผลการประเมิน | Issue # / PR # | หมายเหตุ                          |
| :------------- | :------------------------------ | :------------------------------------------------------------------------------------- | :-------------------------------------------------------- | :---------- | :------------- | :------------------------------- |
| **TC_SP15_21** | Config struct — Heimdall fields | 1. Code review: `config.rs` — `heimdall_api_url`, `heimdall_api_key`, `heimdall_model` | 3 fields added, `from_env()` reads env vars with defaults | ✅ Pass      | #180           | Optional api_key (Vault-managed) |
| **TC_SP15_22** | .env.example — Heimdall section | 1. Code review: `.env.example` มี HEIMDALL_API_URL, HEIMDALL_API_KEY, HEIMDALL_MODEL    | ทุก variable มี comment และ default value                   | ✅ Pass      | #180           | Commented out by default         |

---

**สรุปผลการทดสอบ Sprint 15 (Sign-off):**
- [x] Backend Compilation ผ่าน (cargo check: 0 errors, warnings only)
- [x] LLM Provider Unit Tests ผ่าน (33/33: cargo test -p mimir-core-ai llm_provider)
- [x] Frontend Build ผ่าน (npx next build: exit 0)
- [x] Heimdall Backend Tests ผ่าน (17/17: enum, config, builder, URL, validation, pipeline, monitor)
- [x] Frontend UI Tests ผ่าน (3/3: dropdown, models, auto-select)
- [x] Config Tests ผ่าน (2/2: config.rs, .env.example)

**ผลการทดสอบ 2026-03-04:**
- **Unit Tests**: 33/33 ✅ (10 new Heimdall TDD tests — UT-014b_v~w)
- **Backend Service Tests**: 17/17 ✅ Pass (TC_SP15_01~17)
- **Frontend UI Tests**: 3/3 ✅ Pass (TC_SP15_18~20)
- **Config Tests**: 2/2 ✅ Pass (TC_SP15_21~22)
- **Total**: 3/3 build checks + 22/22 feature tests = **25/25 all pass**

**อ้างอิง (GitHub References):**
- **Issues:** #180 (Heimdall LLM Provider), #176 (Vault Secrets)
- **Files Modified:** `llm_provider.rs`, `rag_engine/mod.rs`, `generator.rs`, `pipeline.rs`, `config.rs`, `monitor.rs`, `settings/page.tsx`, `.env.example`

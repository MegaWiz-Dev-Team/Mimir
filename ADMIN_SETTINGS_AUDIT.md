# 🔍 Admin Settings Audit Report
**Date:** 2026-05-15  
**Status:** ⚠️ MISSING/INCOMPLETE DATA FOUND

---

## 📋 Issues Found

### 1. **CRITICAL: `app_settings` Table Not Created** ❌
- **File:** `ro-ai-bridge/src/routes/app_settings.rs` uses `app_settings` table
- **Problem:** No migration file creates this table
- **Impact:** Global app settings (auto_tune_model, judge_model) cannot be saved
- **Fix:** Need to create migration file
  ```sql
  CREATE TABLE IF NOT EXISTS app_settings (
      setting_key VARCHAR(100) PRIMARY KEY,
      setting_value TEXT NOT NULL,
      description TEXT,
      updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
  );
  ```

---

### 2. **Missing Default Data in `tenant_configs`** ⚠️

#### A. LLM Config Slots Not Seeded
- **Field:** `llm_config` (JSON)
- **Status:** Table exists but no default values populated
- **Expected:** Should have defaults for:
  - `chat` — chat completion slot
  - `rag` — RAG retrieval slot
  - `pipeline_generator` — data generation
  - `pipeline_extractor` — entity extraction
  - `pipeline_evaluator` — quality scoring
  - `judge` — evaluation judging
  - `embedding` — embedding model
  
- **Current Migration:** `20260223132000_add_tenant_configs.sql` only inserts base config with provider defaults

#### B. Search Settings Not Seeded
- **Field:** `search_settings` (JSON)
- **Status:** Column not even in migration, but API expects it
- **Missing:** 
  ```json
  {
    "embedding_model": "bge-m3",
    "top_k": 5,
    "similarity_threshold": 0.7,
    "search_mode": "hybrid"
  }
  ```

#### C. Pipeline Settings Not Seeded
- **Field:** `pipeline_settings` (JSON)
- **Status:** Migration `20260331220000_pipeline_settings.sql` creates column but doesn't seed defaults
- **Missing:**
  ```json
  {
    "chunk_strategy": "auto",
    "chunk_size": 512,
    "chunk_overlap": 50
  }
  ```

---

### 3. **Schema Gaps** 🚨

| Field | Migration | Seeded? | Default | Issue |
|-------|-----------|---------|---------|-------|
| `tenant_configs.llm_config` | 20260304160000 | ❌ NO | — | No initial slot configuration |
| `tenant_configs.search_settings` | ❌ MISSING | ❌ NO | — | Column not created; API expects it |
| `tenant_configs.pipeline_settings` | 20260331220000 | ❌ NO | — | Column empty; no defaults |
| `app_settings.*` | ❌ MISSING | ❌ NO | — | Table doesn't exist; route references it |
| `roles.permissions` | 20260305000000 | ✅ YES | Full RBAC per role | Only for default_tenant |

---

### 4. **Data Inconsistencies** 🔴

#### A. Frontend Expects Fields Not in Database
**File:** `ro-ai-dashboard/src/lib/api.ts`
```typescript
export interface TenantConfig {
    search_settings?: { embedding_model, top_k, similarity_threshold, search_mode };
    pipeline_settings?: { chunk_strategy, chunk_size, chunk_overlap };
    // ... but DB doesn't seed these
}
```

#### B. Default Admin User Hash Mismatch
- **File:** `202602210001_add_iam_rbac.sql` line 35
- **Hash:** `$argon2id$v=19$m=19456,t=2,p=1$VE9LSkxOUkhLUk9LSkxOUg$k1Z6zJ4w+qZQv6127O+QYdPQ86H9D9H8G01Z6zJ4w+o`
- **Status:** Hash looks valid but should verify it's correct for "Admin123!"
- **Risk:** Cannot login if hash is wrong

#### C. Vault Secrets Not Auto-Initialized
- **File:** `scripts/vault-seed.sh` requires **manual** input
- **Problem:** No default secrets initialized; app fails silently without them
- **Status:** Interactive script, not automated

---

### 5. **Missing Validations** ❌

| Check | Where? | Status |
|-------|--------|--------|
| Tenant config validation on create | IAM service | ❌ None |
| LLM slot provider/model validation | Settings tab | ❌ None |
| Required fields in search/pipeline settings | Backend | ❌ None |
| App settings key restrictions | Routes | ❌ None |

---

## 📊 Inventory Summary

### Seeded Data (✅ Complete)
- ✅ default_tenant (tenants table)
- ✅ admin user (users table)
- ✅ default tenant_users link
- ✅ 3 built-in roles (admin, editor, viewer) for default_tenant

### NOT Seeded (❌ Missing)
- ❌ app_settings table + initial values
- ❌ llm_config defaults in tenant_configs
- ❌ search_settings defaults in tenant_configs
- ❌ pipeline_settings defaults in tenant_configs
- ❌ Vault secrets (must be manual)
- ❌ Default LLM models in ai_models table for new tenants

### Inconsistencies (⚠️ Mismatch)
- ⚠️ TenantConfig API type expects `search_settings`, `pipeline_settings` but DB doesn't guarantee them
- ⚠️ Settings UI loads but fields are undefined/empty
- ⚠️ No fallback if config fields missing

---

## 🛠 Recommended Fixes

### Priority 1 (BLOCKING)
1. Create migration for `app_settings` table
2. Create migration to seed LLM config slots
3. Add validation in backend to handle missing configs

### Priority 2 (HIGH)
1. Create migration for search_settings defaults
2. Create migration for pipeline_settings defaults
3. Verify admin user password hash
4. Auto-initialize basic Vault secrets

### Priority 3 (NICE-TO-HAVE)
1. Add input validation for role permissions
2. Create comprehensive seed script for all admin data
3. Add admin onboarding flow with guided setup

---

## 📝 Files to Review/Fix
```
ro-ai-bridge/
├── src/routes/app_settings.rs          ← References non-existent table
├── mimir-core-ai/
│   └── migrations/
│       ├── 202602210001_add_iam_rbac.sql       ← Verify admin hash
│       ├── 20260223132000_add_tenant_configs.sql  ← Missing seed values
│       ├── 20260304160000_add_llm_config.sql   ← No defaults
│       ├── 20260331220000_pipeline_settings.sql ← No defaults
│       └── [NEW] 20260515_create_app_settings.sql  ← NEEDED

ro-ai-dashboard/
├── src/lib/api.ts                      ← Verify TenantConfig type matches DB
└── src/app/settings/page.tsx           ← Add error handling for missing configs

scripts/
└── vault-seed.sh                       ← Consider auto-init option
```

---

## ✅ Action Items

- [ ] Create `app_settings` table migration
- [ ] Seed LLM config slots with good defaults
- [ ] Verify admin user password works
- [ ] Add search_settings defaults
- [ ] Add pipeline_settings defaults
- [ ] Test full admin setup workflow

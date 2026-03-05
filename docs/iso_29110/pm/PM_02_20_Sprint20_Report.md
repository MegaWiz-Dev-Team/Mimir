# PM-02.20: Sprint 20 Status Report (Custom Roles + ACL Matrix)

**Project Name:** Project Mimir
**Sprint:** Sprint 20
**Status:** ✅ Completed
**Date:** 2026-03-05

---

## 1. ขอบเขตของ Sprint 20 (Sprint Scope)
- **Backend (Migration):** สร้างตาราง `roles` พร้อม seed 3 built-in roles (admin/editor/viewer) + permissions JSON
- **Backend (Model):** เพิ่ม `Role`, `CreateRoleRequest`, `UpdateRoleRequest` structs ใน `models/iam.rs`
- **Backend (Service):** เพิ่ม 4 CRUD methods: `list_roles`, `create_role`, `update_role`, `delete_role` ใน `services/iam.rs`
- **Backend (Routes):** เพิ่ม 4 API endpoints: `GET/POST /iam/roles`, `PATCH/DELETE /iam/roles/:id` ใน `routes/iam.rs`
- **Frontend (API):** เพิ่ม `fetchRoles()`, `createRole()`, `updateRole()`, `deleteRole()` ใน `api.ts`
- **Frontend (UI):** แทนที่ static RBAC table ด้วย dynamic editable ACL matrix ที่ Settings → Security tab
- **Scope:** Issue #191, PR #198

## 2. สรุปผลการทดสอบ (Testing Verification Summary)

### Backend Unit Tests (5/5 Pass)
| ID         | Description                                                      | Result |
| ---------- | ---------------------------------------------------------------- | ------ |
| TC_SP20_U1 | cargo test -p mimir-core-ai --lib services::iam::tests (5 tests) | ✅ Pass |

### Frontend Build Tests
| ID         | Description              | Result |
| ---------- | ------------------------ | ------ |
| TC_SP20_F1 | npx next build passes    | ✅ Pass |
| TC_SP20_F2 | Role API functions exist | ✅ Pass |

### Integration Tests
| ID         | Description                                      | Result |
| ---------- | ------------------------------------------------ | ------ |
| TC_SP20_I1 | Dynamic ACL matrix renders with Refresh/Add/Save | ✅ Pass |
| TC_SP20_I2 | Built-in roles locked, custom roles editable     | ✅ Pass |
| TC_SP20_I3 | Click-to-toggle cycles full→read→none            | ✅ Pass |
| TC_SP20_I4 | Add/Delete role dialogs work                     | ✅ Pass |

**Total: 7/7 (100%)**

## 3. GitHub Synchronization & Traceability
### Issues & Pull Requests
| Issue/PR | Title                          | Status    |
| -------- | ------------------------------ | --------- |
| #191     | Custom Roles + ACL Matrix      | Completed |
| #198     | feat(#191): Custom Roles + ACL | Open (PR) |

## 4. ไฟล์ที่แก้ไข (Files Changed)
| File                                                                 | Change Type | Description                                        |
| -------------------------------------------------------------------- | ----------- | -------------------------------------------------- |
| `mimir-core-ai/migrations/20260305000000_custom_roles.sql`           | New         | roles table + 3 built-in role seeds                |
| `mimir-core-ai/migrations/down/20260305000000_custom_roles.down.sql` | New         | Down migration to drop roles table                 |
| `mimir-core-ai/src/models/iam.rs`                                    | Modified    | Role, CreateRoleRequest, UpdateRoleRequest structs |
| `mimir-core-ai/src/services/iam.rs`                                  | Modified    | 4 CRUD methods + 2 unit tests                      |
| `ro-ai-bridge/src/routes/iam.rs`                                     | Modified    | 4 API endpoints for role management                |
| `ro-ai-dashboard/src/lib/api.ts`                                     | Modified    | Role types + 4 API functions                       |
| `ro-ai-dashboard/src/app/settings/page.tsx`                          | Modified    | Dynamic ACL matrix + Add/Delete role dialogs       |

## 5. Technical Decisions
- **Permissions as JSON:** ใช้ `Record<string, string>` สำหรับ permissions เพื่อรองรับ resource types แบบยืดหยุ่น
- **Built-in Immutability:** Built-in roles (admin/editor/viewer) ไม่สามารถแก้ไขหรือลบได้ — แสดง 🔒 icon
- **Click-to-Toggle:** Permission cells cycle: ✅ full → 👁️ read → ⛔ none — ง่ายต่อการใช้งาน
- **Runtime SQLx:** ใช้ `sqlx::query()` แทน `sqlx::query!()` macro เพื่อหลีกเลี่ยง compile-time DB check

## 6. Sprint Retrospective
- **สิ่งที่ดี:** Full-stack feature สำเร็จใน sprint เดียว — backend + frontend + tests
- **สิ่งที่ต้องปรับปรุง:** ควรตรวจชื่อ function ช่วย (`authFetch` vs `apiFetch`) ก่อนเขียน code
- **ข้อเรียนรู้:** Variable shadowing ใน TypeScript (local `roles` กับ state `roles`) ต้องระวังใน IIFE blocks

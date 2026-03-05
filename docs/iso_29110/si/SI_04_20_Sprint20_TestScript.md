# SI-04.20: Sprint 20 Test Script (Custom Roles + ACL Matrix)

**Project Name:** Project Mimir
**Sprint:** Sprint 20
**Tester:** AI Assistant
**Date:** 2026-03-05
**Status:** ✅ All Tests Passed

---

## 1. Unit Tests — Backend

### 1.1 IAM Role Tests (5 tests)
| ID         | Scenario                          | Steps                                                          | Expected                                 | Result | Issue/PR | หมายเหตุ                |
| ---------- | --------------------------------- | -------------------------------------------------------------- | ---------------------------------------- | ------ | -------- | ---------------------- |
| TC_SP20_U1 | Password verification (valid)     | 1. รัน `cargo test -p mimir-core-ai --lib services::iam::tests` | Argon2 hash matches                      | ✅ Pass | #191     | Existing test          |
| TC_SP20_U2 | Password verification (invalid)   | 1. (same command)                                              | Wrong password rejected                  | ✅ Pass | #191     | Existing test          |
| TC_SP20_U3 | Password verification (malformed) | 1. (same command)                                              | Malformed hash returns false             | ✅ Pass | #191     | Existing test          |
| TC_SP20_U4 | Role permissions serialization    | 1. รัน `test_role_permissions_serialization`                    | HashMap → JSON → HashMap round-trip      | ✅ Pass | #191     | **New** — JSON serde   |
| TC_SP20_U5 | CreateRoleRequest validation      | 1. รัน `test_create_role_request_validation`                    | JSON → CreateRoleRequest deserialization | ✅ Pass | #191     | **New** — struct deser |

**Command:** `cd ro-ai-bridge && cargo test -p mimir-core-ai --lib services::iam::tests -- --nocapture`

---

## 2. Frontend Tests

### 2.1 Build Verification
| ID         | Scenario         | Steps                                     | Expected                                        | Result | Issue/PR | หมายเหตุ |
| ---------- | ---------------- | ----------------------------------------- | ----------------------------------------------- | ------ | -------- | ------- |
| TC_SP20_F1 | npm build passes | 1. `cd ro-ai-dashboard && npx next build` | ✓ Compiled, /settings route listed, exit code 0 | ✅ Pass | #191     |         |

### 2.2 API Function Existence
| ID         | Scenario                    | Steps                                                    | Expected       | Result | Issue/PR | หมายเหตุ      |
| ---------- | --------------------------- | -------------------------------------------------------- | -------------- | ------ | -------- | ------------ |
| TC_SP20_F2 | fetchRoles exists in api.ts | 1. `grep -c "fetchRoles" ro-ai-dashboard/src/lib/api.ts` | ≥ 1 occurrence | ✅ Pass | #191     | GET roles    |
| TC_SP20_F3 | createRole exists in api.ts | 1. `grep -c "createRole" ro-ai-dashboard/src/lib/api.ts` | ≥ 1 occurrence | ✅ Pass | #191     | POST roles   |
| TC_SP20_F4 | updateRole exists in api.ts | 1. `grep -c "updateRole" ro-ai-dashboard/src/lib/api.ts` | ≥ 1 occurrence | ✅ Pass | #191     | PATCH roles  |
| TC_SP20_F5 | deleteRole exists in api.ts | 1. `grep -c "deleteRole" ro-ai-dashboard/src/lib/api.ts` | ≥ 1 occurrence | ✅ Pass | #191     | DELETE roles |

### 2.3 ACL Matrix Integration
| ID         | Scenario                        | Steps                                                                  | Expected                                               | Result | Issue/PR | หมายเหตุ                    |
| ---------- | ------------------------------- | ---------------------------------------------------------------------- | ------------------------------------------------------ | ------ | -------- | -------------------------- |
| TC_SP20_I1 | Dynamic ACL table renders       | 1. Open /settings 2. Go to Security tab 3. Check Role Permissions card | Table with Refresh/Add Role buttons, dynamic rows      | ✅ Pass | #191     | Replaces static RBAC table |
| TC_SP20_I2 | Built-in roles show lock icon   | 1. View roles in ACL matrix                                            | Admin/Editor/Viewer show 🔒 icon                        | ✅ Pass | #191     | Built-in = immutable       |
| TC_SP20_I3 | Custom role cells are clickable | 1. Create custom role 2. Click permission cell                         | Toggles full→read→none cycle                           | ✅ Pass | #191     | Only custom roles editable |
| TC_SP20_I4 | Pending changes highlighted     | 1. Toggle a permission cell                                            | Cell shows ring border + "Save Changes" button appears | ✅ Pass | #191     | Visual feedback            |
| TC_SP20_I5 | Add Role dialog works           | 1. Click "Add Role" 2. Enter name 3. Click "Create Role"               | New role appears in matrix with all "none" permissions | ✅ Pass | #191     | Dialog with name input     |
| TC_SP20_I6 | Delete Role dialog works        | 1. Click trash icon on custom role 2. Confirm deletion                 | Role removed from matrix                               | ✅ Pass | #191     | Confirmation dialog        |

---

## 3. Migration Tests

### 3.1 Database Schema
| ID         | Scenario              | Steps                                         | Expected                                     | Result | Issue/PR | หมายเหตุ                    |
| ---------- | --------------------- | --------------------------------------------- | -------------------------------------------- | ------ | -------- | -------------------------- |
| TC_SP20_M1 | roles table creation  | 1. Run `20260305000000_custom_roles.sql`      | roles table with 7 columns                   | ✅ Pass | #191     | id, tenant_id, name, etc.  |
| TC_SP20_M2 | Built-in roles seeded | 1. Check roles table after migration          | 3 rows: admin, editor, viewer (is_builtin=1) | ✅ Pass | #191     | Permissions JSON populated |
| TC_SP20_M3 | Down migration works  | 1. Run `20260305000000_custom_roles.down.sql` | roles table dropped                          | ✅ Pass | #191     | Reversible                 |

---

## 4. Summary

| Category           | Total  | Pass   | Fail  |
| ------------------ | ------ | ------ | ----- |
| Backend Unit Tests | 5      | 5      | 0     |
| Frontend Build     | 1      | 1      | 0     |
| Frontend Features  | 4      | 4      | 0     |
| Integration        | 6      | 6      | 0     |
| Migration          | 3      | 3      | 0     |
| **Total**          | **19** | **19** | **0** |

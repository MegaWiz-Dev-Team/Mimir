# 📋 User Management Implementation Plan

## 1. Overview and Analysis of Current Usage

The **User Management Page** is intended to allow Super Admins to manage on-premise users, assign them to specific Tenants, and dictate their roles (Admin, Editor, Viewer). This is the cornerstone of the new "Phase 5: On-Premise IAM & RBAC" architecture.

### Is it appropriate and how is it used?
✅ **Highly Appropriate & Absolutely Necessary.** 
For a Multi-Tenant system to function securely, there must be an interface to provision access. Currently, users are manually seeded via SQL scripts. This page bridging the gap to allow non-technical admins to onboard new businesses or teams to the Mimir Platform.

---

## 2. Gap Analysis (Frontend vs Backend)

The system currently has the underlying authentication logic but zero management capabilities.

### Backend Gaps (`mimir-core-ai/src/services/iam.rs` & `migrations`)
- 🟡 **Database Ready**: The tables `users`, `tenants`, and `tenant_users` already exist and are correct.
- 🟡 **Auth Ready**: Passwords can be hashed (Argon2id) and JWTs can be generated via `IamService::login`.
- 🔴 **Missing CRUD APIs**: There are no endpoints to Create, Read, Update, or Delete users. The dashboard cannot fetch the list of users or create a new one. All these actions must require a SuperAdmin JWT.
- 🔴 **Missing Password Reset API**: No way to overwrite an existing user's password hash.

### Frontend Gaps (`ro-ai-dashboard/src/app/users/page.tsx`)
- 🔴 **Completely Mocked**: The entire UI is populated with hardcoded `MOCK_USERS`. It makes zero API calls.
- 🔴 **Missing Edit Flow**: There is a "Create New User" form, but the "Edit Role" button does nothing.
- 🔴 **Missing Tenant Data**: The tenant dropdown in the "Add User" form is missing. The user must be assigned to an existing tenant (fetched dynamically), but it currently only has a static "Role" dropdown.

---

## 3. Implementation Plan & UX/UI Redesign

This plan covers building the backend REST APIs and heavily redesigning the frontend to be more functional, dynamic, and visually appealing.

### Phase 1: Backend CRUD APIs (`src/routes/iam.rs`)
Implement the following routes, protected by an `Admin` role check in the JWT middleware:
1. `GET /api/v1/iam/users` — Returns a joined list of users and their assigned `tenant_id` and `role`.
2. `GET /api/v1/iam/tenants` — Returns a list of active tenants (to populate dropdowns).
3. `POST /api/v1/iam/users` — Creates a user, hashes the password via Argon2id, and links them to a tenant in `tenant_users`.
4. `PATCH /api/v1/iam/users/:id/role` — Updates the role in `tenant_users`.
5. `PATCH /api/v1/iam/users/:id/password` — Generates a new Argon2id hash for the user.
6. `DELETE /api/v1/iam/users/:id` — Hard deletes (or soft deletes) a user from the system.

*(Note: Ensure all endpoints perform validation, e.g., username uniqueness).*

### Phase 2: UX/UI Redesign (`page.tsx`)
We will modernize the User Management page to match the premium feel of the rest of the application.

1. **Dashboard Header & Metrics**:
   - Add summary cards at the top (similar to the Vector page): **Total Users**, **Active Tenants**, **Admins**, **Recent Logins**.
2. **Interactive Data Table**:
   - Replace the static table with a paginated, sortable DataTable.
   - Add a "Search by username" input.
   - Add a "Filter by Tenant" dropdown.
   - Use dynamic badges for Roles (e.g., Red for Admin, Blue for Editor, Gray for Viewer).
3. **Add/Edit User Modal (Drawer/Dialog) instead of Inline Form**:
   - Remove the clunky inline form that pushes the table down.
   - Use a sleek sliding **Sheet (Drawer)** from the right side, or a centered **Dialog (Modal)** for adding and editing users.
   - The form must dynamically fetch the list of `Tenants` from `GET /api/v1/iam/tenants` to populate a select box.
4. **Security Tooltips & Actions**:
   - Replace generic buttons with labeled Icon Buttons inside a Dropdown Menu (three dots `...`) for cleaner UI to prevent accidental clicks:
     - `Reset Password` (Opens a confirmation modal requiring the admin to type the new temporary password).
     - `Change Role/Tenant` (Opens the edit modal).
     - `Deactivate User` (Red destructive action with confirmation).

### Phase 3: Frontend Data Hook-up
1. Connect the redesigned UI to the new APIs via `authFetch` (to ensure the JWT token is sent).
2. Implement global state or `React Query` to ensure the table seamlessly reloads after a user is added, edited, or deleted without full page refreshes.

---

## 4. Verification Plan

1. **API Security Test**:
   - Attempt to call `POST /api/v1/iam/users` without a JWT token, or with a JWT token that has a `viewer` role. Ensure the backend returns `401 Unauthorized` or `403 Forbidden`.
   
2. **End-to-End Creation Test (Happy Path)**:
   - Use the Dashboard to click "Add User".
   - Fill in username= `test_editor`, Password= `temp123`, Tenant= `ragnarok_th`, Role= `editor`.
   - Submit. Verify a success toast appears and the table updates.
   - Run the login API endpoint manually with `test_editor` / `temp123` to verify the Argon2id hash worked and a JWT is issued.

3. **UX Observation**:
   - Verify the sliding Drawer/Modal feels responsive and does not shift the main layout unexpectedly when interacting.
   - Search for a user in the table to verify frontend filtering works.

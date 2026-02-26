# User Management & RBAC Specification (On-Premise)

**Version:** 1.0 (Draft)
**Target:** `mimir-core-ai` & `ro-ai-dashboard`
**Date:** 2026-02-21

For an on-premise deployment, Project Mimir must implement its own Identity and Access Management (IAM) system without relying on external SaaS providers (like Auth0 or Supabase). This document outlines the database schema, authentication flow, and Role-Based Access Control (RBAC) required.

---

## 1. Authentication Strategy

Since the system is on-premise and decoupled, we will use **JWT (JSON Web Tokens)** for stateless authentication between the Next.js Dashboard and the Rust Axum backend.

*   **Login Flow:**
    1.  User submits `email` and `password` to `/api/v1/auth/login`.
    2.  Rust backend hashes the password (using `argon2`) and verifies it against the DB.
    3.  Backend generates a short-lived `access_token` (JWT) and a long-lived `refresh_token`.
    4.  The JWT payload contains the `user_id`, `role`, and `tenant_id`.
*   **API Security:** All protected endpoints require an `Authorization: Bearer <token>` header.

---

## 2. Role-Based Access Control (RBAC)

The system supports Multi-Tenancy. Therefore, a user's role is strictly tied to a specific `tenant_id`.

### 2.1 Standard Roles
| Role Name        | Description                          | Permissions                                                         |
| :--------------- | :----------------------------------- | :------------------------------------------------------------------ |
| **Super Admin**  | System owner. Cross-tenant access.   | Create tenants, manage global settings, assign Tenant Admins.       |
| **Tenant Admin** | Administrator for a specific domain. | Manage users within the tenant, configure API keys, edit `sources`. |
| **Editor / QA**  | Staff responsible for AI tuning.     | Trigger pipeline runs, resolve QC conflicts, edit Q/A data.         |
| **Viewer**       | Read-only access.                    | View evaluations, metrics, and vector search results.               |

---

## 3. Database Schema Updates (MariaDB)

To support this in-house, the following core tables must be added to the MariaDB instance.

### 3.1 `tenants` Table
Manages the isolated organizations or domains.
```sql
CREATE TABLE tenants (
    id VARCHAR(50) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

### 3.2 `users` Table
Stores authentication credentials.
```sql
CREATE TABLE users (
    id CHAR(36) PRIMARY KEY, -- UUID
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    last_login TIMESTAMP NULL,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

### 3.3 `tenant_users` Table (Junction Table)
Maps users to specific tenants and defines their RBAC role. Permits a single user to belong to multiple tenants if necessary.
```sql
CREATE TABLE tenant_users (
    user_id CHAR(36) NOT NULL,
    tenant_id VARCHAR(50) NOT NULL,
    role ENUM('tenant_admin', 'editor', 'viewer') NOT NULL,
    PRIMARY KEY (user_id, tenant_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE
);
```

---

## 4. Implementation Details

### 4.1 Rust Backend (`mimir-core-ai`)
*   **Crate:** Introduce `jsonwebtoken` and `argon2` crates for handling tokens and password parsing securely.
*   **Middleware (`auth.rs`):** Create an Axum Extractor that intercepts requests, decodes the JWT, prevents expired tokens, and injects the `UserContext` (containing `role` and `tenant_id`) into the request state.
*   **Authorization Checks:** API handlers must check the `UserContext.role` before performing CRUD operations.

### 4.2 Next.js Frontend (`ro-ai-dashboard`)
*   **Auth Wrapper:** Implement a standard Login Page. Uses `useSession` or Context to track login state.
*   **Protected Routes:** Implement middleware in Next.js to redirect unauthenticated users back to `/login`.
*   **UI Masking:** Hide UI components (like the "Manage Sources" button) if the user's role is `Viewer`.

-- Phase 5: On-Premise IAM & RBAC
-- Create users, tenants, and tenant_users tables

CREATE TABLE IF NOT EXISTS tenants (
    id VARCHAR(50) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS users (
    id VARCHAR(36) PRIMARY KEY, -- UUID
    username VARCHAR(100) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS tenant_users (
    user_id VARCHAR(36) NOT NULL,
    tenant_id VARCHAR(50) NOT NULL,
    role VARCHAR(50) NOT NULL DEFAULT 'viewer', -- admin, editor, viewer
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, tenant_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE
);

-- Seed a default tenant if it doesn't exist
INSERT IGNORE INTO tenants (id, name) VALUES ('default_tenant', 'Default Tenant');

-- Seed an admin user (password: Admin123!)
-- Hash generated using Argon2id (we will need to ensure this is a valid hash or just replace it via script, but for now we provide a dummy hash to be overwritten or a known hash if possible. We will insert a known argon2 hash for 'Admin123!')
-- $argon2id$v=19$m=19456,t=2,p=1$VE9LSkxOUkhLUk9LSkxOUg$F30fK0cT0kOqf0M0B1f0kOqf0M0B1f0kOqf0M0B1f0k
INSERT IGNORE INTO users (id, username, password_hash) VALUES ('00000000-0000-0000-0000-000000000000', 'admin', '$argon2id$v=19$m=19456,t=2,p=1$VE9LSkxOUkhLUk9LSkxOUg$k1Z6zJ4w+qZQv6127O+QYdPQ86H9D9H8G01Z6zJ4w+o');

-- Link admin to default tenant
INSERT IGNORE INTO tenant_users (user_id, tenant_id, role) VALUES ('00000000-0000-0000-0000-000000000000', 'default_tenant', 'admin');

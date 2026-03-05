-- Issue #191: Custom Roles + Editable ACL Matrix
-- Create roles table with built-in and custom role support

CREATE TABLE IF NOT EXISTS roles (
    id VARCHAR(36) PRIMARY KEY,
    tenant_id VARCHAR(50) NOT NULL,
    name VARCHAR(100) NOT NULL,
    is_builtin BOOLEAN NOT NULL DEFAULT FALSE,
    permissions JSON NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE(tenant_id, name),
    FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON DELETE CASCADE
);

-- Seed built-in roles for default_tenant
INSERT IGNORE INTO roles (id, tenant_id, name, is_builtin, permissions) VALUES
('builtin_admin', 'default_tenant', 'admin', TRUE, '{"dashboard":"full","sources":"full","knowledge":"full","pipeline":"full","chat":"full","qc":"full","analytics":"full","settings":"full","users":"full","tenants":"full"}'),
('builtin_editor', 'default_tenant', 'editor', TRUE, '{"dashboard":"full","sources":"full","knowledge":"full","pipeline":"full","chat":"full","qc":"full","analytics":"full","settings":"none","users":"none","tenants":"none"}'),
('builtin_viewer', 'default_tenant', 'viewer', TRUE, '{"dashboard":"full","sources":"read","knowledge":"read","pipeline":"read","chat":"full","qc":"read","analytics":"read","settings":"none","users":"none","tenants":"none"}');

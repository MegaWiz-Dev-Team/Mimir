-- Migration: Add domain column to tenants table
-- Issue #76: Domain Connector Architecture (Medical vs Game)
-- Supports: 'game', 'medical', 'general' (default)

ALTER TABLE tenants ADD COLUMN domain VARCHAR(20) NOT NULL DEFAULT 'general';

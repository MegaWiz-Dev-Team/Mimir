"use client";

import { TenantConfig, LlmConfig, LlmSlot, VaultStatus, VaultSecretsResponse, Tenant, User, Role } from "@/lib/api";

/**
 * Shared props interface for all settings tab components.
 * Passed from the parent SettingsPage which owns all state.
 */
export interface SettingsTabProps {
    // Core state
    isLoading: boolean;
    isSaving: boolean;
    config: TenantConfig | null;
    setConfig: (config: TenantConfig) => void;
    currentTenantId: string | null;
    tenantName: string;
    setTenantName: (name: string) => void;
    handleSave: (e: React.FormEvent) => void;

    // Pipeline state
    chunkStrategy: string;
    setChunkStrategy: (v: string) => void;
    chunkSize: number;
    setChunkSize: (v: number) => void;
    chunkOverlap: number;
    setChunkOverlap: (v: number) => void;
    dedupThreshold: number;
    setDedupThreshold: (v: number) => void;
    updateTenantConfigFn: (tenantId: string, config: Partial<TenantConfig>) => Promise<void>;

    // Search state
    embeddingModel: string;
    setEmbeddingModel: (v: string) => void;
    topK: number;
    setTopK: (v: number) => void;
    similarityThreshold: number;
    setSimilarityThreshold: (v: number) => void;
    searchMode: string;
    setSearchMode: (v: string) => void;

    // Vault state
    vaultStatus: VaultStatus | null;
    vaultSecrets: VaultSecretsResponse | null;
    isVaultLoading: boolean;
    refreshVaultData: () => void;
    rotateDialog: { open: boolean; key: string };
    setRotateDialog: (v: { open: boolean; key: string }) => void;
    rotateValue: string;
    setRotateValue: (v: string) => void;
    isRotating: boolean;
    rotatingKey: string | null;
    handleRotateSecret: () => void;

    // Roles state
    roles: Role[];
    isRolesLoading: boolean;
    loadRoles: () => void;
    addRoleDialog: boolean;
    setAddRoleDialog: (v: boolean) => void;
    newRoleName: string;
    setNewRoleName: (v: string) => void;
    deleteRoleDialog: { open: boolean; role: Role | null };
    setDeleteRoleDialog: (v: { open: boolean; role: Role | null }) => void;
    hasPendingChanges: boolean;
    isSavingRoles: boolean;
    handleSaveRoles: () => void;
    handleAddRole: () => void;
    handleDeleteRole: () => void;
    togglePermission: (roleId: string, resource: string, currentLevel: string) => void;
    getEffectivePermission: (role: Role, resource: string) => string;
    PERMISSION_RESOURCES: string[];
    PERMISSION_LEVELS: string[];
    PERMISSION_ICONS: Record<string, string>;

    // Tenants & Users state
    allTenants: Tenant[];
    allUsers: User[];
    showCreateTenantDialog: boolean;
    setShowCreateTenantDialog: (v: boolean) => void;
    showCreateUserDialog: boolean;
    setShowCreateUserDialog: (v: boolean) => void;
    loadData: () => void;
}

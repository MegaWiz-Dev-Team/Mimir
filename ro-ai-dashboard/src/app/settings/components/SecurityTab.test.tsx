import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import { SecurityTab } from './SecurityTab';

describe('SecurityTab', () => {
    const mockSetConfig = jest.fn();

    const mockConfig = {
        tenant_id: "t1",
        llm_config: { heimdall_url: "http://localhost:8080", heimdall_api_key: "sk-heimdall" },
        provider_api_keys: { openai: "sk-openai", google: "aiza-google" }
    };

    const baseProps: any = {
        config: mockConfig,
        setConfig: mockSetConfig,
        vaultStatus: null,
        vaultSecrets: null,
        isVaultLoading: false,
        refreshVaultData: jest.fn(),
        rotateDialog: { open: false, key: "" },
        setRotateDialog: jest.fn(),
        rotateValue: "",
        setRotateValue: jest.fn(),
        isRotating: false,
        rotatingKey: null,
        handleRotateSecret: jest.fn(),
        roles: [],
        isRolesLoading: false,
        loadRoles: jest.fn(),
        addRoleDialog: false,
        setAddRoleDialog: jest.fn(),
        newRoleName: "",
        setNewRoleName: jest.fn(),
        deleteRoleDialog: { open: false, role: null },
        setDeleteRoleDialog: jest.fn(),
        hasPendingChanges: false,
        isSavingRoles: false,
        handleSaveRoles: jest.fn(),
        handleAddRole: jest.fn(),
        handleDeleteRole: jest.fn(),
        togglePermission: jest.fn(),
        getEffectivePermission: jest.fn(),
        PERMISSION_RESOURCES: [],
        PERMISSION_LEVELS: [],
        PERMISSION_ICONS: {},
        allTenants: [],
        allUsers: [],
        showCreateTenantDialog: false,
        setShowCreateTenantDialog: jest.fn(),
        showCreateUserDialog: false,
        setShowCreateUserDialog: jest.fn(),
        loadData: jest.fn()
    };

    it('renders provider credentials form', () => {
        render(<SecurityTab {...baseProps} />);
        expect(screen.getByText('External Provider Credentials')).toBeInTheDocument();
        const urlInput = screen.getByDisplayValue('http://localhost:8080');
        expect(urlInput).toBeInTheDocument();
    });

    it('calls setConfig when Heimdall URL is changed', () => {
        render(<SecurityTab {...baseProps} />);
        const urlInput = screen.getByDisplayValue('http://localhost:8080');
        fireEvent.change(urlInput, { target: { value: 'http://new-url' } });
        expect(mockSetConfig).toHaveBeenCalled();
    });
});

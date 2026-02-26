import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import UsersPage from './page';
import * as api from '@/lib/api';

// Mock the API module
jest.mock('@/lib/api', () => ({
    fetchUsers: jest.fn(),
    fetchTenants: jest.fn(),
    createUser: jest.fn(),
    updateUserRole: jest.fn(),
    updateUserPassword: jest.fn(),
    deleteUser: jest.fn(),
}));

const mockUsers = [
    { id: 1, username: 'admin', tenant_id: 'default_tenant', role: 'admin', created_at: '2026-02-20T00:00:00Z' },
    { id: 2, username: 'viewer1', tenant_id: 'default_tenant', role: 'viewer', created_at: '2026-02-21T00:00:00Z' },
];

const mockTenants = [
    { id: 'default_tenant', name: 'Default Tenant', created_at: '2026-02-20T00:00:00Z' },
];

describe('UsersPage', () => {
    beforeEach(() => {
        jest.clearAllMocks();
    });

    it('renders user data when API succeeds', async () => {
        (api.fetchUsers as jest.Mock).mockResolvedValue(mockUsers);
        (api.fetchTenants as jest.Mock).mockResolvedValue(mockTenants);

        render(<UsersPage />);

        expect(screen.getByText('User Management')).toBeInTheDocument();
        expect(screen.getByText('Loading users...')).toBeInTheDocument();

        await waitFor(() => {
            expect(screen.getByText('admin')).toBeInTheDocument();
            expect(screen.getByText('viewer1')).toBeInTheDocument();
        });
    });

    it('shows inline error banner instead of alert when API fails', async () => {
        // Mock alert to verify it is NOT called
        const alertSpy = jest.spyOn(window, 'alert').mockImplementation(() => { });

        (api.fetchUsers as jest.Mock).mockRejectedValue(new TypeError('Failed to fetch'));
        (api.fetchTenants as jest.Mock).mockRejectedValue(new TypeError('Failed to fetch'));

        render(<UsersPage />);

        await waitFor(() => {
            // Should show inline error, not call alert()
            expect(screen.getByText(/Unable to load user data/i)).toBeInTheDocument();
        });

        // alert() should NOT be called
        expect(alertSpy).not.toHaveBeenCalled();
        alertSpy.mockRestore();
    });

    it('shows empty state when no users exist', async () => {
        (api.fetchUsers as jest.Mock).mockResolvedValue([]);
        (api.fetchTenants as jest.Mock).mockResolvedValue(mockTenants);

        render(<UsersPage />);

        await waitFor(() => {
            expect(screen.getByText('No users found.')).toBeInTheDocument();
        });
    });
});

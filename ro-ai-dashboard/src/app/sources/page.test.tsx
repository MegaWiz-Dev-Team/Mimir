import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import SourcesPage from './page';
import * as api from '@/lib/api';

// Mock the API module
jest.mock('@/lib/api', () => ({
    fetchSources: jest.fn(),
    createSource: jest.fn(),
    deleteSource: jest.fn(),
    syncSource: jest.fn(),
    updateSource: jest.fn(),
}));

describe('SourcesPage', () => {
    const mockSources = [
        {
            id: 1,
            tenant_id: 'tenant-1',
            name: 'Wiki Guide',
            source_type: 'web',
            config_json: { url: 'https://wiki.example.com' },
            schedule: 'Manual',
            last_sync_status: 'COMPLETED',
            last_sync_at: null,
            created_at: '2026-02-25T00:00:00Z',
            updated_at: '2026-02-25T00:00:00Z',
        }
    ];

    beforeEach(() => {
        jest.clearAllMocks();
        // Setup default mock implementation
        (api.fetchSources as jest.Mock).mockResolvedValue(mockSources);
    });

    it('renders the sources page and loads data', async () => {
        render(<SourcesPage />);

        expect(screen.getByText('Data Ingress Sources')).toBeInTheDocument();
        expect(screen.getByText('Loading sources...')).toBeInTheDocument();

        // Wait for data to load
        await waitFor(() => {
            expect(screen.getByText('Wiki Guide')).toBeInTheDocument();
        });
        expect(api.fetchSources).toHaveBeenCalledTimes(1);
    });

    it('opens the configuration dialog when the configure button is clicked', async () => {
        render(<SourcesPage />);

        // Wait for data to load
        await waitFor(() => {
            expect(screen.getByText('Wiki Guide')).toBeInTheDocument();
        });

        // Find and click the configure button (gear icon)
        // We'll select it by its title attribute
        const configureButton = screen.getByTitle('Configure');
        fireEvent.click(configureButton);

        // Verify that the dialog opens with the correct title
        await waitFor(() => {
            expect(screen.getByText('Configure Data Source')).toBeInTheDocument();
        });

        // Verify the form fields are populated with the source's data
        const nameInput = screen.getByLabelText('Source Name');
        expect(nameInput).toHaveValue('Wiki Guide');

        const urlInput = screen.getByLabelText('Target URL');
        expect(urlInput).toHaveValue('https://wiki.example.com');

        const typeInput = screen.getByLabelText('Source Type');
        expect(typeInput).toHaveValue('web');

        const scheduleInput = screen.getByLabelText('Execution Schedule');
        expect(scheduleInput).toHaveValue('Manual');
    });

    it('calls updateSource and reloads data when configuration is saved', async () => {
        (api.updateSource as jest.Mock).mockResolvedValue({ ...mockSources[0], name: 'Updated Wiki' });

        render(<SourcesPage />);

        // Wait for data to load
        await waitFor(() => {
            expect(screen.getByText('Wiki Guide')).toBeInTheDocument();
        });

        const configureButton = screen.getByTitle('Configure');
        fireEvent.click(configureButton);

        await waitFor(() => {
            expect(screen.getByText('Configure Data Source')).toBeInTheDocument();
        });

        // Change the name
        const nameInput = screen.getByLabelText('Source Name');
        fireEvent.change(nameInput, { target: { value: 'Updated Wiki' } });

        // Click Save Changes
        const saveButton = screen.getByText('Save Changes');
        fireEvent.click(saveButton);

        // Verify updateSource was called correctly
        await waitFor(() => {
            expect(api.updateSource).toHaveBeenCalledWith(1, expect.objectContaining({
                name: 'Updated Wiki',
                config_json: { url: 'https://wiki.example.com' },
                schedule: 'Manual'
            }));

            // Should reload sources after saving
            expect(api.fetchSources).toHaveBeenCalledTimes(2);
        });
    });
});

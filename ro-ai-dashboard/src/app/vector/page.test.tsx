import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import VectorPage from './page';
import * as api from '@/lib/api';

// Mock the API module
jest.mock('@/lib/api', () => ({
    fetchVectorStats: jest.fn(),
    triggerIndexing: jest.fn(),
    searchVectors: jest.fn(),
}));

const mockStats = {
    database: { indexed_qa: 150 },
    qdrant: {
        result: {
            vectors_count: 150,
            points_count: 150,
            config: { params: { vectors: { size: 768, distance: 'Cosine' } } },
        }
    },
};

const mockSearchResults = {
    result: [
        {
            id: 'vec-1',
            score: 0.92,
            payload: {
                question: 'What is Prontera?',
                answer: 'The capital city of Rune-Midgarts Kingdom.',
                source: 'prontera_guide.md',
                chunk: 3,
            },
        },
    ],
};

describe('VectorPage - Sprint 7 Phase 4: Traceability Badges', () => {
    beforeEach(() => {
        jest.clearAllMocks();
        (api.fetchVectorStats as jest.Mock).mockResolvedValue(mockStats);
    });

    it('renders vector stats correctly', async () => {
        render(<VectorPage />);

        await waitFor(() => {
            expect(api.fetchVectorStats).toHaveBeenCalledTimes(1);
        });
    });

    it('displays traceability badges in search results', async () => {
        (api.searchVectors as jest.Mock).mockResolvedValue(mockSearchResults);

        render(<VectorPage />);

        // Wait for stats to load first
        await waitFor(() => {
            expect(api.fetchVectorStats).toHaveBeenCalled();
        });

        // Find the search input and perform a search
        const searchInput = screen.getByPlaceholderText('Type a game question (e.g., What is Moonstone?)');
        fireEvent.change(searchInput, { target: { value: 'Prontera' } });

        // Submit the search form
        const form = searchInput.closest('form')!;
        fireEvent.submit(form);

        // Wait for searchVectors to be called
        await waitFor(() => {
            expect(api.searchVectors).toHaveBeenCalledWith('Prontera');
        });

        // Wait for traceability badges to render
        await waitFor(() => {
            // Source document badge
            const sourceTexts = screen.getAllByText('prontera_guide.md');
            expect(sourceTexts.length).toBeGreaterThan(0);
        });

        // Approval status badge
        expect(screen.getByText('System Approved')).toBeInTheDocument();
    });
});

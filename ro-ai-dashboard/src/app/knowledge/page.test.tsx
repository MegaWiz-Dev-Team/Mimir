import { render, screen, waitFor, fireEvent, act } from '@testing-library/react';
import KnowledgePage from './page';
import * as api from '@/lib/api';
import Cookies from 'js-cookie';

// Mock next/link
jest.mock('next/link', () => {
    return ({ children, href }: { children: React.ReactNode; href: string }) => (
        <a href={href}>{children}</a>
    );
});

// Mock js-cookie
jest.mock('js-cookie', () => ({
    get: jest.fn(),
}));

// Mock the API module
jest.mock('@/lib/api', () => ({
    fetchChunks: jest.fn(),
    fetchSources: jest.fn(),
    generateQaForChunks: jest.fn(),
    syncAllSources: jest.fn(),
}));

const mockChunks: api.ChunkItem[] = [
    { id: 1, source_id: 10, source_name: 'adk-rush', chunk_index: 155, content: 'Role-based permissions and audit logging', token_count: 102, metadata_json: null, created_at: '2026-03-01T00:00:00Z' },
    { id: 2, source_id: 10, source_name: 'adk-rush', chunk_index: 154, content: 'Logging, tracing, and monitoring', token_count: 115, metadata_json: null, created_at: '2026-03-01T00:00:00Z' },
    { id: 3, source_id: 11, source_name: 'docs-web', chunk_index: 153, content: 'Managing conversation state with prefixes', token_count: 104, metadata_json: null, created_at: '2026-03-01T00:00:00Z' },
];

const mockChunkResponse: api.ChunkListResponse = {
    chunks: mockChunks,
    total: 3,
    total_tokens: 321,
    page: 1,
    per_page: 20,
};

describe('KnowledgePage — Selective Chunk QA (Issue #179)', () => {
    beforeEach(() => {
        jest.clearAllMocks();
        (Cookies.get as jest.Mock).mockReturnValue('test-token');
        (api.fetchChunks as jest.Mock).mockResolvedValue(mockChunkResponse);
        (api.fetchSources as jest.Mock).mockResolvedValue([
            { id: 10, name: 'adk-rush' },
            { id: 11, name: 'docs-web' },
        ]);
    });

    it('renders chunk table with checkboxes', async () => {
        await act(async () => {
            render(<KnowledgePage />);
        });

        await waitFor(() => {
            expect(api.fetchChunks).toHaveBeenCalled();
        });

        // Each chunk row should have a checkbox
        const checkboxes = screen.getAllByRole('checkbox');
        // +1 for "select all" header checkbox
        expect(checkboxes.length).toBeGreaterThanOrEqual(mockChunks.length);
    });

    it('shows floating action bar when chunks are selected', async () => {
        await act(async () => {
            render(<KnowledgePage />);
        });

        await waitFor(() => {
            expect(api.fetchChunks).toHaveBeenCalled();
        });

        // Action bar should not be visible initially
        expect(screen.queryByText(/chunks selected/i)).not.toBeInTheDocument();

        // Click first chunk checkbox
        const checkboxes = screen.getAllByRole('checkbox');
        await act(async () => {
            fireEvent.click(checkboxes[1]); // [0] is select-all
        });

        // Action bar should now appear
        expect(screen.getByText(/1 chunk.* selected/i)).toBeInTheDocument();
        expect(screen.getByRole('button', { name: /generate qa/i })).toBeInTheDocument();
    });

    it('select all checkbox selects all chunk checkboxes', async () => {
        await act(async () => {
            render(<KnowledgePage />);
        });

        await waitFor(() => {
            expect(api.fetchChunks).toHaveBeenCalled();
        });

        const checkboxes = screen.getAllByRole('checkbox');
        const selectAll = checkboxes[0]; // First checkbox is select-all

        await act(async () => {
            fireEvent.click(selectAll);
        });

        // All checkboxes should be checked
        checkboxes.forEach(cb => {
            expect(cb).toBeChecked();
        });

        // Action bar should show total count
        expect(screen.getByText(/3 chunks selected/i)).toBeInTheDocument();
    });

    it('Generate QA button calls generateQaForChunks with selected IDs', async () => {
        (api.generateQaForChunks as jest.Mock).mockResolvedValue({
            success: true,
            message: 'QA generation started for 2 chunks',
            chunk_count: 2,
        });

        await act(async () => {
            render(<KnowledgePage />);
        });

        await waitFor(() => {
            expect(api.fetchChunks).toHaveBeenCalled();
        });

        // Select first two chunks
        const checkboxes = screen.getAllByRole('checkbox');
        await act(async () => {
            fireEvent.click(checkboxes[1]); // chunk id=1
            fireEvent.click(checkboxes[2]); // chunk id=2
        });

        // Click Generate QA in the action bar
        const generateBtn = screen.getByRole('button', { name: /generate qa/i });
        await act(async () => {
            fireEvent.click(generateBtn);
        });

        await waitFor(() => {
            expect(api.generateQaForChunks).toHaveBeenCalledWith([1, 2]);
        });
    });

    it('deselecting all hides the floating action bar', async () => {
        await act(async () => {
            render(<KnowledgePage />);
        });

        await waitFor(() => {
            expect(api.fetchChunks).toHaveBeenCalled();
        });

        const checkboxes = screen.getAllByRole('checkbox');

        // Select then deselect
        await act(async () => {
            fireEvent.click(checkboxes[1]);
        });
        expect(screen.getByText(/1 chunk.* selected/i)).toBeInTheDocument();

        await act(async () => {
            fireEvent.click(checkboxes[1]);
        });
        expect(screen.queryByText(/chunks selected/i)).not.toBeInTheDocument();
    });

    it('displays total tokens from API (not page sum)', async () => {
        await act(async () => {
            render(<KnowledgePage />);
        });

        await waitFor(() => {
            expect(api.fetchChunks).toHaveBeenCalled();
        });

        // Should show 321 tokens (from API total_tokens), not 102+115+104=321
        expect(screen.getByText('321')).toBeInTheDocument();
    });
});

import { render, screen, waitFor } from '@testing-library/react';
import { PipelineStatusBar } from './pipeline-status-bar';
import * as api from '@/lib/api';
import Cookies from 'js-cookie';

// Mock next/link
jest.mock('next/link', () => {
    return ({ children, href }: { children: React.ReactNode; href: string }) => (
        <a href={href}>{children}</a>
    );
});

// Mock next/navigation
const mockPathname = jest.fn();
jest.mock('next/navigation', () => ({
    usePathname: () => mockPathname(),
}));

// Mock js-cookie
jest.mock('js-cookie', () => ({
    get: jest.fn(),
}));

// Mock the API calls
jest.mock('@/lib/api', () => ({
    fetchSources: jest.fn().mockResolvedValue([]),
    fetchRuns: jest.fn().mockResolvedValue([]),
    fetchQcClusters: jest.fn().mockResolvedValue({ clusters: [] }),
    fetchVectorStats: jest.fn().mockResolvedValue({ database: { indexed_qa: 0 } }),
}));

describe('PipelineStatusBar', () => {
    beforeEach(() => {
        jest.clearAllMocks();
        // Default: authenticated, on dashboard
        mockPathname.mockReturnValue('/');
        (Cookies.get as jest.Mock).mockReturnValue('valid-token');
    });

    it('renders correctly when authenticated', async () => {
        render(<PipelineStatusBar />);

        expect(screen.getByText(/Global Pipeline Status:/i)).toBeInTheDocument();

        await waitFor(() => {
            expect(api.fetchSources).toHaveBeenCalledTimes(1);
            expect(api.fetchRuns).toHaveBeenCalledTimes(1);
            expect(api.fetchQcClusters).toHaveBeenCalledTimes(1);
            expect(api.fetchVectorStats).toHaveBeenCalledTimes(1);
        });

        expect(screen.getByText(/Sources/i)).toBeInTheDocument();
        expect(screen.getByText(/Generating/i)).toBeInTheDocument();
        expect(screen.getByText(/Pending QC/i)).toBeInTheDocument();
        expect(screen.getByText(/Vectorized/i)).toBeInTheDocument();
    });

    it('does NOT render on the /login page', () => {
        mockPathname.mockReturnValue('/login');

        const { container } = render(<PipelineStatusBar />);

        expect(container.innerHTML).toBe('');
        expect(screen.queryByText(/Global Pipeline Status/i)).not.toBeInTheDocument();
    });

    it('does NOT render when access_token cookie is missing', () => {
        (Cookies.get as jest.Mock).mockReturnValue(undefined);

        const { container } = render(<PipelineStatusBar />);

        expect(container.innerHTML).toBe('');
        expect(screen.queryByText(/Global Pipeline Status/i)).not.toBeInTheDocument();
    });

    it('calculates counts correctly with mocked active items', async () => {
        (api.fetchSources as jest.Mock).mockResolvedValue([
            { id: 1, last_sync_status: 'PENDING' },
            { id: 2, last_sync_status: 'COMPLETED' }
        ]);
        (api.fetchRuns as jest.Mock).mockResolvedValue([
            { id: "run1", status: "RUNNING" },
            { id: "run2", status: "COMPLETED" }
        ]);
        (api.fetchQcClusters as jest.Mock).mockResolvedValue({
            clusters: [
                { id: "c1", status: "PENDING" },
                { id: "c2", status: "RESOLVED_A" }
            ]
        });
        (api.fetchVectorStats as jest.Mock).mockResolvedValue({
            database: { indexed_qa: 42 }
        });

        render(<PipelineStatusBar />);

        await waitFor(() => {
            expect(api.fetchSources).toHaveBeenCalledTimes(1);
        });

        expect(screen.getByText(/Global Pipeline Status/i)).toBeInTheDocument();
    });
});

import { render, screen, waitFor, act } from '@testing-library/react';
import KbBrowserPage from './page';
import * as api from '@/lib/api';

let mockKbId = 'clinicalkb';
jest.mock('next/navigation', () => ({
    useParams: () => ({ kbId: mockKbId }),
}));

jest.mock('next/link', () => {
    return ({ children, href }: { children: React.ReactNode; href: string }) => <a href={href}>{children}</a>;
});

jest.mock('@/lib/api', () => ({
    API_BASE_URL: 'http://api.test/api/v1',
    authFetch: jest.fn(),
    searchPrimekgEntity: jest.fn(),
    fetchPrimekgNeighbors: jest.fn(),
    searchClinicalkbEntity: jest.fn(),
    fetchClinicalkbNeighbors: jest.fn(),
}));

// The real component needs WebGL; the page only has to pick it.
jest.mock('./KbGraph3D', () => ({ kbId }: { kbId: string }) => <div>graph browser: {kbId}</div>);

describe('KbBrowserPage — graph KBs get the 3D browser, tables get rows', () => {
    beforeEach(() => jest.clearAllMocks());

    it.each(['clinicalkb', 'primekg'])('renders the graph browser for %s without asking for table rows', async (kbId) => {
        mockKbId = kbId;
        await act(async () => {
            render(<KbBrowserPage />);
        });
        expect(screen.getByText(`graph browser: ${kbId}`)).toBeInTheDocument();
        expect(api.authFetch).not.toHaveBeenCalled();
    });

    it('fetches table rows for a table KB', async () => {
        mockKbId = 'rxnorm';
        (api.authFetch as jest.Mock).mockResolvedValue({
            ok: true,
            json: async () => ({
                kb_id: 'rxnorm', total: 1, page: 1, per_page: 20, filters: {},
                columns: [{ name: 'str', label: 'Name', kind: 'string' }],
                items: [{ str: 'metformin' }],
            }),
        });
        await act(async () => {
            render(<KbBrowserPage />);
        });
        await waitFor(() => expect(screen.getByText('metformin')).toBeInTheDocument());
        expect((api.authFetch as jest.Mock).mock.calls[0][0]).toContain('/knowledge/shared/rxnorm/items');
    });

    it('shows the API error and hint as a sentence, not raw JSON', async () => {
        mockKbId = 'pubmed';
        (api.authFetch as jest.Mock).mockResolvedValue({
            ok: false,
            status: 501,
            text: async () => JSON.stringify({ error: 'pubmed has no table browser', hint: 'Use the Cross-KB Search' }),
        });
        await act(async () => {
            render(<KbBrowserPage />);
        });
        await waitFor(() =>
            expect(screen.getByText('HTTP 501: pubmed has no table browser. Use the Cross-KB Search')).toBeInTheDocument(),
        );
    });

    it('does not mistake an Object.prototype key for a graph KB', async () => {
        mockKbId = 'constructor';
        (api.authFetch as jest.Mock).mockResolvedValue({
            ok: false, status: 404, text: async () => '{"error":"Unknown kb_id: constructor"}',
        });
        await act(async () => {
            render(<KbBrowserPage />);
        });
        expect(screen.queryByText(/graph browser/)).not.toBeInTheDocument();
        await waitFor(() => expect(screen.getByText('HTTP 404: Unknown kb_id: constructor')).toBeInTheDocument());
    });
});

import { render, screen, waitFor, act, fireEvent } from '@testing-library/react';
import UnifiedKnowledgeSearchPage from './page';
import * as api from '@/lib/api';

jest.mock('next/link', () => {
    return ({ children, href, className }: { children: React.ReactNode; href: string; className?: string }) => (
        <a href={href} className={className}>{children}</a>
    );
});

jest.mock('@/lib/api', () => ({
    API_BASE_URL: 'http://api.test/api/v1',
    authFetch: jest.fn(),
}));

function hit(kb_id: string, kb_name: string, browser?: string) {
    return { kb_id, kb_name, browser, items: [], count: 0, latency_ms: 1 };
}

describe('UnifiedKnowledgeSearchPage — "Open in browser" only where a detail page exists', () => {
    beforeEach(() => jest.clearAllMocks());

    it('shows the link for table/graph KBs and hides it for search-only or derived views', async () => {
        (api.authFetch as jest.Mock).mockResolvedValue({
            ok: true,
            json: async () => ({
                q: 'metformin', k: 3, total_ms: 5,
                results: [
                    hit('rxnorm', 'RxNorm', 'table'),
                    hit('primekg', 'PrimeKG', 'graph'),
                    hit('pubmed', 'PubMed', 'search'),
                    hit('symptoms', 'Symptom → Disease', 'none'),
                ],
            }),
        });

        render(<UnifiedKnowledgeSearchPage />);
        await act(async () => {
            fireEvent.change(screen.getByPlaceholderText(/metformin/), { target: { value: 'metformin' } });
        });
        await waitFor(() => expect(screen.getByText('PubMed')).toBeInTheDocument(), { timeout: 2000 });

        const hrefs = screen
            .getAllByText('Open in browser →')
            .map((el) => el.closest('a')?.getAttribute('href'));
        expect(hrefs).toEqual([
            '/knowledge/shared/rxnorm?q=metformin',
            '/knowledge/shared/primekg?q=metformin',
        ]);
    });
});

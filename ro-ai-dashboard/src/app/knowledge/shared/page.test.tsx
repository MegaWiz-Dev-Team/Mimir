import { render, screen, waitFor, act } from '@testing-library/react';
import SharedKnowledgePage from './page';
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

function kb(id: string, name: string, browser?: string) {
    return {
        id, name, browser,
        description: `${name} description`,
        kind: 'terminology', stores: ['mariadb'], source_url: '',
        maintainer: 'm', region: 'INTL', languages: ['en'], vintage_year: null,
        license: 'l', fhir_binding: null, update_cadence: 'c', schema_version: 's',
        notes: null, counts: { rows: 1 }, source_version: null, status: 'active',
        last_local_refresh: null,
    };
}

async function renderWith(items: ReturnType<typeof kb>[]) {
    (api.authFetch as jest.Mock).mockResolvedValue({
        ok: true,
        json: async () => ({ items, count: items.length }),
    });
    await act(async () => {
        render(<SharedKnowledgePage />);
    });
    await waitFor(() => expect(screen.getByText(items[0].name)).toBeInTheDocument());
}

/** The <a> wrapping a card, or null when the card is not a link. */
function cardLink(name: string): HTMLAnchorElement | null {
    return screen.getByText(name).closest('a');
}

describe('SharedKnowledgePage — cards link only where the backend has a browser', () => {
    beforeEach(() => jest.clearAllMocks());

    it('links table and graph KBs to their detail page', async () => {
        await renderWith([kb('rxnorm', 'RxNorm', 'table'), kb('primekg', 'PrimeKG', 'graph')]);
        expect(cardLink('RxNorm')).toHaveAttribute('href', '/knowledge/shared/rxnorm');
        expect(cardLink('PrimeKG')).toHaveAttribute('href', '/knowledge/shared/primekg');
    });

    it('sends a search-only KB to the cross-KB search page', async () => {
        await renderWith([kb('pubmed', 'PubMed', 'search')]);
        expect(cardLink('PubMed')).toHaveAttribute('href', '/knowledge/search');
        expect(screen.getByText('Search only')).toBeInTheDocument();
    });

    it('renders a KB with no browser as a plain card, never a link to a 404', async () => {
        await renderWith([kb('clinicalkb', 'ClinicalKB', 'none')]);
        expect(cardLink('ClinicalKB')).toBeNull();
        expect(screen.getByText('Browser not available')).toBeInTheDocument();
    });

    it('treats a missing browser field (older API) as no browser', async () => {
        await renderWith([kb('new-kb', 'Brand New KB')]);
        expect(cardLink('Brand New KB')).toBeNull();
    });
});

import { fetchQcClusters, resolveQcCluster, triggerQcGeneration, fetchQcStatus } from './api';
import { discoverHierarchy, importPages, fetchLlmUsage, fetchLlmUsageSummary } from './api';
import { fetchSources, createSource, updateSource, deleteSource, syncSource } from './api';
import Cookies from 'js-cookie';

// Mock js-cookie
jest.mock('js-cookie', () => ({
    get: jest.fn(),
    set: jest.fn(),
    remove: jest.fn(),
}));

describe('Quality Control API client functionality', () => {
    const originalFetch = global.fetch;

    beforeEach(() => {
        global.fetch = jest.fn();
        (Cookies.get as jest.Mock).mockReturnValue('mock-tenant-id');
        // Ensure the ENV variable is set
        process.env.NEXT_PUBLIC_API_URL = 'http://localhost:8080';
    });

    afterEach(() => {
        global.fetch = originalFetch;
        jest.resetAllMocks();
    });

    it('fetchQcClusters should call correct endpoint including status parameter', async () => {
        (global.fetch as jest.Mock).mockResolvedValue({
            ok: true,
            json: async () => [{ id: '1', topic: 'Mock Topic' }]
        });

        const result = await fetchQcClusters('PENDING');

        expect(global.fetch).toHaveBeenCalledWith(
            'http://localhost:8080/api/v1/qc/clusters?status=PENDING',
            expect.objectContaining({
                headers: {
                    'Authorization': 'Bearer mock-tenant-id',
                    'X-Tenant-Id': 'mock-tenant-id',
                }
            })
        );
        expect(result).toEqual([{ id: '1', topic: 'Mock Topic' }]);
    });

    it('fetchQcClusters should omit status parameter if not provided', async () => {
        (global.fetch as jest.Mock).mockResolvedValue({
            ok: true,
            json: async () => []
        });

        await fetchQcClusters();

        expect(global.fetch).toHaveBeenCalledWith(
            'http://localhost:8080/api/v1/qc/clusters',
            expect.any(Object)
        );
    });

    it('resolveQcCluster should call the respective endpoint with payload', async () => {
        (global.fetch as jest.Mock).mockResolvedValue({
            ok: true,
            json: async () => ({ success: true, message: 'Resolved' })
        });

        const result = await resolveQcCluster('mock-cluster', 'MERGE', 'Golden rule');

        expect(global.fetch).toHaveBeenCalledWith(
            'http://localhost:8080/api/v1/qc/resolve/mock-cluster',
            expect.objectContaining({
                method: 'POST',
                body: JSON.stringify({ resolution_type: 'MERGE', golden_answer: 'Golden rule' }),
            })
        );
        expect(result).toBe(true);
    });

    it('triggerQcGeneration should throw an Error on response !ok', async () => {
        (global.fetch as jest.Mock).mockResolvedValue({
            ok: false,
            json: async () => { throw new Error('Failed to trigger QC generation'); }
        });

        await expect(triggerQcGeneration()).rejects.toThrow('Failed to trigger QC generation');
    });

    it('fetchQcStatus should fetch status and return correct object', async () => {
        (global.fetch as jest.Mock).mockResolvedValue({
            ok: true,
            json: async () => ({ is_generating: true })
        });

        const result = await fetchQcStatus();

        expect(global.fetch).toHaveBeenCalledWith(
            'http://localhost:8080/api/v1/qc/status',
            expect.objectContaining({
                headers: {
                    'Authorization': 'Bearer mock-tenant-id',
                    'X-Tenant-Id': 'mock-tenant-id',
                }
            })
        );
        expect(result).toEqual({ is_generating: true });
    });

    it('fetchQcStatus should throw an Error when failing to fetch', async () => {
        (global.fetch as jest.Mock).mockResolvedValue({
            ok: false,
            statusText: 'Internal Server Error'
        });

        await expect(fetchQcStatus()).rejects.toThrow('Failed to fetch QC status: Internal Server Error');
    });

    it('fetchQcClusters should throw an Error on response !ok', async () => {
        (global.fetch as jest.Mock).mockResolvedValue({
            ok: false,
            statusText: 'Internal Server Error'
        });

        await expect(fetchQcClusters()).rejects.toThrow('Failed to fetch QC clusters');
    });
});

import { fetchSources, createSource, deleteSource, syncSource } from './api';

describe('Data Sources API client functionality', () => {
    const originalFetch = global.fetch;

    beforeEach(() => {
        global.fetch = jest.fn();
        (Cookies.get as jest.Mock).mockReturnValue('mock-tenant-id');
        process.env.NEXT_PUBLIC_API_URL = 'http://localhost:8080';
    });

    afterEach(() => {
        global.fetch = originalFetch;
        jest.resetAllMocks();
    });

    it('fetchSources should call correct endpoint and return sources', async () => {
        const mockSources = [{ id: 1, name: 'Test Source', source_type: 'web', url: 'https://test.com', status: 'active', is_active: true }];
        (global.fetch as jest.Mock).mockResolvedValue({
            ok: true,
            json: async () => mockSources
        });

        const result = await fetchSources();

        expect(global.fetch).toHaveBeenCalledWith(
            'http://localhost:8080/api/v1/sources',
            expect.objectContaining({
                cache: 'no-store',
                headers: {
                    'Authorization': 'Bearer mock-tenant-id',
                    'X-Tenant-Id': 'mock-tenant-id',
                }
            })
        );
        expect(result).toEqual(mockSources);
    });

    it('createSource should call POST correct endpoint with payload', async () => {
        const payload = { name: 'New Source', source_type: 'web' as const, url: 'https://new.com' };
        const mockResponse = { id: 2, ...payload, status: 'active', is_active: true };

        (global.fetch as jest.Mock).mockResolvedValue({
            ok: true,
            json: async () => mockResponse
        });

        const result = await createSource(payload);

        expect(global.fetch).toHaveBeenCalledWith(
            'http://localhost:8080/api/v1/sources',
            expect.objectContaining({
                method: 'POST',
                headers: expect.objectContaining({ 'Content-Type': 'application/json' }),
                body: JSON.stringify(payload)
            })
        );
        expect(result).toEqual(mockResponse);
    });

    it('deleteSource should call DELETE endpoint', async () => {
        (global.fetch as jest.Mock).mockResolvedValue({
            ok: true,
            json: async () => ({ success: true })
        });

        await deleteSource(123);

        expect(global.fetch).toHaveBeenCalledWith(
            'http://localhost:8080/api/v1/sources/123',
            expect.objectContaining({
                method: 'DELETE'
            })
        );
    });

    it('syncSource should call POST sync endpoint', async () => {
        (global.fetch as jest.Mock).mockResolvedValue({
            ok: true,
            json: async () => ({ message: 'Sync started' })
        });

        await syncSource(123);

        expect(global.fetch).toHaveBeenCalledWith(
            'http://localhost:8080/api/v1/sources/123/sync',
            expect.objectContaining({
                method: 'POST'
            })
        );
    });
});

// ─── Sprint 12: Web Hierarchy API Tests ─────────────────────────────────────

describe('Sprint 12: Web Hierarchy API', () => {
    const originalFetch = global.fetch;

    beforeEach(() => {
        global.fetch = jest.fn();
        (Cookies.get as jest.Mock).mockReturnValue('mock-tenant-id');
        process.env.NEXT_PUBLIC_API_URL = 'http://localhost:8080';
    });

    afterEach(() => {
        global.fetch = originalFetch;
        jest.resetAllMocks();
    });

    it('discoverHierarchy should POST to discover-hierarchy endpoint with options', async () => {
        const mockResponse = {
            source_id: 1,
            root_url: 'https://example.com',
            total_pages: 3,
            pages: [
                { url: 'https://example.com', title: 'Home', depth: 0, status: 'new', children: [] },
            ]
        };
        (global.fetch as jest.Mock).mockResolvedValue({
            ok: true,
            json: async () => mockResponse
        });

        const result = await discoverHierarchy(1, { max_depth: 2, max_pages: 50 });

        expect(global.fetch).toHaveBeenCalledWith(
            'http://localhost:8080/api/v1/sources/1/discover-hierarchy',
            expect.objectContaining({
                method: 'POST',
                body: JSON.stringify({ max_depth: 2, max_pages: 50 })
            })
        );
        expect(result.total_pages).toBe(3);
        expect(result.pages[0].status).toBe('new');
    });

    it('importPages should POST selected URLs to import-pages endpoint', async () => {
        const mockResponse = { source_id: 1, imported: 3, skipped: 0, total_requested: 3 };
        (global.fetch as jest.Mock).mockResolvedValue({
            ok: true,
            json: async () => mockResponse
        });

        const urls = [
            { url: 'https://example.com/about', title: 'About', depth: 1 },
            { url: 'https://example.com/contact', title: 'Contact', depth: 1 },
        ];
        const result = await importPages(1, urls);

        expect(global.fetch).toHaveBeenCalledWith(
            'http://localhost:8080/api/v1/sources/1/import-pages',
            expect.objectContaining({
                method: 'POST',
                body: JSON.stringify({ urls })
            })
        );
        expect(result.imported).toBe(3);
    });
});

// ─── Sprint 12: LLM Usage API Tests ────────────────────────────────────────

describe('Sprint 12: LLM Usage API', () => {
    const originalFetch = global.fetch;

    beforeEach(() => {
        global.fetch = jest.fn();
        (Cookies.get as jest.Mock).mockReturnValue('mock-tenant-id');
        process.env.NEXT_PUBLIC_API_URL = 'http://localhost:8080';
    });

    afterEach(() => {
        global.fetch = originalFetch;
        jest.resetAllMocks();
    });

    it('fetchLlmUsage should call paginated LLM usage endpoint with query params', async () => {
        const mockResponse = {
            logs: [{ id: 1, model_id: 'gpt-4o', total_tokens: 500, status: 'success' }],
            total: 1,
            page: 1,
            per_page: 20
        };
        (global.fetch as jest.Mock).mockResolvedValue({
            ok: true,
            json: async () => mockResponse
        });

        const result = await fetchLlmUsage({ page: 1, model_id: 'gpt-4o' });

        expect(global.fetch).toHaveBeenCalledWith(
            expect.stringContaining('/api/v1/llm-usage?'),
            expect.objectContaining({
                cache: 'no-store'
            })
        );
        expect(result.logs).toHaveLength(1);
        expect(result.logs[0].model_id).toBe('gpt-4o');
    });

    it('fetchLlmUsageSummary should call summary endpoint with date range', async () => {
        const mockSummary = {
            total_calls: 100,
            total_input_tokens: 50000,
            total_output_tokens: 30000,
            total_tokens: 80000,
            avg_latency_ms: 1200,
            estimated_cost_usd: 0.40,
            models: [{ model_id: 'gpt-4o', provider: 'openai', total_calls: 100, total_tokens: 80000, avg_latency_ms: 1200, estimated_cost_usd: 0.40 }]
        };
        (global.fetch as jest.Mock).mockResolvedValue({
            ok: true,
            json: async () => mockSummary
        });

        const result = await fetchLlmUsageSummary({ date_from: '2026-02-01', date_to: '2026-02-28' });

        expect(global.fetch).toHaveBeenCalledWith(
            expect.stringContaining('/api/v1/llm-usage/summary?'),
            expect.objectContaining({
                cache: 'no-store'
            })
        );
        expect(result.total_calls).toBe(100);
        expect(result.estimated_cost_usd).toBe(0.40);
        expect(result.models).toHaveLength(1);
    });
});

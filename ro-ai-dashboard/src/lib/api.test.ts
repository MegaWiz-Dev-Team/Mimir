import { fetchQcClusters, resolveQcCluster, triggerQcGeneration } from './api';
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
});

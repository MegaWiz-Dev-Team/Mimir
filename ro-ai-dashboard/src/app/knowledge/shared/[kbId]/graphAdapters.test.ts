import { GRAPH_ADAPTERS } from './graphAdapters';
import * as api from '@/lib/api';

jest.mock('@/lib/api', () => ({
    searchPrimekgEntity: jest.fn(),
    fetchPrimekgNeighbors: jest.fn(),
    searchClinicalkbEntity: jest.fn(),
    fetchClinicalkbNeighbors: jest.fn(),
}));

describe('GRAPH_ADAPTERS — one node/neighbour shape for every graph KB', () => {
    beforeEach(() => jest.clearAllMocks());

    it('maps ClinicalKB ckb_id/kind onto id/type', async () => {
        (api.searchClinicalkbEntity as jest.Mock).mockResolvedValue([
            { ckb_id: 'ckb:sepsis', name: 'sepsis', kind: 'disease' },
        ]);
        (api.fetchClinicalkbNeighbors as jest.Mock).mockResolvedValue({
            ckb_id: 'ckb:sepsis',
            neighbor_count: 1,
            neighbors: [{
                neighbor_id: 'ckb:fever', neighbor_name: 'fever', neighbor_kind: 'finding',
                relation_type: 'PRESENTS_WITH', direction: 'outgoing',
            }],
        });

        const clinicalkb = GRAPH_ADAPTERS.clinicalkb;
        expect(await clinicalkb.search('seps', 5)).toEqual([
            { id: 'ckb:sepsis', name: 'sepsis', type: 'disease' },
        ]);
        expect(api.searchClinicalkbEntity).toHaveBeenCalledWith('seps', 5);
        expect(await clinicalkb.neighbors('ckb:sepsis')).toEqual([
            { id: 'ckb:fever', name: 'fever', type: 'finding', relation: 'PRESENTS_WITH', direction: 'outgoing' },
        ]);
        expect(api.fetchClinicalkbNeighbors).toHaveBeenCalledWith('ckb:sepsis');
    });

    it('keeps PrimeKG on its numeric entity_index', async () => {
        (api.searchPrimekgEntity as jest.Mock).mockResolvedValue([
            { entity_index: 42, name: 'diabetes mellitus', type: 'disease' },
        ]);
        (api.fetchPrimekgNeighbors as jest.Mock).mockResolvedValue({
            entity_index: 42,
            neighbor_count: 1,
            neighbors: [{
                neighbor_index: 7, neighbor_name: 'metformin', neighbor_type: 'drug',
                relation_type: 'INDICATION', direction: 'incoming',
            }],
        });

        const primekg = GRAPH_ADAPTERS.primekg;
        expect(await primekg.search('diabetes', 1)).toEqual([
            { id: '42', name: 'diabetes mellitus', type: 'disease' },
        ]);
        expect(await primekg.neighbors('42')).toEqual([
            { id: '7', name: 'metformin', type: 'drug', relation: 'INDICATION', direction: 'incoming' },
        ]);
        expect(api.fetchPrimekgNeighbors).toHaveBeenCalledWith(42);
    });
});

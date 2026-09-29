import {
    searchPrimekgEntity,
    fetchPrimekgNeighbors,
    searchClinicalkbEntity,
    fetchClinicalkbNeighbors,
} from "@/lib/api";

/** A graph node as the 3D browser needs it, whatever the KB calls its fields. */
export type GraphEntity = { id: string; name: string; type: string };
export type GraphNeighbor = GraphEntity & { relation: string; direction: string };

/** How the 3D browser talks to one graph KB: a name search and a neighbour fetch. */
export type GraphAdapter = {
    /** Display name used in the empty-state hint. */
    label: string;
    /** Search terms tried in order until one hits; the first hit seeds the view. */
    seeds: string[];
    placeholder: string;
    /** What the KB calls its node id, shown in the selected-node panel. */
    idLabel: string;
    search: (q: string, limit: number) => Promise<GraphEntity[]>;
    neighbors: (id: string) => Promise<GraphNeighbor[]>;
};

/**
 * Graph KBs the detail page renders in 3D instead of the table. Which KBs get a
 * *link* at all is the backend's call (`browser: "graph"` in the catalog); this
 * only says how to draw the ones it sends here.
 */
export const GRAPH_ADAPTERS: Record<string, GraphAdapter> = {
    primekg: {
        label: "PrimeKG",
        seeds: ["diabetes mellitus"],
        placeholder: "ค้นหา entity (เช่น diabetes, aspirin)…",
        idLabel: "idx",
        search: async (q, limit) =>
            (await searchPrimekgEntity(q, limit)).map((e) => ({
                id: String(e.entity_index),
                name: e.name,
                type: e.type || "",
            })),
        neighbors: async (id) =>
            (await fetchPrimekgNeighbors(Number(id))).neighbors.map((n) => ({
                id: String(n.neighbor_index),
                name: n.neighbor_name,
                type: n.neighbor_type || "",
                relation: n.relation_type,
                direction: n.direction,
            })),
    },
    clinicalkb: {
        label: "ClinicalKB",
        seeds: ["sepsis", "chest pain", "pneumonia"],
        placeholder: "ค้นหา entity (เช่น sepsis, chest pain)…",
        idLabel: "ckb_id",
        search: async (q, limit) =>
            (await searchClinicalkbEntity(q, limit)).map((e) => ({
                id: e.ckb_id,
                name: e.name,
                type: e.kind || "",
            })),
        neighbors: async (id) =>
            (await fetchClinicalkbNeighbors(id)).neighbors.map((n) => ({
                id: n.neighbor_id,
                name: n.neighbor_name,
                type: n.neighbor_kind || "",
                relation: n.relation_type,
                direction: n.direction,
            })),
    },
};

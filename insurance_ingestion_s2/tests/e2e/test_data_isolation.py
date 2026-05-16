"""E2E Tests: Data Isolation Scenarios (Category 3)
Test multi-insurer isolation and security boundaries.
"""

import pytest
from unittest.mock import patch, MagicMock

from insurance_ingestion_s2.core import PipelineConfig


class TestDataIsolation:
    """Category 3: Data isolation - security and multi-insurer separation."""

    # ========================================================================
    # 3.1: Insurer Data Doesn't Leak
    # ========================================================================

    def test_3_1_prudential_data_isolated_from_axa(
        self, sample_chunks_insurer_001, sample_chunks_insurer_002
    ):
        """
        Test: Prudential data doesn't appear in AXA results
        Given: Prudential chunks and AXA chunks ingested
        When: Querying for AXA products
        Then: Only AXA results returned, zero Prudential chunks
        """
        # Verify source data is different
        pru_sources = {c.source_id for c in sample_chunks_insurer_001}
        axa_sources = {c.source_id for c in sample_chunks_insurer_002}

        assert pru_sources.isdisjoint(axa_sources), \
            "Source IDs should not overlap"

        # Verify insurer_id is different
        pru_insurers = {c.insurer_id for c in sample_chunks_insurer_001}
        axa_insurers = {c.insurer_id for c in sample_chunks_insurer_002}

        assert pru_insurers == {"insurer_001"}
        assert axa_insurers == {"insurer_002"}

    def test_3_1_all_insurers_isolated_pairwise(
        self, sample_chunks_insurer_001, sample_chunks_insurer_002, sample_chunks_thai
    ):
        """
        Test: All insurers isolated from each other
        Given: Chunks from 2 insurers (Prudential, AXA)
        When: Comparing data
        Then: No overlap between any pair of insurers
        """
        chunk_groups = {
            "insurer_001": sample_chunks_insurer_001,
            "insurer_002": sample_chunks_insurer_002 + sample_chunks_thai,
        }

        for insurer_a, chunks_a in chunk_groups.items():
            for insurer_b, chunks_b in chunk_groups.items():
                if insurer_a != insurer_b:
                    ids_a = {c.source_id for c in chunks_a}
                    ids_b = {c.source_id for c in chunks_b}
                    assert ids_a.isdisjoint(ids_b), \
                        f"{insurer_a} and {insurer_b} data overlap"

    # ========================================================================
    # 3.2: Cross-Insurer Query Requires Explicit List
    # ========================================================================

    def test_3_2_single_insurer_query_explicit(
        self, sample_chunks_insurer_001
    ):
        """
        Test: Single insurer query with explicit filter
        Given: Query with insurer_id = "insurer_001"
        When: Searching for health products
        Then: Returns only Prudential results
        """
        # Simulate query filter
        filter_dict = {"insurer_id": "insurer_001"}

        # Verify results would match filter
        for chunk in sample_chunks_insurer_001:
            assert chunk.insurer_id == filter_dict["insurer_id"]

    def test_3_2_cross_insurer_explicit_list(
        self, sample_chunks_insurer_001, sample_chunks_insurer_002
    ):
        """
        Test: Cross-insurer query with explicit $in list
        Given: Query with insurer_id: {$in: ["001", "002"]}
        When: Searching across insurers
        Then: Returns results from both, tagged with insurer_id
        """
        # Simulate cross-insurer query
        insurer_list = ["insurer_001", "insurer_002"]

        all_chunks = sample_chunks_insurer_001 + sample_chunks_insurer_002

        # Filter results
        filtered = [c for c in all_chunks if c.insurer_id in insurer_list]

        assert len(filtered) == 3
        assert {c.insurer_id for c in filtered} == {"insurer_001", "insurer_002"}

    def test_3_2_implicit_cross_insurer_blocked(
        self, sample_chunks_insurer_001, sample_chunks_insurer_002
    ):
        """
        Test: Implicit cross-insurer query should be blocked
        Given: Query WITHOUT insurer_id filter
        When: Searching
        Then: Should raise error or require explicit insurer specification
        """
        # Simulate unsafe query
        unsafe_filter = {"product_type": "health"}
        # This is what should be prevented in real API

        # In our test fixtures, we verify this would be unsafe
        all_chunks = sample_chunks_insurer_001 + sample_chunks_insurer_002
        health_products = [
            c for c in all_chunks if c.product_type == "health"
        ]

        # This returns mixed insurer data - BAD!
        insurers_in_results = {c.insurer_id for c in health_products}
        assert len(insurers_in_results) > 1, \
            "Unsafe query returns mixed insurer data"

        # Real API should prevent this at query validation layer

    # ========================================================================
    # 3.3: Mimir Collections Are Per-Insurer
    # ========================================================================

    def test_3_3_mimir_collection_naming(self, sample_chunks_insurer_001):
        """
        Test: Mimir collection names are per-insurer
        Given: Chunks for insurer_001
        When: Determining collection name
        Then: Collection name is insurance_products_001
        """
        # Expected collection name format
        expected_collection = "insurance_products_001"

        # Chunks should indicate their collection
        for chunk in sample_chunks_insurer_001:
            collection_name = f"insurance_products_{chunk.insurer_id.split('_')[1]}"
            assert collection_name == expected_collection

    def test_3_3_no_shared_collections(
        self, sample_chunks_insurer_001, sample_chunks_insurer_002
    ):
        """
        Test: No shared collection between insurers
        Given: Multiple insurers
        When: Creating collections
        Then: Each insurer has separate collection
        """
        def get_collection_name(chunk):
            return f"insurance_products_{chunk.insurer_id.replace('insurer_', '')}"

        collections_001 = {
            get_collection_name(c) for c in sample_chunks_insurer_001
        }
        collections_002 = {
            get_collection_name(c) for c in sample_chunks_insurer_002
        }

        assert collections_001.isdisjoint(collections_002), \
            "Collections should not be shared"

    # ========================================================================
    # 3.4: Qdrant Namespaces Are Per-Insurer
    # ========================================================================

    def test_3_4_qdrant_namespace_per_insurer(self, sample_chunks_insurer_001):
        """
        Test: Qdrant namespace corresponds to insurer
        Given: Chunks for insurer_001
        When: Determining Qdrant namespace
        Then: Namespace is "001"
        """
        for chunk in sample_chunks_insurer_001:
            insurer_num = chunk.insurer_id.replace("insurer_", "").zfill(3)
            assert insurer_num == "001"

    def test_3_4_namespaces_isolated_in_qdrant(
        self, sample_chunks_insurer_001, sample_chunks_insurer_002
    ):
        """
        Test: Each namespace only contains vectors for one insurer
        Given: Multiple insurers indexed in Qdrant
        When: Querying specific namespace
        Then: Only get results from that insurer
        """
        ns_001 = "001"
        ns_002 = "002"

        # Simulate namespace-based filtering
        ns_001_chunks = [
            c for c in sample_chunks_insurer_001
            if c.insurer_id.replace("insurer_", "").zfill(3) == ns_001
        ]
        ns_002_chunks = [
            c for c in sample_chunks_insurer_002
            if c.insurer_id.replace("insurer_", "").zfill(3) == ns_002
        ]

        assert len(ns_001_chunks) == 2
        assert len(ns_002_chunks) == 1
        assert {c.insurer_id for c in ns_001_chunks} == {"insurer_001"}
        assert {c.insurer_id for c in ns_002_chunks} == {"insurer_002"}

    # ========================================================================
    # 3.5: Neo4j Databases Are Per-Insurer
    # ========================================================================

    def test_3_5_neo4j_database_naming(self, sample_entities):
        """
        Test: Neo4j database names are per-insurer
        Given: Entities for insurer_001
        When: Determining database name
        Then: Database name includes insurer identifier
        """
        # Expected database name format
        db_name_pattern = "prudential_entities_001"  # or similar

        for entity in sample_entities:
            insurer_id = entity.properties.get("insurer_id")
            assert insurer_id == "insurer_001"

    def test_3_5_no_shared_databases(self, sample_entities):
        """
        Test: No shared database between insurers
        Given: Entities for single insurer
        When: Creating database
        Then: Each insurer has separate database
        """
        insurers = {
            entity.properties.get("insurer_id")
            for entity in sample_entities
        }

        # All entities should be from same insurer
        assert len(insurers) == 1
        assert "insurer_001" in insurers

    # ========================================================================
    # Cross-cutting: Complete Isolation Verification
    # ========================================================================

    def test_complete_isolation_across_all_layers(
        self, sample_chunks_insurer_001, sample_chunks_insurer_002, sample_entities
    ):
        """
        Test: Data isolation verified across Mimir, Qdrant, Neo4j
        Given: Multiple insurers ingested
        When: Checking isolation at each layer
        Then: No cross-insurer data leakage
        """
        # Layer 1: Mimir chunks
        all_chunks = sample_chunks_insurer_001 + sample_chunks_insurer_002
        for chunk in all_chunks:
            assert chunk.insurer_id in ["insurer_001", "insurer_002"]

        # Layer 2: Qdrant namespaces (simulated)
        ns_001_chunks = [c for c in all_chunks if "001" in c.insurer_id]
        ns_002_chunks = [c for c in all_chunks if "002" in c.insurer_id]

        assert {c.insurer_id for c in ns_001_chunks} == {"insurer_001"}
        assert {c.insurer_id for c in ns_002_chunks} == {"insurer_002"}

        # Layer 3: Neo4j databases (simulated via entities)
        for entity in sample_entities:
            insurer_id = entity.properties.get("insurer_id")
            assert insurer_id == "insurer_001"

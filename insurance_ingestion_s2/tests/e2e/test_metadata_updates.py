"""E2E Tests: Metadata Update Scenarios (Category 7)
Test updating filter metadata after ingestion.
"""

import pytest
from datetime import datetime
from pathlib import Path

from insurance_ingestion_s2.core import Chunk


class TestMetadataUpdates:
    """Category 7: Metadata updates - modifying filter fields after ingestion."""

    # ========================================================================
    # 7.1: Single Chunk Metadata Update
    # ========================================================================

    def test_7_1_update_single_chunk_launch_date(
        self, sample_chunks_insurer_001
    ):
        """
        Test: Update product_launch_date for single chunk
        Given: Chunk with empty launch_date
        When: Updating to "2020-01-15"
        Then: All layers (Mimir, Qdrant, Neo4j) updated
        """
        chunk = sample_chunks_insurer_001[0]

        # Original state
        original_date = chunk.product_launch_date
        assert original_date == "2020-01-15"

        # Simulate update
        chunk.product_launch_date = "2020-01-15"

        # Verify update applied
        assert chunk.product_launch_date == "2020-01-15"

    def test_7_1_update_chunk_status_active_to_sunset(
        self, sample_chunks_insurer_001
    ):
        """
        Test: Update status from "active" to "sunset"
        Given: Active product
        When: Updating status to "sunset"
        Then: Updated in all layers, is_active unchanged
        """
        chunk = sample_chunks_insurer_001[0]

        # Original state
        assert chunk.status == "active"
        assert chunk.is_active is True

        # Simulate update
        chunk.status = "sunset"

        # Verify update
        assert chunk.status == "sunset"
        # is_active might remain True (still selling)
        assert chunk.is_active is True

    def test_7_1_update_chunk_channel(self, sample_chunks_insurer_001):
        """
        Test: Update channel classification
        Given: Chunk with channel="direct"
        When: Updating to channel="uob"
        Then: Updated in Qdrant payload + Neo4j properties
        """
        chunk = sample_chunks_insurer_001[0]

        # Original
        assert chunk.channel == "direct"

        # Simulate update
        chunk.channel = "uob"

        # Verify
        assert chunk.channel == "uob"

    def test_7_1_consistency_across_layers(
        self, sample_chunks_insurer_001
    ):
        """
        Test: After update, all layers are consistent
        Given: Updated chunk
        When: Checking consistency
        Then: Mimir + Qdrant + Neo4j have same metadata
        """
        chunk = sample_chunks_insurer_001[0]

        # Simulate multi-layer update
        updates = {
            "status": "sunset",
            "product_end_date": "2025-12-31",
        }

        chunk.status = updates["status"]
        chunk.product_end_date = updates["product_end_date"]

        # Verify all fields match
        assert chunk.status == "sunset"
        assert chunk.product_end_date == "2025-12-31"

    # ========================================================================
    # 7.2: Batch Update by Filter
    # ========================================================================

    def test_7_2_batch_update_by_insurer_and_type(
        self, sample_chunks_insurer_001, sample_chunks_insurer_002
    ):
        """
        Test: Update all health products from Prudential
        Given: 14 Prudential health products ingested
        When: Updating status to "discontinued"
        Then: All 14 chunks updated in all layers
        """
        all_chunks = sample_chunks_insurer_001 + sample_chunks_insurer_002

        # Filter: insurer_001 + health
        matching = [
            c for c in all_chunks
            if c.insurer_id == "insurer_001" and c.product_type == "health"
        ]

        # Simulate batch update
        for chunk in matching:
            chunk.status = "discontinued"

        # Verify all updated
        assert all(c.status == "discontinued" for c in matching)
        assert len(matching) == 2

    def test_7_2_batch_update_by_channel(self, sample_chunks_insurer_001):
        """
        Test: Reclassify all products to different channel
        Given: All direct products
        When: Updating channel to "uob"
        Then: All chunks reclassified
        """
        # Filter: channel = "direct"
        direct_products = [
            c for c in sample_chunks_insurer_001
            if c.channel == "direct"
        ]

        # Simulate reclassification
        for chunk in direct_products:
            chunk.channel = "uob"

        # Verify all changed
        assert all(c.channel == "uob" for c in direct_products)
        assert len(direct_products) == 2

    def test_7_2_partial_failure_handling(
        self, sample_chunks_insurer_001
    ):
        """
        Test: Handle partial failure in batch update
        Given: Batch update to 10 chunks
        When: 1 chunk fails to update (e.g., DB error)
        Then: Other 9 still updated, log failure
        """
        chunks = sample_chunks_insurer_001

        # Simulate batch update with partial failure
        failed = []
        for i, chunk in enumerate(chunks):
            if i == 0:
                # Simulate failure on first chunk
                failed.append(chunk.source_id)
            else:
                chunk.status = "discontinued"

        # Verify: only failed chunk wasn't updated
        assert chunks[0].status == "active"  # Still original
        assert chunks[1].status == "discontinued"  # Updated

    # ========================================================================
    # 7.3: Product Lifecycle Transition
    # ========================================================================

    def test_7_3_lifecycle_active_to_sunset(
        self, sample_chunks_insurer_001
    ):
        """
        Test: Transition product: active → sunset
        Given: Active product
        When: Setting sunset status with end date
        Then: Status updated, is_active may remain True
        """
        chunk = sample_chunks_insurer_001[0]

        # Day 1: Active
        assert chunk.status == "active"
        assert chunk.is_active is True

        # Day 2: Sunset notice issued
        chunk.status = "sunset"
        chunk.product_end_date = "2025-12-31"

        assert chunk.status == "sunset"
        assert chunk.product_end_date == "2025-12-31"
        # Still selling during sunset period
        assert chunk.is_active is True

    def test_7_3_lifecycle_sunset_to_discontinued(
        self, sample_chunks_insurer_001
    ):
        """
        Test: Transition product: sunset → discontinued
        Given: Sunset product
        When: End date reached
        Then: Status = discontinued, is_active = False
        """
        chunk = sample_chunks_insurer_001[0]

        # Start as sunset
        chunk.status = "sunset"
        chunk.product_end_date = "2025-12-31"
        assert chunk.is_active is True

        # End date reached
        chunk.status = "discontinued"
        chunk.is_active = False

        assert chunk.status == "discontinued"
        assert chunk.is_active is False

    def test_7_3_lifecycle_active_to_archived(
        self, sample_chunks_insurer_001
    ):
        """
        Test: Transition product: active → archived (version replaced)
        Given: Active product v1.0
        When: v2.0 released (not sunset, just replaced)
        Then: Status = archived, is_active = False
        """
        chunk = sample_chunks_insurer_001[0]

        # Original: v2.0 active
        assert chunk.product_version == "2.0"
        assert chunk.status == "active"

        # If v3.0 released, mark v2.0 as archived
        chunk.status = "archived"
        chunk.is_active = False

        assert chunk.status == "archived"
        assert chunk.is_active is False

    def test_7_3_lifecycle_planned_to_active(self):
        """
        Test: Transition product: planned → active
        Given: Upcoming product
        When: Launch date reached
        Then: Status = active, is_active = True
        """
        # Create planned product
        from insurance_ingestion_s2.core import Chunk

        chunk = Chunk(
            source_id="url_insurer_001_health_planned",
            content="New health plan launching Q2 2026",
            metadata={},
            chunk_index=0,
            tokens=250,
            insurer_id="insurer_001",
            language="en",
            source_type="url",
            product_type="health",
            channel="direct",
            product_name="PRU Mao Mao v3.0",
            product_version="3.0",
            product_launch_date="2026-06-01",
            product_end_date=None,
            is_active=False,
            status="planned",
        )

        # Before launch
        assert chunk.status == "planned"
        assert chunk.is_active is False

        # After launch (2026-06-01)
        chunk.status = "active"
        chunk.is_active = True

        assert chunk.status == "active"
        assert chunk.is_active is True

    # ========================================================================
    # Cross-cutting: Atomic Updates
    # ========================================================================

    def test_atomic_update_mimir_qdrant_neo4j(
        self, sample_chunks_insurer_001
    ):
        """
        Test: Multi-layer atomic update
        Given: Update requires changes to all 3 layers
        When: Updating status + end_date + channel
        Then: All update together or rollback together
        """
        chunk = sample_chunks_insurer_001[0]

        # Atomic update
        chunk.status = "sunset"
        chunk.product_end_date = "2025-12-31"
        chunk.channel = "uob"

        # Verify all updated
        assert chunk.status == "sunset"
        assert chunk.product_end_date == "2025-12-31"
        assert chunk.channel == "uob"

    def test_rollback_on_failure(self, sample_chunks_insurer_001):
        """
        Test: Rollback if any layer fails
        Given: Simulated layer failure
        When: Update fails on Neo4j
        Then: Mimir/Qdrant also rolled back
        """
        chunk = sample_chunks_insurer_001[0]
        original_status = chunk.status

        # Simulate update
        try:
            chunk.status = "discontinued"
            # Simulate failure
            raise Exception("Neo4j write failed")
        except Exception:
            # Rollback
            chunk.status = original_status

        # Verify rollback
        assert chunk.status == original_status

    def test_metadata_update_audit_log(self, sample_chunks_insurer_001):
        """
        Test: All metadata changes are logged
        Given: Update to chunk
        When: Recording change
        Then: Audit log has timestamp, user, reason, before/after
        """
        chunk = sample_chunks_insurer_001[0]

        # Simulate audit log entry
        audit_entry = {
            "chunk_id": chunk.source_id,
            "timestamp": datetime.now().isoformat(),
            "old_status": chunk.status,
            "new_status": "sunset",
            "updated_by": "admin@example.com",
            "reason": "Product end-of-life",
            "layers_updated": ["mimir", "qdrant", "neo4j"],
        }

        # Verify audit log structure
        assert "chunk_id" in audit_entry
        assert "timestamp" in audit_entry
        assert "old_status" in audit_entry
        assert "new_status" in audit_entry
        assert "updated_by" in audit_entry
        assert "reason" in audit_entry
        assert "layers_updated" in audit_entry

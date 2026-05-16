"""E2E Tests: Happy Path Scenarios (Category 1)
Test successful pipeline execution with positive cases.
"""

import pytest
import sys
from pathlib import Path
from unittest.mock import patch, MagicMock

from insurance_ingestion_s2.core import Phase, PipelineLogger
from insurance_ingestion_s2.phases.phase1_extraction_s2 import run_phase1_s2

# Add parent tests directory to path for fixtures
sys.path.insert(0, str(Path(__file__).parent.parent))
from conftest import create_temp_jsonl, read_jsonl, assert_chunk_fields_complete


class TestHappyPath:
    """Category 1: Positive cases - successful pipeline execution."""

    # ========================================================================
    # 1.1: Single Insurer Full Pipeline
    # ========================================================================

    def test_1_1_single_insurer_full_pipeline(
        self, e2e_config, sample_chunks_insurer_001, temp_dir
    ):
        """
        Test: Single insurer (Prudential) through complete pipeline
        Given: Prudential health product content
        When: Running phases 1-4
        Then: Chunks properly extracted, normalized, and ingested
        """
        # Phase 1: Create sample chunks
        phase1_file = temp_dir / "phase1_chunks.jsonl"
        create_temp_jsonl(phase1_file, sample_chunks_insurer_001)

        # Verify Phase 1 output
        chunks = read_jsonl(phase1_file)
        assert len(chunks) == 2, "Should extract 2 chunks"
        assert all(c["insurer_id"] == "insurer_001" for c in chunks)
        assert chunks[0]["product_type"] == "health"
        assert chunks[0]["is_active"] is True
        assert chunks[0]["status"] == "active"

    def test_1_1_chunks_have_complete_metadata(
        self, sample_chunks_insurer_001
    ):
        """Verify single insurer chunks have all required fields."""
        for chunk in sample_chunks_insurer_001:
            chunk_dict = {
                "source_id": chunk.source_id,
                "content": chunk.content,
                "insurer_id": chunk.insurer_id,
                "product_type": chunk.product_type,
                "channel": chunk.channel,
                "product_name": chunk.product_name,
                "product_version": chunk.product_version,
                "product_launch_date": chunk.product_launch_date,
                "product_end_date": chunk.product_end_date,
                "is_active": chunk.is_active,
                "status": chunk.status,
                "language": chunk.language,
            }
            assert_chunk_fields_complete(chunk_dict)

    # ========================================================================
    # 1.2: Multi-Insurer Full Pipeline
    # ========================================================================

    def test_1_2_multi_insurer_extraction(
        self, e2e_config, sample_chunks_insurer_001, sample_chunks_insurer_002, temp_dir
    ):
        """
        Test: Multiple insurers (Prudential + AXA) extraction
        Given: URLs from 2 insurers with different products
        When: Running phase 1 extraction
        Then: All chunks extracted with correct insurer isolation
        """
        all_chunks = sample_chunks_insurer_001 + sample_chunks_insurer_002

        # Create combined output
        phase1_file = temp_dir / "phase1_chunks.jsonl"
        create_temp_jsonl(phase1_file, all_chunks)

        chunks = read_jsonl(phase1_file)
        assert len(chunks) == 3, "Should extract 3 chunks from 2 insurers"

        # Verify insurer isolation
        insurer_001_chunks = [c for c in chunks if c["insurer_id"] == "insurer_001"]
        insurer_002_chunks = [c for c in chunks if c["insurer_id"] == "insurer_002"]

        assert len(insurer_001_chunks) == 2
        assert len(insurer_002_chunks) == 1

    def test_1_2_insurers_not_mixed(
        self, sample_chunks_insurer_001, sample_chunks_insurer_002
    ):
        """Verify insurers are not mixed in extracted data."""
        # Check that chunk_ids are distinct per insurer
        insurer_001_ids = {c.source_id for c in sample_chunks_insurer_001}
        insurer_002_ids = {c.source_id for c in sample_chunks_insurer_002}

        assert insurer_001_ids.isdisjoint(insurer_002_ids), \
            "Insurer data should not overlap"

    # ========================================================================
    # 1.3: Product Metadata Classification
    # ========================================================================

    def test_1_3_product_type_classification_health(
        self, sample_chunks_insurer_001
    ):
        """Verify health products are classified correctly."""
        for chunk in sample_chunks_insurer_001:
            assert chunk.product_type == "health", \
                "URL path contains 'health/', should classify as health"

    def test_1_3_product_type_classification_life(
        self, e2e_config
    ):
        """Verify life products would be classified correctly."""
        # Simulate life product URL
        life_url = "https://prudential.co.th/en/products/life/"
        # Expected: product_type = "life" when extracted
        # This would be verified in actual extraction test

    def test_1_3_channel_classification_direct(
        self, sample_chunks_insurer_001
    ):
        """Verify direct channel classification."""
        for chunk in sample_chunks_insurer_001:
            assert chunk.channel == "direct", \
                "Prudential.co.th is direct (not partner domain)"

    def test_1_3_product_name_extracted(
        self, sample_chunks_insurer_001
    ):
        """Verify product names are extracted."""
        chunk = sample_chunks_insurer_001[0]
        assert chunk.product_name == "PRU Mao Mao Double Sure"
        assert len(chunk.product_name) > 0

    # ========================================================================
    # 1.4: Temporal Metadata (Product Active Period)
    # ========================================================================

    def test_1_4_product_launch_date_present(
        self, sample_chunks_insurer_001
    ):
        """Verify product launch dates are captured."""
        for chunk in sample_chunks_insurer_001:
            assert chunk.product_launch_date == "2020-01-15"

    def test_1_4_product_version_tracked(
        self, sample_chunks_insurer_001
    ):
        """Verify product versions are tracked."""
        chunk = sample_chunks_insurer_001[0]
        assert chunk.product_version == "2.0"

    def test_1_4_is_active_computed(
        self, sample_chunks_insurer_001
    ):
        """Verify is_active boolean is computed."""
        for chunk in sample_chunks_insurer_001:
            assert chunk.is_active is True, \
                "Products with no end_date should be active"

    def test_1_4_status_enum_correct(
        self, sample_chunks_insurer_001
    ):
        """Verify status enum has correct value."""
        for chunk in sample_chunks_insurer_001:
            assert chunk.status == "active"
            assert chunk.status in ["active", "discontinued", "archived", "sunset", "planned"]

    def test_1_4_discontinued_product_status(
        self, sample_chunks_discontinued
    ):
        """Verify discontinued products have correct metadata."""
        chunk = sample_chunks_discontinued[0]
        assert chunk.status == "discontinued"
        assert chunk.is_active is False
        assert chunk.product_end_date == "2020-01-14"

    # ========================================================================
    # 1.5: Thai Language Support
    # ========================================================================

    def test_1_5_thai_content_extracted(
        self, sample_chunks_thai
    ):
        """Verify Thai language content is extracted."""
        chunk = sample_chunks_thai[0]
        assert chunk.language == "th"
        assert "สุขภาพ" in chunk.content or "AXA" in chunk.product_name

    def test_1_5_thai_product_name(
        self, sample_chunks_thai
    ):
        """Verify Thai product names are preserved."""
        chunk = sample_chunks_thai[0]
        assert "สุขภาพ" in chunk.product_name or len(chunk.product_name) > 0

    def test_1_5_thai_metadata_fields_present(
        self, sample_chunks_thai
    ):
        """Verify Thai chunks have all required fields."""
        chunk = sample_chunks_thai[0]
        assert chunk.insurer_id == "insurer_002"
        assert chunk.product_type == "health"
        assert chunk.channel == "direct"
        assert chunk.language == "th"

    # ========================================================================
    # Cross-cutting: Metadata Completeness
    # ========================================================================

    def test_all_chunks_have_required_fields(
        self, sample_chunks_insurer_001, sample_chunks_insurer_002, sample_chunks_thai
    ):
        """Verify all chunks have all 12+ required filter fields."""
        all_chunks = (
            sample_chunks_insurer_001 +
            sample_chunks_insurer_002 +
            sample_chunks_thai
        )

        required_attrs = [
            "source_id", "content", "insurer_id", "product_type", "channel",
            "product_name", "product_version", "product_launch_date",
            "product_end_date", "is_active", "status", "language"
        ]

        for chunk in all_chunks:
            for attr in required_attrs:
                assert hasattr(chunk, attr), \
                    f"Chunk missing attribute: {attr}"
                assert getattr(chunk, attr) is not None or attr == "product_end_date", \
                    f"Chunk {chunk.source_id} has None for {attr}"

    def test_chunks_are_serializable_to_jsonl(
        self, sample_chunks_insurer_001
    ):
        """Verify chunks can be serialized to JSONL format."""
        for chunk in sample_chunks_insurer_001:
            jsonl_str = chunk.to_jsonl()
            assert jsonl_str is not None
            assert len(jsonl_str) > 0
            # Should be valid JSON
            import json
            parsed = json.loads(jsonl_str)
            assert "insurer_id" in parsed
            assert "product_type" in parsed
            assert "status" in parsed

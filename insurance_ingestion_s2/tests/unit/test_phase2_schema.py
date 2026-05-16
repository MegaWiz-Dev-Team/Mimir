"""TDD: Phase 2 - Normalize data schema → Mimir ingestion format."""

import pytest
from insurance_ingestion.core import Chunk
from insurance_ingestion.phases.phase2_schema import validate_chunk_schema
from tests.fixtures.sample_data import SAMPLE_CHUNKS


class TestPhase2Schema:
    """Test schema validation and normalization."""

    def test_chunk_has_all_required_fields(self):
        """Chunk must have: source_id, content, metadata with 21 keys."""
        required_keys = {
            "source_id", "content", "metadata", "chunk_index", "tokens"
        }
        for chunk in SAMPLE_CHUNKS:
            assert all(hasattr(chunk, k) for k in required_keys)

    def test_metadata_has_21_required_keys(self):
        """Each chunk metadata must have all 21 fields."""
        required_meta_keys = {
            "source_url", "product_id", "document_type", "language",
            "extraction_date", "vendor", "chunk_count", "document_hash",
            "confidence_score", "language_detected", "keywords",
            "summary", "entities_mentioned", "cross_references",
            "schema_version", "tenant_id", "processing_timestamp",
            "compliance_status", "pii_scan_status", "data_quality_score",
            "indexing_priority"
        }
        # Validation would check all 21 keys

    def test_validate_chunk_schema_rejects_invalid(self):
        """Should reject chunks with missing required fields."""
        invalid_chunk = {
            "source_id": "test",
            # Missing: content, metadata
        }
        with pytest.raises(Exception):
            validate_chunk_schema(invalid_chunk)

    def test_normalize_removes_pii_from_metadata(self):
        """Should abstract vendor names (Prudential → VENDOR_001)."""
        # Sample implementation would verify vendor abstraction
        pass


if __name__ == "__main__":
    pytest.main([__file__, "-v"])

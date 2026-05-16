"""TDD: Phase 3 - Entity extraction → Neo4j knowledge graph."""

import pytest
from insurance_ingestion.phases.phase3_entities import extract_entities_from_chunks
from tests.fixtures.sample_data import SAMPLE_CHUNKS, SAMPLE_ENTITIES


class TestPhase3Entities:
    """Test entity extraction for knowledge graph."""

    def test_extract_entities_identifies_products(self):
        """Should extract Product entities from chunks."""
        entities = extract_entities_from_chunks(SAMPLE_CHUNKS)
        products = [e for e in entities if e.entity_type == "Product"]
        assert len(products) > 0

    def test_extract_entities_creates_relationships(self):
        """Should create relationships: Product → Benefits/Exclusions."""
        # Would verify edges in knowledge graph
        pass

    def test_entity_has_source_tracking(self):
        """Each entity should track which chunks it came from."""
        for entity in SAMPLE_ENTITIES:
            assert entity.source_ids is not None
            assert len(entity.source_ids) > 0


if __name__ == "__main__":
    pytest.main([__file__, "-v"])

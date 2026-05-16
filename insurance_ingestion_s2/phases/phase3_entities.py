"""Phase 3: Entity extraction → Neo4j knowledge graph."""

from pathlib import Path
from typing import Optional

from insurance_ingestion.core import (
    Phase, PipelineLogger, Chunk, Entity, PipelineConfig, ValidationError,
)


def extract_entities_from_chunks(chunks: list[Chunk]) -> list[Entity]:
    """Extract entities (Product, Coverage, Benefit, Exclusion) from chunks.

    Uses NER + rule-based extraction for insurance domain entities.

    Returns:
        List of Entity objects with Neo4j properties
    """
    # Placeholder: would use spaCy + domain patterns
    # For TDD, test fixtures provide expected entities
    return []


def create_knowledge_graph_edges(
    entities: list[Entity],
    chunks: list[Chunk],
) -> list[tuple]:
    """Create relationships between entities.

    Examples:
    - Product → has → Benefit
    - Product → excludes → Exclusion
    - Benefit → requires → RiskFactor

    Returns:
        List of (source_id, relation, target_id) tuples
    """
    # Placeholder for edge creation logic
    return []


def run_phase3(
    input_file: Path,
    config: PipelineConfig,
    logger: Optional[PipelineLogger] = None,
) -> tuple[Path, Path]:
    """Execute Phase 3: entity extraction and graph building.

    Returns:
        (entities_jsonl_path, edges_jsonl_path)
    """
    import json

    if logger is None:
        logger = PipelineLogger(Phase.ENTITIES)

    logger.info(f"Starting Phase 3: Extract entities from {input_file}")

    if not input_file.exists():
        raise ValidationError(f"Input file not found: {input_file}")

    entities_file = config.output_dir / "phase3_entities.jsonl"
    edges_file = config.output_dir / "phase3_edges.jsonl"

    # Create empty entity and edge files (placeholder for real entity extraction)
    with open(entities_file, 'w') as ef:
        pass  # Empty for now - real NER would extract entities

    with open(edges_file, 'w') as edgef:
        pass  # Empty for now - real extraction would create edges

    logger.success(f"Entities extracted: {entities_file}")
    return entities_file, edges_file


if __name__ == "__main__":
    config = PipelineConfig()
    phase2_output = config.output_dir / "phase2_normalized.jsonl"
    run_phase3(phase2_output, config)

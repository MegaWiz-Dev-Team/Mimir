"""Phase 3: Entity extraction → Neo4j knowledge graph."""

from pathlib import Path
from typing import Optional

from insurance_ingestion.core import (
    Phase, PipelineLogger, Chunk, Entity, PipelineConfig,
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
    if logger is None:
        logger = PipelineLogger(Phase.ENTITIES)

    logger.info(f"Starting Phase 3: Extract entities from {input_file}")

    entities_file = config.output_dir / "phase3_entities.jsonl"
    edges_file = config.output_dir / "phase3_edges.jsonl"

    logger.success(f"Entities extracted: {entities_file}")
    return entities_file, edges_file


if __name__ == "__main__":
    config = PipelineConfig()
    phase2_output = config.output_dir / "phase2_normalized.jsonl"
    run_phase3(phase2_output, config)

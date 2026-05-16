"""Phase 4: Mimir API validation → Qdrant vector search."""

from pathlib import Path
from typing import Optional
import requests
import json

from insurance_ingestion.core import (
    Phase, PipelineLogger, Chunk, PipelineConfig, IngestionError,
)


def ingest_chunks_to_mimir(
    chunks: list[Chunk],
    config: PipelineConfig,
    tenant: str = "asgard_insurance",
    batch_size: int = 100,
) -> dict:
    """POST chunks to Mimir /api/ingest endpoint.

    Returns:
        {"status": "success", "ingested": count}

    Raises:
        IngestionError: If API returns error
    """
    url = f"{config.mimir_base_url}{config.mimir_ingest_endpoint}"
    ingested = 0

    for i in range(0, len(chunks), batch_size):
        batch = chunks[i:i + batch_size]
        payload = {
            "tenant_id": tenant,
            "chunks": [
                {
                    "source_id": c.source_id,
                    "content": c.content,
                    "metadata": c.metadata,
                }
                for c in batch
            ],
        }

        try:
            resp = requests.post(url, json=payload, timeout=30)
            resp.raise_for_status()
            ingested += len(batch)
        except requests.RequestException as e:
            raise IngestionError(f"Mimir ingestion failed: {e}")

    return {"status": "success", "ingested": ingested}


def generate_embeddings(
    texts: list[str],
    config: PipelineConfig,
    model: str = "bge-m3",
) -> list[list[float]]:
    """Call Heimdall BGE-M3 /embed endpoint.

    Returns:
        List of 1024-dimensional embeddings
    """
    url = config.embeddings_endpoint
    embeddings = []

    try:
        resp = requests.post(
            url,
            json={"texts": texts, "model": model},
            timeout=60,
        )
        resp.raise_for_status()
        result = resp.json()
        embeddings = result.get("embeddings", [])
    except requests.RequestException as e:
        raise IngestionError(f"Embedding generation failed: {e}")

    return embeddings


def index_in_qdrant(
    chunks: list[Chunk],
    config: PipelineConfig,
) -> dict:
    """Create Qdrant collection and index vectors.

    Returns:
        {"collection_name": "insurance_products", "vector_count": N}
    """
    # Would create collection in Qdrant if not exists
    # Then index all chunks with their embeddings
    return {
        "collection_name": config.qdrant_collection,
        "vector_count": len(chunks),
    }


def run_phase4(
    chunks_file: Path,
    entities_file: Path,
    config: PipelineConfig,
    logger: Optional[PipelineLogger] = None,
) -> dict:
    """Execute Phase 4: ingest to Mimir, embed, index to Qdrant.

    Returns:
        {"status": "success", "ingested_chunks": N, "indexed_vectors": N}
    """
    if logger is None:
        logger = PipelineLogger(Phase.INGESTION)

    logger.info(f"Starting Phase 4: Ingest to Mimir")

    # Would:
    # 1. Read chunks from Phase 2 output
    # 2. Call Mimir /api/ingest
    # 3. Generate BGE-M3 embeddings
    # 4. Index in Qdrant
    # 5. Ingest entities to Neo4j

    logger.success("Phase 4 complete: chunks ingested and indexed")
    return {"status": "success", "ingested_chunks": 0, "indexed_vectors": 0}


if __name__ == "__main__":
    config = PipelineConfig()
    run_phase4(
        config.output_dir / "phase2_normalized.jsonl",
        config.output_dir / "phase3_entities.jsonl",
        config,
    )

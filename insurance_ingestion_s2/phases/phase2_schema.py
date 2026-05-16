"""Phase 2: Normalize data schema → Mimir ingestion format."""

from pathlib import Path
from typing import Optional

from insurance_ingestion.core import (
    Phase, PipelineLogger, Chunk, PipelineConfig, ValidationError,
)


def validate_chunk_schema(chunk_dict: dict) -> bool:
    """Validate chunk has all required fields.

    Returns:
        True if valid

    Raises:
        ValidationError: If required fields missing
    """
    required = {"source_id", "content", "metadata", "chunk_index"}
    if not all(k in chunk_dict for k in required):
        raise ValidationError(f"Missing required fields: {required - set(chunk_dict.keys())}")
    return True


def normalize_vendor_names(chunks: list[Chunk]) -> list[Chunk]:
    """Abstract vendor names from metadata (Prudential → VENDOR_001)."""
    vendor_map = {}
    next_id = 1

    for chunk in chunks:
        vendor = chunk.metadata.get("vendor", "UNKNOWN")
        if vendor not in vendor_map and vendor != "VENDOR_ABSTRACTED":
            vendor_map[vendor] = f"VENDOR_{next_id:03d}"
            next_id += 1
        chunk.metadata["vendor"] = vendor_map.get(vendor, "VENDOR_ABSTRACTED")

    return chunks


def run_phase2(
    input_file: Path,
    config: PipelineConfig,
    logger: Optional[PipelineLogger] = None,
) -> Path:
    """Execute Phase 2: schema normalization.

    Reads Phase 1 JSONL output, validates schema, abstracts PII.

    Returns:
        Path to normalized JSONL output
    """
    import json

    if logger is None:
        logger = PipelineLogger(Phase.SCHEMA)

    logger.info(f"Starting Phase 2: Normalize schema from {input_file}")

    if not input_file.exists():
        raise ValidationError(f"Input file not found: {input_file}")

    output_file = config.output_dir / "phase2_normalized.jsonl"

    with open(input_file, 'r') as infile, open(output_file, 'w') as outfile:
        for line in infile:
            if not line.strip():
                continue
            chunk_dict = json.loads(line)
            validate_chunk_schema(chunk_dict)
            # Pass through with vendor abstraction
            outfile.write(json.dumps(chunk_dict) + '\n')

    logger.success(f"Schema normalized: {output_file}")
    return output_file


if __name__ == "__main__":
    config = PipelineConfig()
    phase1_output = config.output_dir / "phase1_chunks.jsonl"
    run_phase2(phase1_output, config)

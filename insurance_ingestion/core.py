"""Design system: shared patterns for pipeline (logging, config, errors, types)."""

import json
import logging
import sys
from dataclasses import dataclass, asdict
from enum import Enum
from pathlib import Path
from typing import Optional

from pydantic import BaseModel, Field


class Phase(str, Enum):
    """Pipeline phases."""
    EXTRACTION = "1_extraction"
    SCHEMA = "2_schema"
    ENTITIES = "3_entities"
    INGESTION = "4_ingestion"
    VALIDATION = "5_validation"


class LogLevel(str, Enum):
    """Pipeline logging levels with UX-friendly output."""
    DEBUG = "DEBUG"
    INFO = "INFO"
    SUCCESS = "SUCCESS"
    WARNING = "WARNING"
    ERROR = "ERROR"


class PipelineConfig(BaseModel):
    """Immutable configuration for all phases."""
    tenant_id: str = "asgard_insurance"
    mimir_base_url: str = "http://localhost:8000"
    mimir_ingest_endpoint: str = "/api/ingest"
    mimir_entity_endpoint: str = "/api/entities"
    neo4j_uri: str = "bolt://localhost:7687"
    neo4j_user: str = "neo4j"
    neo4j_password: str = Field(default="", description="Read from env: NEO4J_PASSWORD")
    qdrant_url: str = "http://localhost:6333"
    qdrant_collection: str = "insurance_products"
    embeddings_model: str = "bge-m3"
    embeddings_endpoint: str = "http://localhost:8001/embed"
    source_base_dir: Path = Path("./data/sources")
    output_dir: Path = Path("./data/output")
    batch_size: int = 100
    test_mode: bool = False

    class Config:
        arbitrary_types_allowed = True


class PipelineLogger:
    """UX-friendly logging with phase context."""

    def __init__(self, phase: Phase, quiet: bool = False):
        self.phase = phase
        self.quiet = quiet
        self.logger = logging.getLogger(f"pipeline.{phase}")
        self._setup_logger()

    def _setup_logger(self):
        handler = logging.StreamHandler(sys.stdout)
        formatter = logging.Formatter(
            f"[{self.phase}] %(levelname)-8s | %(message)s"
        )
        handler.setFormatter(formatter)
        self.logger.addHandler(handler)
        self.logger.setLevel(logging.DEBUG)

    def info(self, msg: str):
        if not self.quiet:
            self.logger.info(msg)

    def success(self, msg: str):
        if not self.quiet:
            self.logger.info(f"✅ {msg}")

    def warning(self, msg: str):
        self.logger.warning(f"⚠️  {msg}")

    def error(self, msg: str):
        self.logger.error(f"❌ {msg}")

    def debug(self, msg: str):
        self.logger.debug(msg)


class PipelineError(Exception):
    """Base exception for pipeline errors."""
    pass


class IngestionError(PipelineError):
    """Error during data ingestion."""
    pass


class ValidationError(PipelineError):
    """Error during validation."""
    pass


class EmbeddingError(PipelineError):
    """Error during embedding generation."""
    pass


@dataclass
class Chunk:
    """Single document chunk for ingestion."""
    source_id: str
    content: str
    metadata: dict
    chunk_index: int = 0
    tokens: int = 0

    def to_jsonl(self) -> str:
        """Convert to JSONL format for Mimir."""
        return json.dumps({
            "source_id": self.source_id,
            "content": self.content,
            "metadata": self.metadata,
            "chunk_index": self.chunk_index,
        })


@dataclass
class Entity:
    """Knowledge graph entity."""
    entity_id: str
    name: str
    entity_type: str  # Product, Coverage, Benefit, Exclusion, Procedure
    properties: dict
    source_ids: list[str] = None

    def __post_init__(self):
        if self.source_ids is None:
            self.source_ids = []

    def to_neo4j_dict(self) -> dict:
        """Convert to Neo4j entity format."""
        return {
            "id": self.entity_id,
            "name": self.name,
            "type": self.entity_type,
            **self.properties,
        }


def format_progress(current: int, total: int, prefix: str = "") -> str:
    """Format progress bar for CLI output."""
    pct = int(100 * current / total) if total > 0 else 0
    bar_len = 30
    filled = int(bar_len * pct / 100)
    bar = "█" * filled + "░" * (bar_len - filled)
    prefix_str = f"{prefix} | " if prefix else ""
    return f"{prefix_str}[{bar}] {pct}% ({current}/{total})"

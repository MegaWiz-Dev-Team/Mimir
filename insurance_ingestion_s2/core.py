"""Design system: shared patterns for pipeline (logging, config, errors, types)."""

import json
import logging
import sys
import os
from dataclasses import dataclass
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
    COMPLIANCE = "6_compliance"  # S2 new phase


class ProductStatus(str, Enum):
    """Product lifecycle status (S2: temporal filtering)."""
    ACTIVE = "active"  # Selling now
    DISCONTINUED = "discontinued"  # Stopped selling, kept for history
    ARCHIVED = "archived"  # Old version, new version available
    SUNSET = "sunset"  # Phasing out, deadline approaching
    PLANNED = "planned"  # Not yet launched


class LogLevel(str, Enum):
    """Pipeline logging levels with UX-friendly output."""
    DEBUG = "DEBUG"
    INFO = "INFO"
    SUCCESS = "SUCCESS"
    WARNING = "WARNING"
    ERROR = "ERROR"


class PipelineConfig(BaseModel):
    """Immutable configuration for all phases (S1 + S2 features)."""
    tenant_id: str = "asgard_insurance"
    mimir_base_url: str = Field(default_factory=lambda: os.getenv("MIMIR_BASE_URL", "http://localhost:8000"))
    mimir_ingest_endpoint: str = "/api/ingest"
    mimir_entity_endpoint: str = "/api/entities"
    neo4j_uri: str = Field(default_factory=lambda: os.getenv("NEO4J_URI", "bolt://localhost:7687"))
    neo4j_user: str = "neo4j"
    neo4j_password: str = Field(default="", description="Read from env: NEO4J_PASSWORD")
    qdrant_url: str = Field(default_factory=lambda: os.getenv("QDRANT_URL", "http://localhost:6333"))
    qdrant_collection: str = "insurance_products"
    embeddings_model: str = "bge-m3"
    embeddings_endpoint: str = Field(default_factory=lambda: os.getenv("EMBEDDINGS_ENDPOINT", "http://localhost:8001/embed"))
    source_base_dir: Path = Path("./data/sources")
    output_dir: Path = Path("./data/output")
    batch_size: int = 100
    test_mode: bool = False

    # S2: Multi-insurer support
    multi_insurer_enabled: bool = True
    default_insurer_id: str = "insurer_001"  # Prudential for S1 compat
    insurer_dedup_enabled: bool = True

    # S2: Thai language support
    language: str = "en"  # en, th, bi (bilingual)
    thai_nlp_endpoint: str = "http://localhost:9001"  # Thai NER service
    thai_tokenizer: str = "pythainlp"  # pythainlp, fasttext

    # S2: File upload support
    upload_dir: Path = Path("./data/uploads")
    supported_formats: list[str] = ["pdf", "docx", "txt", "jpg", "png", "jpeg"]
    max_file_size_mb: int = 50

    # S2: OCR (Syn integration)
    ocr_enabled: bool = True
    syn_endpoint: str = "http://localhost:9002/ocr"  # Syn OCR service
    ocr_confidence_threshold: float = 0.8

    # S2: QA layer
    qa_enabled: bool = False  # Phase 5 variant
    rerank_enabled: bool = False

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
    """Single document chunk for ingestion (S1 + S2 fields with hierarchical filtering + temporal)."""
    source_id: str
    content: str
    metadata: dict
    chunk_index: int = 0
    tokens: int = 0
    insurer_id: str = "insurer_001"  # S2: Multi-insurer (required for isolation)
    language: str = "en"  # S2: Language tag (en, th, bi)
    source_type: str = "url"  # Where data came from: url, upload, pdf, ocr, docx
    product_type: str = "health"  # S2: Product category: health, life, savings, investment
    channel: str = "direct"  # S2: Distribution channel: direct, uob, ttb, cimb, broker, agent
    product_name: str = ""  # S2: Product name (e.g., "PRU Mao Mao Double Sure")
    product_version: str = "1.0"  # S2: Product version (e.g., "1.0", "2.0", "2.1")
    product_launch_date: str = ""  # S2: ISO date when product launched (e.g., "2020-01-15")
    product_end_date: Optional[str] = None  # S2: ISO date when product ended (null = still active)
    is_active: bool = True  # S2: Derived from today check (current_date < launch vs > end)
    status: str = "active"  # S2: Lifecycle status (active, discontinued, archived, sunset, planned)

    def to_jsonl(self) -> str:
        """Convert to JSONL format for Mimir (with hierarchical filters + temporal)."""
        return json.dumps({
            "source_id": self.source_id,
            "content": self.content,
            "insurer_id": self.insurer_id,  # Filter: insurer isolation
            "product_type": self.product_type,  # Filter: product category
            "channel": self.channel,  # Filter: distribution channel
            "language": self.language,  # Filter: language
            "product_name": self.product_name,  # Filter: product name
            "product_version": self.product_version,  # Filter: version tracking
            "product_launch_date": self.product_launch_date,  # Filter: temporal (range)
            "product_end_date": self.product_end_date,  # Filter: temporal (range)
            "is_active": self.is_active,  # Filter: boolean (current products)
            "status": self.status,  # Filter: lifecycle (active/discontinued/archived/sunset/planned)
            "metadata": {
                **self.metadata,
                "insurer_id": self.insurer_id,
                "product_type": self.product_type,
                "channel": self.channel,
                "language": self.language,
                "product_name": self.product_name,
                "product_version": self.product_version,
                "product_launch_date": self.product_launch_date,
                "product_end_date": self.product_end_date,
                "is_active": self.is_active,
                "status": self.status,
                "source_type": self.source_type,
            },
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

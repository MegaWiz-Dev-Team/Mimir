"""Pytest configuration and fixtures for E2E tests."""

import pytest
import json
import tempfile
from pathlib import Path
from unittest.mock import Mock, MagicMock, patch
from datetime import datetime

from insurance_ingestion_s2.core import (
    PipelineConfig, Chunk, Entity, Phase, PipelineLogger
)


@pytest.fixture
def temp_dir():
    """Temporary directory for test outputs."""
    with tempfile.TemporaryDirectory() as tmpdir:
        yield Path(tmpdir)


@pytest.fixture
def e2e_config(temp_dir):
    """Pipeline config for E2E tests."""
    return PipelineConfig(
        tenant_id="asgard_insurance_test",
        mimir_base_url="http://localhost:8000",
        qdrant_url="http://localhost:6333",
        neo4j_uri="bolt://localhost:7687",
        output_dir=temp_dir / "output",
        upload_dir=temp_dir / "uploads",
        test_mode=True,
    )


@pytest.fixture
def mock_mimir_client():
    """Mock Mimir API client."""
    client = MagicMock()
    client.ingest = MagicMock(return_value={"status": "success", "ingested": 10})
    client.search = MagicMock(return_value=[
        {"source_id": "chunk_1", "score": 0.95, "content": "health insurance"},
        {"source_id": "chunk_2", "score": 0.87, "content": "coverage"},
    ])
    client.update_chunk_metadata = MagicMock(return_value={"updated": True})
    return client


@pytest.fixture
def mock_qdrant_client():
    """Mock Qdrant API client."""
    client = MagicMock()
    client.upsert = MagicMock(return_value={"points_count": 10})
    client.search = MagicMock(return_value=[
        {"id": "vec_1", "score": 0.95, "payload": {"source_id": "chunk_1"}},
        {"id": "vec_2", "score": 0.87, "payload": {"source_id": "chunk_2"}},
    ])
    client.set_payload = MagicMock(return_value={"updated": 10})
    return client


@pytest.fixture
def mock_neo4j_driver():
    """Mock Neo4j driver."""
    driver = MagicMock()
    session = MagicMock()
    driver.session = MagicMock(return_value=session)

    # Mock execute_write for queries
    session.execute_write = MagicMock(return_value=10)

    return driver


@pytest.fixture
def sample_chunks_insurer_001():
    """Sample chunks for Prudential (insurer_001)."""
    return [
        Chunk(
            source_id="url_insurer_001_health_0",
            content="PRU Mao Mao Double Sure is a comprehensive health insurance plan covering hospitalization up to THB 2,000,000 per year.",
            metadata={
                "source_url": "https://prudential.co.th/en/products/health/",
                "document_type": "product_catalog",
                "language": "en",
                "vendor": "VENDOR_INSURANCE_001",
                "source_type": "url",
            },
            chunk_index=0,
            tokens=285,
            insurer_id="insurer_001",
            language="en",
            source_type="url",
            product_type="health",
            channel="direct",
            product_name="PRU Mao Mao Double Sure",
            product_version="2.0",
            product_launch_date="2020-01-15",
            product_end_date=None,
            is_active=True,
            status="active",
        ),
        Chunk(
            source_id="url_insurer_001_health_1",
            content="Exclusions: Cosmetic procedures, experimental treatments, dental care, pre-existing conditions within 12 months.",
            metadata={
                "source_url": "https://prudential.co.th/en/products/health/",
                "document_type": "product_catalog",
                "language": "en",
                "vendor": "VENDOR_INSURANCE_001",
                "source_type": "url",
            },
            chunk_index=1,
            tokens=250,
            insurer_id="insurer_001",
            language="en",
            source_type="url",
            product_type="health",
            channel="direct",
            product_name="PRU Mao Mao Double Sure",
            product_version="2.0",
            product_launch_date="2020-01-15",
            product_end_date=None,
            is_active=True,
            status="active",
        ),
    ]


@pytest.fixture
def sample_chunks_insurer_002():
    """Sample chunks for AXA (insurer_002)."""
    return [
        Chunk(
            source_id="url_insurer_002_health_0",
            content="AXA Health Plus offers comprehensive medical coverage with flexible deductible options and 24/7 customer support.",
            metadata={
                "source_url": "https://axa.co.th/en/products/health/",
                "document_type": "product_catalog",
                "language": "en",
                "vendor": "VENDOR_INSURANCE_002",
                "source_type": "url",
            },
            chunk_index=0,
            tokens=295,
            insurer_id="insurer_002",
            language="en",
            source_type="url",
            product_type="health",
            channel="direct",
            product_name="AXA Health Plus",
            product_version="1.0",
            product_launch_date="2019-06-01",
            product_end_date=None,
            is_active=True,
            status="active",
        ),
    ]


@pytest.fixture
def sample_chunks_thai():
    """Sample chunks for Thai language (AXA Thai)."""
    return [
        Chunk(
            source_id="url_insurer_002_health_th_0",
            content="แผน AXA สุขภาพ พลัส ให้ความคุ้มครองสำหรับการรักษาพยาบาลแบบครอบคลุม รวมถึงการนอนโรงพยาบาล การผ่าตัด",
            metadata={
                "source_url": "https://axa.co.th/th/products/health/",
                "document_type": "product_catalog",
                "language": "th",
                "vendor": "VENDOR_INSURANCE_002",
                "source_type": "url",
            },
            chunk_index=0,
            tokens=280,
            insurer_id="insurer_002",
            language="th",
            source_type="url",
            product_type="health",
            channel="direct",
            product_name="AXA สุขภาพ พลัส",
            product_version="1.0",
            product_launch_date="2019-06-01",
            product_end_date=None,
            is_active=True,
            status="active",
        ),
    ]


@pytest.fixture
def sample_chunks_discontinued():
    """Sample chunks for discontinued product."""
    return [
        Chunk(
            source_id="url_insurer_001_health_discontinued_0",
            content="PRU Mao Mao v1.0 (legacy version, no longer available). This product has been replaced by v2.0.",
            metadata={
                "source_url": "https://prudential.co.th/en/products/health/archive/",
                "document_type": "product_catalog",
                "language": "en",
                "vendor": "VENDOR_INSURANCE_001",
                "source_type": "url",
            },
            chunk_index=0,
            tokens=245,
            insurer_id="insurer_001",
            language="en",
            source_type="url",
            product_type="health",
            channel="direct",
            product_name="PRU Mao Mao Double Sure",
            product_version="1.0",
            product_launch_date="2018-06-01",
            product_end_date="2020-01-14",
            is_active=False,
            status="discontinued",
        ),
    ]


@pytest.fixture
def sample_entities():
    """Sample entities extracted from chunks."""
    return [
        Entity(
            entity_id="PROD_001",
            name="PRU Mao Mao Double Sure",
            entity_type="Product",
            properties={
                "insurer_id": "insurer_001",
                "product_type": "health",
                "version": "2.0",
            },
            source_ids=["url_insurer_001_health_0", "url_insurer_001_health_1"],
        ),
        Entity(
            entity_id="COV_001",
            name="Hospitalization Coverage",
            entity_type="Coverage",
            properties={
                "insurer_id": "insurer_001",
                "product_id": "PROD_001",
                "limit": "2000000 THB",
            },
            source_ids=["url_insurer_001_health_0"],
        ),
        Entity(
            entity_id="EXC_001",
            name="Cosmetic Procedures",
            entity_type="Exclusion",
            properties={
                "insurer_id": "insurer_001",
                "product_id": "PROD_001",
            },
            source_ids=["url_insurer_001_health_1"],
        ),
    ]


@pytest.fixture
def mock_http_responses():
    """Mock HTTP responses for URL extraction."""
    responses = {
        "health_success": {
            "status_code": 200,
            "content": b"""
                <html>
                    <title>Health Insurance Products</title>
                    <p>PRU Mao Mao Double Sure covers hospitalization</p>
                    <p>Daily room benefit is THB 6,000</p>
                </html>
            """,
        },
        "timeout": {"exception": TimeoutError("Connection timeout")},
        "404": {"status_code": 404, "content": b"<html>Not Found</html>"},
        "500": {"status_code": 500, "content": b"<html>Server Error</html>"},
        "malformed": {"content": b"<p>No closing tags"},
        "empty": {"content": b""},
    }
    return responses


@pytest.fixture
def logger():
    """Pipeline logger for tests."""
    return PipelineLogger(Phase.EXTRACTION, quiet=True)


# ============================================================================
# Helper functions
# ============================================================================

def create_temp_jsonl(path: Path, chunks: list) -> Path:
    """Create temporary JSONL file with chunks."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w") as f:
        for chunk in chunks:
            f.write(chunk.to_jsonl() + "\n")
    return path


def read_jsonl(path: Path) -> list:
    """Read JSONL file and return list of dicts."""
    data = []
    if path.exists():
        with open(path, "r") as f:
            for line in f:
                if line.strip():
                    data.append(json.loads(line))
    return data


def assert_chunk_fields_complete(chunk_dict: dict) -> None:
    """Assert that chunk has all required fields."""
    required_fields = [
        "source_id", "content", "insurer_id", "product_type", "channel",
        "product_name", "product_version", "product_launch_date",
        "product_end_date", "is_active", "status", "language"
    ]
    for field in required_fields:
        assert field in chunk_dict, f"Missing field: {field}"


def assert_no_data_leakage(results: list, expected_insurer_id: str) -> None:
    """Assert that results don't contain data from other insurers."""
    for result in results:
        insurer_id = result.get("insurer_id") or result.get("metadata", {}).get("insurer_id")
        assert insurer_id == expected_insurer_id, \
            f"Data leakage: expected {expected_insurer_id}, got {insurer_id}"

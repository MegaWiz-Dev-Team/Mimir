"""Pytest configuration and shared fixtures."""

import pytest
from pathlib import Path
from unittest.mock import Mock, patch

from insurance_ingestion.core import PipelineConfig, Chunk, Entity
from tests.fixtures.sample_data import SAMPLE_CHUNKS, SAMPLE_ENTITIES, SAMPLE_CONFIG


@pytest.fixture
def temp_output_dir(tmp_path):
    """Temporary directory for pipeline output."""
    return tmp_path


@pytest.fixture
def config_test(temp_output_dir):
    """Test configuration with temp output directory."""
    config = SAMPLE_CONFIG.copy()
    config.output_dir = temp_output_dir
    config.source_base_dir = Path(__file__).parent / "fixtures" / "data"
    return config


@pytest.fixture
def sample_chunks():
    """Fixture: sample chunks for testing."""
    return SAMPLE_CHUNKS


@pytest.fixture
def sample_entities():
    """Fixture: sample entities for testing."""
    return SAMPLE_ENTITIES


@pytest.fixture
def mock_mimir_client():
    """Mock Mimir API client."""
    with patch("insurance_ingestion.phases.phase4_ingestion.requests.post") as mock:
        mock.return_value.json.return_value = {
            "status": "success",
            "ingested": 100,
        }
        yield mock


@pytest.fixture
def mock_embeddings_endpoint():
    """Mock Heimdall embeddings endpoint."""
    with patch("insurance_ingestion.phases.phase4_ingestion.requests.post") as mock:
        mock.return_value.json.return_value = {
            "embeddings": [
                [0.1] * 1024 for _ in range(100)  # BGE-M3 dimension
            ],
        }
        yield mock


@pytest.mark.integration
def pytest_configure(config):
    """Configure pytest markers."""
    config.addinivalue_line("markers", "integration: integration tests (require services)")
    config.addinivalue_line("markers", "slow: slow tests")


# CLI flag: pytest -m integration (only integration tests)
#           pytest -m "not integration" (skip integration tests)

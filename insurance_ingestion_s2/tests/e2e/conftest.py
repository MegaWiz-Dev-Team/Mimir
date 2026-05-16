"""E2E test configuration - re-exports fixtures and helpers from parent conftest."""

import sys
from pathlib import Path

# Add parent tests directory to path
sys.path.insert(0, str(Path(__file__).parent.parent))

# Import all fixtures from parent conftest
from conftest import (
    temp_dir,
    e2e_config,
    mock_mimir_client,
    mock_qdrant_client,
    mock_neo4j_driver,
    sample_chunks_insurer_001,
    sample_chunks_insurer_002,
    sample_chunks_thai,
    sample_chunks_discontinued,
    sample_entities,
    logger,
    create_temp_jsonl,
    read_jsonl,
    assert_chunk_fields_complete,
    assert_no_data_leakage,
)

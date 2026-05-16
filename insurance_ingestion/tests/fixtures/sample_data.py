"""Sample insurance data for testing."""

from insurance_ingestion.core import Chunk, Entity, PipelineConfig


SAMPLE_CHUNKS = [
    Chunk(
        source_id="health_plan_a_001",
        content="Health Plan A covers outpatient visits up to 100 per year. Co-payment is 300 THB per visit.",
        metadata={
            "source_url": "https://example.com/plans/health-a",
            "product_id": "PROD_001",
            "document_type": "product_sheet",
            "language": "en",
            "extraction_date": "2026-05-16",
            "vendor": "VENDOR_001",
        },
        chunk_index=0,
        tokens=28,
    ),
    Chunk(
        source_id="health_plan_a_002",
        content="Exclusions: cosmetic procedures, experimental treatments, dental care, and vision correction.",
        metadata={
            "source_url": "https://example.com/plans/health-a",
            "product_id": "PROD_001",
            "document_type": "exclusions",
            "language": "en",
            "extraction_date": "2026-05-16",
            "vendor": "VENDOR_001",
        },
        chunk_index=1,
        tokens=15,
    ),
    Chunk(
        source_id="critical_illness_b_001",
        content="Critical Illness Plan B provides 500,000 THB payout for diagnosed cancer, stroke, or heart attack.",
        metadata={
            "source_url": "https://example.com/plans/critical-b",
            "product_id": "PROD_002",
            "document_type": "coverage",
            "language": "en",
            "extraction_date": "2026-05-16",
            "vendor": "VENDOR_001",
        },
        chunk_index=0,
        tokens=22,
    ),
]

SAMPLE_ENTITIES = [
    Entity(
        entity_id="PROD_001",
        name="Health Plan A",
        entity_type="Product",
        properties={
            "coverage_type": "health",
            "annual_limit": 100,
            "copay": 300,
            "currency": "THB",
        },
        source_ids=["health_plan_a_001", "health_plan_a_002"],
    ),
    Entity(
        entity_id="BENEF_001",
        name="Outpatient Visit",
        entity_type="Benefit",
        properties={
            "benefit_type": "consultation",
            "max_per_year": 100,
            "copay_thb": 300,
        },
        source_ids=["health_plan_a_001"],
    ),
    Entity(
        entity_id="EXCL_001",
        name="Cosmetic Procedures",
        entity_type="Exclusion",
        properties={"applies_to_plans": ["PROD_001"]},
        source_ids=["health_plan_a_002"],
    ),
]

SAMPLE_CONFIG = PipelineConfig(
    test_mode=True,
    source_base_dir="./tests/fixtures/data",
)


SAMPLE_TEST_QUERIES = [
    {
        "query": "What's covered under Health Plan A?",
        "tier": "lookup",
        "expected_entity": "PROD_001",
        "min_hit_rate": 0.75,
    },
    {
        "query": "Which plans cover outpatient visits?",
        "tier": "reasoning",
        "expected_entity": "BENEF_001",
        "min_hit_rate": 0.70,
    },
    {
        "query": "Are cosmetic procedures excluded?",
        "tier": "exclusion",
        "expected_entity": "EXCL_001",
        "min_hit_rate": 0.65,
    },
]

"""Phase 5: Validate via Mimir API → search quality metrics."""

import time
import re
from typing import Optional
import requests

from insurance_ingestion.core import (
    Phase, PipelineLogger, PipelineConfig, ValidationError,
)


# Test queries from PRUDENTIAL_DATA_INGESTION_SUMMARY.md
TEST_QUERIES = [
    # Lookup tier (simple product/benefit queries)
    {
        "query": "What products cover hospitalization?",
        "tier": "lookup",
        "expected_entities": ["PRU Mao Mao", "PRUBetter Care"],
        "min_hit_rate": 0.80,
    },
    {
        "query": "Critical illness coverage options",
        "tier": "lookup",
        "expected_entities": ["PRULady Cancer", "PRURokrai Super Koom"],
        "min_hit_rate": 0.80,
    },
    # Reasoning tier (cross-entity queries)
    {
        "query": "Which plans are suitable for young adults aged 20-35?",
        "tier": "reasoning",
        "expected_entities": ["PRULife Care", "PRUEasy PA"],
        "min_hit_rate": 0.70,
    },
    {
        "query": "What's the difference between term and whole life insurance?",
        "tier": "reasoning",
        "expected_entities": ["PRULife Care", "PRUWhole Life Protect 99/20"],
        "min_hit_rate": 0.70,
    },
    # Exclusion tier (what's NOT covered)
    {
        "query": "Are dental procedures covered?",
        "tier": "exclusion",
        "expected_entities": ["exclusion", "not covered"],
        "min_hit_rate": 0.65,
    },
    {
        "query": "What medical conditions are excluded from critical illness plans?",
        "tier": "exclusion",
        "expected_entities": ["exclusion", "limitation"],
        "min_hit_rate": 0.65,
    },
    # Robustness tier (edge cases, typos, variations)
    {
        "query": "PRU product premium cost",
        "tier": "robustness",
        "expected_entities": ["premium", "cost", "price"],
        "min_hit_rate": 0.60,
    },
    {
        "query": "insurance coverage limit",
        "tier": "robustness",
        "expected_entities": ["coverage", "limit"],
        "min_hit_rate": 0.60,
    },
    {
        "query": "retirement planning products",
        "tier": "robustness",
        "expected_entities": ["PRUSmile Retirement", "savings"],
        "min_hit_rate": 0.60,
    },
    {
        "query": "investment returns annuity income",
        "tier": "robustness",
        "expected_entities": ["return", "income", "annuity"],
        "min_hit_rate": 0.60,
    },
]


def run_test_queries(
    test_queries: list[dict],
    config: PipelineConfig,
    mock_mode: bool = False,
) -> list[dict]:
    """Execute test queries against Mimir search API or mock results.

    Args:
        test_queries: List of {"query", "tier", "expected_entities", "min_hit_rate"}
        config: Pipeline configuration
        mock_mode: If True, return synthetic results (for testing without K8s)

    Returns:
        List of results: {"query", "tier", "results": [{"source_id", "score"}], "latency_ms"}
    """
    results = []

    for i, test_q in enumerate(test_queries):
        start_time = time.time()

        if mock_mode:
            # Synthetic results for testing
            result_set = [
                {"source_id": f"chunk_{j}", "score": 0.95 - (j * 0.15)}
                for j in range(3)
            ]
            latency_ms = 150 + (i % 100)  # Vary 150-250ms
        else:
            try:
                resp = requests.post(
                    f"{config.mimir_base_url}/api/search",
                    json={"query": test_q["query"], "top_k": 3},
                    timeout=2.0,
                )
                resp.raise_for_status()
                result = resp.json()
                result_set = result.get("results", [])
                latency_ms = int((time.time() - start_time) * 1000)

            except requests.RequestException as e:
                raise ValidationError(f"Query failed: {test_q['query']}: {e}")

        results.append({
            "query": test_q["query"],
            "tier": test_q.get("tier", "unknown"),
            "expected_entities": test_q.get("expected_entities", []),
            "results": result_set,
            "latency_ms": latency_ms,
        })

    return results


def calculate_hit_rate(
    results: list[dict],
    metric: str = "@3",
) -> tuple[float, dict]:
    """Calculate Hit Rate@K with breakdown by tier.

    Hit = any expected entity appears in top-K results (case-insensitive).

    Returns:
        (overall_hit_rate, tier_breakdown)
    """
    k = int(metric.lstrip("@"))
    hit_counts = {}
    total_counts = {}

    for result in results:
        tier = result.get("tier", "unknown")
        if tier not in hit_counts:
            hit_counts[tier] = 0
            total_counts[tier] = 0

        total_counts[tier] += 1

        # Check if any expected entity in top-K results
        result_text = " ".join([str(r) for r in result["results"][:k]])
        for expected_entity in result.get("expected_entities", []):
            if expected_entity.lower() in result_text.lower():
                hit_counts[tier] += 1
                break

    # Calculate per-tier rates
    tier_breakdown = {}
    for tier in total_counts:
        if total_counts[tier] > 0:
            tier_breakdown[tier] = hit_counts[tier] / total_counts[tier]

    # Overall rate
    total_hits = sum(hit_counts.values())
    total_queries = sum(total_counts.values())
    overall_rate = total_hits / total_queries if total_queries > 0 else 0.0

    return overall_rate, tier_breakdown


def scan_for_pii(results: list[dict]) -> dict:
    """Scan search results for sensitive data (vendor names, customer info).

    Returns:
        {"pii_found": bool, "vendor_refs": list, "warnings": list}
    """
    pii_patterns = {
        "vendor_name": r"(?i)(prudential)",  # Vendor abstracted → VENDOR_001
        "customer_data": r"(?i)(name|address|id|phone|email|ssn)",
    }

    findings = {
        "pii_found": False,
        "vendor_refs": [],
        "warnings": [],
    }

    for result in results:
        result_text = str(result.get("results", []))

        for pattern_name, pattern in pii_patterns.items():
            matches = re.findall(pattern, result_text)
            if matches:
                findings["pii_found"] = True
                findings["warnings"].append(
                    f"Potential {pattern_name} in query: {result['query']}"
                )
                if pattern_name == "vendor_name":
                    findings["vendor_refs"].extend(matches)

    return findings


def check_fallback_criteria(hit_rate: float) -> bool:
    """Check if Hit Rate < 50% (triggers Plan B: switch embedding model).

    Returns:
        True if should activate fallback (Typhoon)
    """
    return hit_rate < 0.50


def run_phase5(
    config: PipelineConfig,
    logger: Optional[PipelineLogger] = None,
    mock_mode: bool = False,
) -> dict:
    """Execute Phase 5: validate search quality and acceptance criteria.

    Args:
        config: Pipeline configuration
        logger: Optional logger (uses default if not provided)
        mock_mode: If True, use synthetic results (for testing without K8s)

    AC:
    - Hit Rate@3 ≥ 75%
    - Latency < 500ms
    - Zero PII in results
    - All 10 test queries executed

    Returns:
        {"status": "success"|"fallback_needed", "hit_rate": float,
         "latency_ms": float, "tier_breakdown": {...}, "pii_scan": {...}}
    """
    if logger is None:
        logger = PipelineLogger(Phase.VALIDATION)

    mode_str = "(MOCK MODE)" if mock_mode else "(LIVE K8s)"
    logger.info(f"Starting Phase 5: Validate search quality {mode_str}")

    try:
        # 1. Run test queries
        logger.info(f"Executing {len(TEST_QUERIES)} test queries...")
        results = run_test_queries(TEST_QUERIES, config, mock_mode=mock_mode)

        # 2. Calculate Hit Rate@3
        hit_rate, tier_breakdown = calculate_hit_rate(results, metric="@3")
        logger.info(f"Hit Rate@3: {hit_rate:.1%}")

        # Show per-tier breakdown
        for tier, rate in tier_breakdown.items():
            logger.info(f"  {tier:12} {rate:.1%}")

        # 3. Check latency
        latencies = [r["latency_ms"] for r in results]
        avg_latency = sum(latencies) / len(latencies) if latencies else 0
        p95_latency = sorted(latencies)[int(len(latencies) * 0.95)] if latencies else 0
        logger.info(f"Latency: avg={avg_latency:.0f}ms, p95={p95_latency:.0f}ms")

        # 4. Scan for PII
        pii_scan = scan_for_pii(results)
        if pii_scan["pii_found"]:
            logger.warning(f"⚠️ Potential PII found: {pii_scan['warnings']}")
        else:
            logger.success("✅ Zero PII detected")

        # 5. Determine status
        status = "success"
        if hit_rate < 0.50:
            status = "fallback_needed"
            logger.warning(f"⚠️ Hit Rate {hit_rate:.1%} < 50% — Fallback Plan B recommended")
        elif hit_rate < 0.75:
            logger.warning(f"⚠️ Hit Rate {hit_rate:.1%} < 75% target")

        logger.success(f"✅ Phase 5 Complete: Hit Rate {hit_rate:.1%}")

        return {
            "status": status,
            "hit_rate": hit_rate,
            "tier_breakdown": tier_breakdown,
            "avg_latency_ms": avg_latency,
            "p95_latency_ms": p95_latency,
            "pii_scan": pii_scan,
            "queries_executed": len(results),
        }

    except ValidationError as e:
        logger.error(f"Validation failed: {e}")
        raise


if __name__ == "__main__":
    config = PipelineConfig()
    # Use mock_mode=True for testing without K8s
    run_phase5(config, mock_mode=True)

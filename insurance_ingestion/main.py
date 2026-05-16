#!/usr/bin/env python3
"""Sprint 1 Insurance Product Knowledge Ingestion — Main Orchestrator.

Usage:
    python main.py --phase 1                    # Run Phase 1 only
    python main.py --phase 1-5                  # Run all phases (end-to-end)
    python main.py --phase 5 --skip-ingest      # Validate without re-ingesting
    python main.py --test                       # Run unit tests
"""

import sys
import argparse
from pathlib import Path
from datetime import datetime

from insurance_ingestion.core import (
    Phase, PipelineLogger, PipelineConfig, PipelineError,
)
from insurance_ingestion.phases.phase1_extraction import run_phase1
from insurance_ingestion.phases.phase2_schema import run_phase2
from insurance_ingestion.phases.phase3_entities import run_phase3
from insurance_ingestion.phases.phase4_ingestion import run_phase4
from insurance_ingestion.phases.phase5_validation import run_phase5, check_fallback_criteria


def parse_args():
    """Parse command-line arguments."""
    parser = argparse.ArgumentParser(
        description="Sprint 1 Insurance Ingestion Pipeline",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  python main.py --phase 1                    # Extract products
  python main.py --phase 1-5                  # Full pipeline
  python main.py --phase 5 --skip-ingest      # Validate existing data
  python main.py --test                       # Run pytest
  python main.py --config custom.json         # Use custom config
        """
    )
    parser.add_argument(
        "--phase",
        type=str,
        default="1-5",
        help="Phases to run: '1' or '1-5' or '2-3' (default: 1-5)",
    )
    parser.add_argument(
        "--test",
        action="store_true",
        help="Run pytest unit tests instead of pipeline",
    )
    parser.add_argument(
        "--skip-ingest",
        action="store_true",
        help="Skip Phases 1-4, only run validation (Phase 5)",
    )
    parser.add_argument(
        "--config",
        type=Path,
        help="Path to custom PipelineConfig JSON",
    )
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="Suppress progress output",
    )
    parser.add_argument(
        "--mock",
        action="store_true",
        help="Use mock/synthetic results (Phase 5 testing without K8s)",
    )
    return parser.parse_args()


def run_pipeline(
    phase_range: str = "1-5",
    config: PipelineConfig = None,
    skip_ingest: bool = False,
    quiet: bool = False,
    mock_mode: bool = False,
) -> dict:
    """Execute pipeline phases.

    Returns:
        {"status": "success"|"fallback_activated"|"error", "results": {...}}
    """
    if config is None:
        config = PipelineConfig()

    config.output_dir.mkdir(parents=True, exist_ok=True)

    # Parse phase range
    parts = phase_range.split("-")
    if len(parts) == 1:
        phases_to_run = [int(parts[0])]
    else:
        start, end = int(parts[0]), int(parts[1])
        phases_to_run = list(range(start, end + 1))

    results = {
        "started_at": datetime.now().isoformat(),
        "phase_results": {},
    }

    try:
        # Phase 1: Extraction
        if 1 in phases_to_run and not skip_ingest:
            logger1 = PipelineLogger(Phase.EXTRACTION, quiet=quiet)
            logger1.info("=" * 70)
            logger1.info("PHASE 1: EXTRACT PRODUCTS FROM WEB SOURCES")
            logger1.info("=" * 70)
            phase1_output = run_phase1(config)
            results["phase_results"]["phase1"] = {"output": str(phase1_output)}

        # Phase 2: Schema Normalization
        if 2 in phases_to_run and not skip_ingest:
            logger2 = PipelineLogger(Phase.SCHEMA, quiet=quiet)
            logger2.info("=" * 70)
            logger2.info("PHASE 2: NORMALIZE DATA SCHEMA")
            logger2.info("=" * 70)
            phase1_output = config.output_dir / "phase1_chunks.jsonl"
            phase2_output = run_phase2(phase1_output, config, logger2)
            results["phase_results"]["phase2"] = {"output": str(phase2_output)}

        # Phase 3: Entity Extraction
        if 3 in phases_to_run and not skip_ingest:
            logger3 = PipelineLogger(Phase.ENTITIES, quiet=quiet)
            logger3.info("=" * 70)
            logger3.info("PHASE 3: EXTRACT ENTITIES & BUILD KNOWLEDGE GRAPH")
            logger3.info("=" * 70)
            phase2_output = config.output_dir / "phase2_normalized.jsonl"
            entities_file, edges_file = run_phase3(phase2_output, config, logger3)
            results["phase_results"]["phase3"] = {
                "entities": str(entities_file),
                "edges": str(edges_file),
            }

        # Phase 4: Ingestion to Mimir
        if 4 in phases_to_run and not skip_ingest:
            logger4 = PipelineLogger(Phase.INGESTION, quiet=quiet)
            logger4.info("=" * 70)
            logger4.info("PHASE 4: INGEST TO MIMIR & QDRANT")
            logger4.info("=" * 70)
            phase2_output = config.output_dir / "phase2_normalized.jsonl"
            phase3_entities = config.output_dir / "phase3_entities.jsonl"
            result4 = run_phase4(phase2_output, phase3_entities, config, logger4)
            results["phase_results"]["phase4"] = result4

        # Phase 5: Validation
        if 5 in phases_to_run:
            logger5 = PipelineLogger(Phase.VALIDATION, quiet=quiet)
            logger5.info("=" * 70)
            logger5.info("PHASE 5: VALIDATE SEARCH QUALITY & ACCEPTANCE CRITERIA")
            logger5.info("=" * 70)
            result5 = run_phase5(config, logger5, mock_mode=mock_mode)
            results["phase_results"]["phase5"] = result5

            # Check fallback criteria
            hit_rate = result5.get("hit_rate", 0.0)
            if check_fallback_criteria(hit_rate):
                logger5.warning(f"Hit Rate {hit_rate:.1%} < 50% — Activating Plan B")
                results["status"] = "fallback_activated"
            else:
                results["status"] = "success"

    except PipelineError as e:
        logger = PipelineLogger(Phase.EXTRACTION)  # Generic
        logger.error(f"Pipeline failed: {e}")
        results["status"] = "error"
        results["error"] = str(e)
        return results
    except Exception as e:
        logger = PipelineLogger(Phase.EXTRACTION)  # Generic
        logger.error(f"Unexpected error: {e}")
        results["status"] = "error"
        results["error"] = str(e)
        return results

    results["completed_at"] = datetime.now().isoformat()
    return results


def main():
    """CLI entry point."""
    args = parse_args()

    if args.test:
        # Run pytest
        import pytest
        sys.exit(pytest.main([
            "insurance_ingestion/tests/unit",
            "-v",
            "--tb=short",
        ]))

    # Load custom config if provided
    config = PipelineConfig()
    if args.config:
        import json
        with open(args.config) as f:
            config_dict = json.load(f)
            config = PipelineConfig(**config_dict)

    # Run pipeline
    results = run_pipeline(
        phase_range=args.phase,
        config=config,
        skip_ingest=args.skip_ingest,
        quiet=args.quiet,
        mock_mode=args.mock,
    )

    # Print results summary
    print("\n" + "=" * 70)
    print(f"PIPELINE {results['status'].upper()}")
    print("=" * 70)
    print(f"Status: {results['status']}")
    print(f"Started: {results['started_at']}")
    if "completed_at" in results:
        print(f"Completed: {results['completed_at']}")
    if "error" in results:
        print(f"Error: {results['error']}")
    print()

    return 0 if results["status"] == "success" else 1


if __name__ == "__main__":
    sys.exit(main())

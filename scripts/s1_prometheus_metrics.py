"""
Prometheus Metrics Module for S1 Insurance Sprint

Purpose: Export metrics to Prometheus for Grafana dashboard visualization
Usage: Import and initialize metrics in extract_entities.py

Installation:
    pip install prometheus-client

Initialize (in extract_entities.py):
    from s1_prometheus_metrics import (
        init_metrics,
        chunks_counter,
        entities_counter,
        confidence_gauge,
        phase_gauge,
        relationships_gauge,
        hit_rate_gauge
    )

    # Start metrics HTTP server (port 8000)
    init_metrics()

During extraction:
    chunks_counter.inc()  # Each chunk extracted
    entities_counter.inc(5)  # Each 5 entities found
    confidence_gauge.set(0.76)  # Average confidence
    phase_gauge.set(1)  # Current phase (1=extract, 2=chunk, etc.)
"""

from prometheus_client import Counter, Gauge, start_http_server
import threading


# Define metrics
chunks_counter = Counter(
    's1_chunks_extracted_total',
    'Total chunks successfully extracted from sources',
    labelnames=['phase']
)

entities_counter = Counter(
    's1_entities_found_total',
    'Total entities extracted from chunks',
    labelnames=['type']  # type: product, coverage, exclusion, condition
)

confidence_gauge = Gauge(
    's1_avg_confidence_gauge',
    'Average confidence score of extracted entities (0.0-1.0)'
)

phase_gauge = Gauge(
    's1_current_phase_gauge',
    'Current S1 phase (1=Extract, 2=Chunk, 3=Entities, 4=Embed)'
)

relationships_gauge = Gauge(
    's1_neo4j_relationships_total',
    'Total relationships created in Neo4j graph'
)

hit_rate_gauge = Gauge(
    's1_hit_rate_3_gauge',
    'Hit Rate@3 percentage from May 22 validation (0-100)'
)

duplicate_chunks_gauge = Gauge(
    's1_duplicate_chunks_total',
    'Number of chunks removed by deduplication'
)

embeddings_counter = Counter(
    's1_embeddings_total',
    'Total chunks embedded into Qdrant vector store'
)


# Initialize metrics server (call once at startup)
_metrics_initialized = False


def init_metrics(port: int = 8000) -> None:
    """
    Initialize Prometheus metrics HTTP server.

    Args:
        port: Port to expose metrics on (default 8000)

    Example:
        init_metrics()  # Exposes metrics at http://localhost:8000/metrics
    """
    global _metrics_initialized

    if _metrics_initialized:
        return

    try:
        # Start HTTP server in background thread
        start_http_server(port)
        _metrics_initialized = True
        print(f"✅ Prometheus metrics server started on port {port}")
        print(f"📊 Metrics available at http://localhost:{port}/metrics")
    except Exception as e:
        print(f"⚠️ Could not start metrics server: {e}")
        print("  Metrics will not be exported to Prometheus")


# Helper functions for common operations
def set_extraction_phase(phase: int) -> None:
    """Set current S1 phase (1=Extract, 2=Chunk, 3=Entities, 4=Embed)."""
    phase_gauge.set(phase)


def increment_chunks(count: int = 1) -> None:
    """Increment chunks extracted counter."""
    chunks_counter.labels(phase='extraction').inc(count)


def increment_entities(count: int = 1, entity_type: str = 'unknown') -> None:
    """Increment entities found counter."""
    entities_counter.labels(type=entity_type).inc(count)


def update_confidence(avg_confidence: float) -> None:
    """Update average confidence gauge (0.0-1.0)."""
    if 0.0 <= avg_confidence <= 1.0:
        confidence_gauge.set(avg_confidence)
    else:
        print(f"⚠️ Invalid confidence value: {avg_confidence} (expected 0.0-1.0)")


def update_hit_rate(hit_rate_pct: float) -> None:
    """Update Hit Rate@3 gauge (0-100)."""
    if 0 <= hit_rate_pct <= 100:
        hit_rate_gauge.set(hit_rate_pct)
    else:
        print(f"⚠️ Invalid hit rate: {hit_rate_pct} (expected 0-100)")


def update_neo4j_relationships(count: int) -> None:
    """Update Neo4j relationships count."""
    relationships_gauge.set(count)


def update_duplicate_chunks(count: int) -> None:
    """Update count of chunks removed by deduplication."""
    duplicate_chunks_gauge.set(count)


def increment_embeddings(count: int = 1) -> None:
    """Increment embedded chunks counter."""
    embeddings_counter.inc(count)


# Example usage (for testing)
if __name__ == "__main__":
    import time

    print("🧪 Testing Prometheus metrics...")

    # Initialize metrics server
    init_metrics(port=8000)

    # Simulate extraction phase
    set_extraction_phase(1)
    for i in range(10):
        increment_chunks()
        time.sleep(0.1)

    update_confidence(0.76)
    update_hit_rate(0.0)  # Not yet measured
    set_extraction_phase(2)

    print("✅ Metrics updated. Check http://localhost:8000/metrics")
    print("\nSample output:")
    print("  s1_chunks_extracted_total{phase=\"extraction\"} 10.0")
    print("  s1_avg_confidence_gauge 0.76")
    print("  s1_current_phase_gauge 2.0")
    print("\nKeep this script running to serve metrics...")

    # Keep server running
    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        print("\n👋 Metrics server stopped")

# RefGraph 🦀

**Multi-domain Data Consolidation Engine for Asgard RAG**

Pure Rust implementation of semantic graph consolidation for insurance, medical, legal, and finance domains.

## Features

✅ **Semantic Graph Building** — Entity relationships and Neo4j integration  
✅ **Intelligent Deduplication** — Jaccard similarity-based entity merging  
✅ **Manifest-Based Configuration** — Domain-specific extraction rules  
✅ **Entity Extraction** — NER with confidence scoring  
✅ **Compressed References** — Efficient source tracking (pageIndex + position)  
✅ **Mimir Integration** — JSON/JSONL output for RAG ingestion  

## Architecture

```
Raw Chunks (from web)
        ↓
[EXTRACT] EntityExtractor
        ↓
[DEDUP] Deduplicator (Jaccard 0.95)
        ↓
[GRAPH] SemanticGraph builder
        ↓
[OUTPUT] MimirOutput (JSON/JSONL)
        ↓
Mimir RAG Service
```

## Module Structure

| Module | Purpose |
|--------|---------|
| `lib.rs` | Main pipeline coordinator |
| `types.rs` | Shared data structures (Entity, Relationship, etc) |
| `error.rs` | Error types and handling |
| `graph.rs` | Semantic graph builder + Neo4j relationships |
| `dedup.rs` | Jaccard similarity deduplication |
| `manifest.rs` | Domain configuration (rules, thresholds, sources) |
| `extract.rs` | Entity extraction (NER patterns) |
| `mimir.rs` | Output formatter for Mimir RAG |
| `main.rs` | CLI interface |

## Quick Start

### Build

```bash
cd refgraph-rs
cargo build --release
```

### Run Tests

```bash
# Unit tests
cargo test

# Integration tests (via CLI)
./target/release/refgraph --test
```

### Basic Usage

```bash
# Consolidate chunks
./target/release/refgraph \
  --domain insurance \
  --input raw_chunks.jsonl \
  --output consolidated.json

# With JSONL output
./target/release/refgraph \
  --domain insurance \
  --input raw_chunks.jsonl \
  --output consolidated.json \
  --jsonl consolidated.jsonl
```

## Input Format (JSONL)

```json
{
  "chunk_id": "chunk_001",
  "content": "PRU Critical Illness covers Heart Attack and Stroke",
  "source_url": "prudential.co.th/product",
  "page_index": 1,
  "token_count": 10
}
```

## Output Format

### JSON

```json
{
  "entities": [
    {
      "id": "ent_001",
      "text": "Critical Illness",
      "entity_type": "product",
      "confidence": 0.95,
      "sources": ["prudential.co.th/product"],
      "merged_from": [],
      "compressed_refs": [],
      "domain": "insurance"
    }
  ],
  "relationships": [
    {
      "source_id": "ent_001",
      "target_id": "ent_002",
      "relationship_type": "HAS_COVERAGE",
      "confidence": 0.92,
      "properties": {}
    }
  ],
  "metadata": {
    "domain": "insurance",
    "timestamp": "2026-05-16T14:00:00Z",
    "entity_count": 123,
    "relationship_count": 456,
    "average_confidence": 0.85,
    "version": "0.1.0"
  }
}
```

### JSONL

One entity/relationship per line (for streaming):

```jsonl
{"type":"metadata","data":{...}}
{"type":"entity","data":{...}}
{"type":"relationship","data":{...}}
```

## Domain Configurations

### Insurance (Default)

```rust
let config = ManifestConfig::insurance();
```

Entity thresholds:
- **product**: 0.85
- **coverage**: 0.80
- **exclusion**: 0.75
- **condition**: 0.70

### Medical

```rust
let config = ManifestConfig::medical();
```

Entity thresholds:
- **symptom**: 0.80
- **treatment**: 0.85
- **diagnosis**: 0.90

### Custom Domain

```rust
let mut config = ManifestConfig::default();
config.domain = "legal".to_string();
config.confidence_threshold = 0.75;
config.dedup_threshold = 0.92;
```

## Performance

| Metric | Value |
|--------|-------|
| **Build time** | ~9s (release) |
| **Binary size** | 2.3M |
| **Compilation** | ✅ Zero warnings (library) |
| **Tests** | ✅ All passing |

## Deduplication Algorithm

**Jaccard Similarity**: Measures overlap between entity token sets

```
similarity(A, B) = |A ∩ B| / |A ∪ B|

if similarity ≥ 0.95 → merge entities
```

**Example**:
- Entity A: "Critical Illness Coverage" (product)
- Entity B: "Critical Illness Coverage" (product)
- Similarity: 1.0 → **MERGED**

**Result**: 1 entity with both sources tracked

## Entity Types

```
Product     (insurance products, medical devices, etc.)
Coverage    (what's covered)
Exclusion   (what's NOT covered)
Condition   (pre-conditions, requirements)
Organization (companies, providers)
Other       (miscellaneous)
```

## Compressed References

Instead of storing full text repeatedly:

```
❌ BEFORE:
chunk_001: "PRU CIL covers heart attack..."
chunk_002: "PRU CIL covers heart attack..."  ← duplicate

✅ AFTER:
Entity: "Critical Illness"
├─ source: prudential.co.th
├─ pageIndex: 1
├─ tokenPosition: 5
└─ tokenPosition: 12
```

## Manifest Configuration

Domain-specific rules in JSON:

```json
{
  "domain": "insurance",
  "confidence_threshold": 0.72,
  "dedup_threshold": 0.95,
  "entity_thresholds": {
    "product": 0.85,
    "coverage": 0.80
  },
  "languages": ["en", "th"],
  "sources": {
    "prudential": {
      "url_pattern": "prudential.co.th/*",
      "rate_limit": 0.5,
      "user_agent": "Mozilla/5.0 (RefGraph/1.0)"
    }
  }
}
```

## Integration with Mimir

RefGraph outputs are ready for Mimir RAG ingestion:

```rust
// 1. Consolidate chunks
let output = graph.consolidate(chunks).await?;

// 2. Save for Mimir
output.save_json("mimir_entities.json")?;
output.save_jsonl("mimir_entities.jsonl")?;

// 3. Ingest into Mimir via API
// POST /api/entities + relationships
```

## Future Phases

### Phase S2 (June 2026)
- Multi-language NER (Thai via python-thainlp bridge)
- BGE-M3 Thai embedding integration
- Confidence scoring refinement

### Phase S3 (July 2026)
- Cross-domain consolidation (medical + insurance)
- Incremental updates (delta consolidation)
- Performance optimization (parallel dedup)

### Phase S4 (August 2026)
- GraphQL query interface
- Web UI for result browsing
- Batch job scheduling (K3s CronJob)

## Dependencies

| Library | Purpose |
|---------|---------|
| `tokio` | Async runtime |
| `serde` | Serialization |
| `neo4rs` | Neo4j client |
| `thiserror` | Error handling |
| `clap` | CLI parsing |
| `log` + `env_logger` | Logging |

## Testing

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_jaccard_similarity

# Coverage (requires tarpaulin)
cargo tarpaulin --out Html
```

## License

AGPL-3.0-or-later (Asgard open-core model)

## Contact

- **Team**: Asgard Engineering
- **Owner**: Data Pipeline Team
- **Email**: paripol@megawiz.co
- **Repository**: https://github.com/megawiz/Mimir

---

**Status**: Production Ready ✅  
**Version**: 0.1.0  
**Last Updated**: May 16, 2026

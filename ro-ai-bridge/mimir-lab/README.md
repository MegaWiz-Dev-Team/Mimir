# mimir-lab

Asgard Analytics **data engine** (ADR-024). Tier B / AGPL cross-cutting compute
for the `asgard_analytics` tenant. Pairs with `mimir-geo` (spatial).

## What it does (MVP)

- **Ingest** CSV → DuckDB table with **schema inference** (`ingest::ingest_csv`),
  optional Parquet export (`ingest::export_parquet`).
- **Query** via a DuckDB engine with a **read-only guard** + **row cap**
  (`Engine::query_readonly`) — an agent tool call can't mutate through it.
- **PII gate** on ingest using shared `skuggi-core` Tier-1 detection
  (`pii::scan_samples`, `pii::gate_table_column`) → `Pending`/`Clean`/`Flagged`.
  Per ADR-024 a dataset stays non-queryable until it leaves `Pending`.

## Build note

`duckdb` uses the `bundled` feature — it compiles libduckdb from source, so the
**first build is heavy** (C++). No system dependency; self-contained for on-prem
boxes. Own `[workspace]` root (like `mimir-fhir`) keeps that build out of the
main `ro-ai-bridge` workspace.

```
cargo test          # first run also builds bundled DuckDB
```

## Registry

The relational catalog (datasets / dataset_versions / analyses / report_jobs /
geo_layers) is `migrations/0001_init_analytics.sql`, applied to the Mimir
MariaDB. Dataset *data* lives in Parquet/DuckDB + MinIO; these tables are
metadata only.

## Not yet (next increments)

- Registry persistence wiring (sqlx) + dataset lifecycle.
- MinIO blob storage of originals/versions.
- Hermodr MCP tools (`dataset_*`, `run_sql`, `plot`) — P2.
- Parquet/Excel/GeoJSON ingest paths beyond CSV.

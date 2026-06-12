# mimir-geo — Asgard Analytics spatial engine (ADR-024 · P4)

Adds GIS + spatial statistics to the analytics stack, behind the same governance as
`mimir-lab` (read-only guard · row-cap · query timeout · Tyr audit · Skuggi PII gate).
**This is the P4 scaffold** — module skeletons + signatures + the checklist below.

## Where it fits
```
analyst-geo / analyst-stats (gemma-4-26b, local)
   └─ Hermodr geo_* / stats_* tools  (hermodr-analytics sidecar)
        └─ analytics-api  (POST /api/v1/analytics/geo/* + /stats/*)   ← add routes here
             └─ mimir-geo
                  ├─ engine.rs    DuckDB + spatial ext (ST_*, ST_Read)
                  ├─ spatial.rs   GeoRust ops (buffer/distance/join/choropleth)
                  ├─ h3.rs        h3o (latlng→cell, k-ring, polyfill, aggregate)
                  ├─ ingest_geo   GeoJSON/Shapefile/Excel → registry + MinIO
                  └─ stats_py.rs  sandboxed, serialized Python (PySAL/scipy)
```
Datasets reuse `mimir-lab`'s `analytics_datasets` registry (+ `geometry_column`,
`srid`); blobs reuse the `asgard-analytics` MinIO bucket.

## Design decisions (carried from ADR-024)
- **Rust-first, one exception:** spatial-stats (Moran/LISA/kriging/point-pattern) run
  in **Python** (PySAL/scipy) because no mature Rust equivalent. Rust owns
  orchestration + governance; Python is a stateless compute kernel.
- **Python is SERIALIZED + resource-capped** — never two Python jobs (or Python + a
  model load) at once on the Mac mini ([[feedback_mac_mini_memory_pressure]]). A
  process-global permit enforces this; over-cap/over-time → kill → `Sandbox` error.
- **No PyO3** — spawn a venv subprocess with temp request/response JSON; keeps SciPy
  out of the Rust build + crash-isolated. ⚠️ some on-prem hosts have **no PyPI** —
  vendor wheels at install time.
- **Shared governance** — geo/spatial SQL goes through the SAME read-only/timeout/
  audit path as tabular `run_sql`; prefer extracting that helper from `mimir-lab`
  rather than duplicating.
- **Single tool per agent** — gemma converges reliably with one tool; keep
  `analyst-geo` / `analyst-stats` allowlists tight (asgard_analytics reliability note).

## P4 checklist (≈5–6 dev-days)
- [ ] `engine.rs` — open DuckDB + `INSTALL spatial; LOAD spatial;`; reuse mimir-lab
      read-only/timeout/audit (extract shared helper). TDD: a spatial `ST_*` query.
- [ ] `spatial.rs` — `geo_buffer` / `geo_distance` / `geo_join` / `geo_choropleth`
      (GeoRust). TDD with fixture WKT/GeoJSON in `tests/`.
- [ ] `h3.rs` — `latlng_to_cell` / `cell_boundary` / `k_ring` / `polyfill` /
      `aggregate` (h3o). TDD: known lat/lng → known cell.
- [ ] `ingest_geo.rs` — GeoJSON/Shapefile via `ST_Read`; Excel via duckdb ext or
      `calamine`; infer geometry+SRID; Skuggi gate attributes; register dataset.
- [ ] `stats_py.rs` + `python/` — serialized capped sandbox; `stats_moran/lisa/
      kriging/pointpattern`; Tyr-audited. Implement `python/stats.py` methods.
- [ ] `api.rs` — handlers; **wire routes into `mimir-lab/src/server.rs`** (the
      analytics-api binary already mounts that router).
- [ ] **Hermodr** — add `geo_*` / `stats_*` ToolDefinitions to `services/analytics.rs`
      (names must match the agents' allowlists).
- [ ] **Portal** — MapLibre GL + PMTiles **offline** tiles in ro-ai-dashboard;
      choropleth + point/H3 layers (deck.gl if needed). New page
      `src/app/analytics/map/page.tsx` + `/api/analytics/geo` proxy route.
- [ ] **Agents** — confirm `analyst-geo` / `analyst-stats` (seeded P0, asgard-infra
      MariaDB) tool allowlists == the new runtime tool names; invoke by NUMERIC id.
- [ ] Build/deploy: `mimir-geo` Dockerfile note — bundled DuckDB+spatial, builder
      **`rust:1.93-slim-bookworm`** (not `-slim`/trixie → GLIBC mismatch, same as
      analytics-api). Tag `mimir-geo-v0.1.0`.

## Not in P4
Research agent (`analyst-research`, deep-research + `lit_search`) + scheduled reports
= **P5**. Open sub-decision still owed before P5: research path local-first vs
cloud-LLM opt-in (Skuggi-gated, default-off).

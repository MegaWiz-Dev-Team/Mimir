# OCR Layout Eval — Runbook

Region-detection OCR evaluation (mAP / parity / GriTS) produced by **Syn** and
stored in **Mimir** for tracking over time. This is the *layout* eval
(`ocr_layout_eval_*` tables, Sprint 53) — distinct from text-level CER/WER,
which is tracked in Syn's own benchmark reports.

> The Sprint 51 `ocr_eval_*` (text CER/WER) schema and the `mimir-text-metrics`
> crate were removed (2026-05-22) — they were never wired to a consumer. See
> migration `20260522000000_drop_ocr_eval.sql`.

## Pieces

| Piece | Where |
|---|---|
| Producer (Syn) | `Syn/services/api/src/bin/syn-eval-ingest.rs` |
| Ingest/read API (Mimir) | `ro-ai-bridge/src/routes/eval_ocr_layout.rs` |
| Schema | `ro-ai-bridge/migrations/sprint53_ocr_layout_eval_schema.sql` |
| Dashboard | `ro-ai-dashboard/src/app/syn-ocr/eval/` (list + `[id]` detail) |

## API

```
POST /api/v1/eval/ocr/layout/runs        # ingest a run + its items
GET  /api/v1/eval/ocr/layout/runs        # list (filter eval_kind, syn_version, dataset_name)
GET  /api/v1/eval/ocr/layout/runs/{id}   # detail + per-image items
```

- `eval_kind` ∈ `{mAP, parity, grits}` (CER/WER is rejected — wrong schema).
- **Tenant**: scoped by the `X-Tenant-Id` header, default `asgard_platform`.
  A run created under one tenant is `404` for another.
- **PII guard**: when `is_synthetic = false`, items must use `image_hash`
  only — any `image_name` is rejected (real data is hash-only; names leak PHI).

## Run an eval end-to-end

1. Produce a result file in Syn (e.g. mAP):

   ```bash
   cd Syn/services/api
   cargo test --test map_layout          # writes benchmarks/.../map_result.json
   ```

2. Ingest it into Mimir (defaults to `asgard_platform`):

   ```bash
   cargo run --bin syn-eval-ingest -- \
       --result benchmarks/region_annotation/fixtures/map_result.json \
       --eval-kind mAP \
       --mimir-url http://localhost:30000
   ```

   For a domain tenant, send the header (the ingest binary forwards it):

   ```bash
   X_TENANT_ID=asgard_medical cargo run --bin syn-eval-ingest -- ...
   ```

3. View in the dashboard: **Analytics → OCR Eval** (`/syn-ocr/eval`). Pick the
   tenant in the dropdown; click a run to see per-image items.

## DB setup

The layout schema is **not** applied by `sqlx::migrate!` (it lives in the
ad-hoc `ro-ai-bridge/migrations/` set, not `mimir-core-ai/migrations/`). Apply
it manually on a fresh DB:

```bash
docker exec -i mimir_mariadb mariadb -uroot -proot mimir_test \
    < ro-ai-bridge/migrations/sprint53_ocr_layout_eval_schema.sql
```

## Verify scoping

```bash
# Seed differs per tenant → list returns only the matching tenant's runs.
curl -s -H 'X-Tenant-Id: asgard_platform' localhost:30000/api/v1/eval/ocr/layout/runs | jq '.tenant, (.runs|length)'
curl -s -H 'X-Tenant-Id: asgard_medical'  localhost:30000/api/v1/eval/ocr/layout/runs | jq '.tenant, (.runs|length)'
```

# Agent Feedback Pipeline — Runbook

Single owned feedback store in Mimir for agent-answer feedback across all tenants.
Replaces the split where dashboard thumbs went to `agent_conversations.feedback`
(a column) while RL read a non-existent `agent_feedback_logs` table.

## Design

```
Dashboard 👍/👎  ──POST /api/v1/conversations/:id/feedback──►  Mimir
                                                                 ├─ UPDATE agent_conversations.feedback   (back-compat, stats)
                                                                 └─ INSERT agent_feedback                 (SOURCE OF TRUTH)
RL / fine-tune / Laminar-bridge  ──read──►  agent_feedback
```

- **One place:** `agent_feedback` (MariaDB `mimir`). Tenant-scoped via `tenant_id`.
- **One API:** `POST /api/v1/conversations/:id/feedback` (tenant-guarded).
- `quality_score`: thumbs_up → 1.000, thumbs_down → 0.000 (room for 1–5 / rubric later).
- `feedback_domain`: NULL until tagged (RL groups by it; NULL rows ignored, not errors).
- `trace_id`: reserved for Phase 2 Laminar span linkage.

## Status (2026-06-06)

| Item | State |
|------|-------|
| `agent_feedback` table | ✅ **applied to live DB** (`mariadb.asgard-infra`) + migration committed |
| Migration files | ✅ `migrations/20260606000000_agent_feedback.sql` (+ `down/`) |
| API patch (`submit_feedback`) | ⚠️ **staged on branch `feat/agent-feedback-store`, NOT built/deployed** |
| Mimir rebuild + redeploy | ⏳ pending safe window (memory pressure — see caution) |

Until Mimir is redeployed, the API still writes only the old column; `agent_feedback`
stays empty. The table + migration are safe and additive regardless.

## Deploy (Phase 1)

> ⚠️ Memory-pressure caution: asgard-infra qdrant had an eviction storm (50+ pods,
> 1 running) and eir was OOM-killed. Per house rule: `sudo purge`, confirm headroom,
> and do **not** run parallel docker builds. Serialize this build.

1. Review staged diff: `cd Mimir && git diff feat/snomed-icd10cm-map..feat/agent-feedback-store -- ro-ai-bridge/src/routes/conversations.rs`
2. Build mimir-api locally (single build, no parallelism).
3. Redeploy `mimir-api` (asgard ns).
4. Commit on green: migration + `conversations.rs` (SemVer bump). Push only when asked.

### Verify
```
# from any pod with curl, against mimir-api.asgard.svc:8080
curl -s -X POST http://mimir-api.asgard.svc:8080/api/v1/conversations/<msgId>/feedback \
  -H 'X-Tenant-Id: asgard_medical' -H 'content-type: application/json' \
  -d '{"feedback":"thumbs_up"}'
# then:
SELECT * FROM agent_feedback ORDER BY id DESC LIMIT 5;
# cross-tenant guard: wrong X-Tenant-Id for a msg id must return 404 Message not found.
```

## Phase 2 — Laminar bridge (deferred; has external blockers)

Push feedback to Laminar as a span score (observability only; Mimir stays SOT).

Prereqs (all currently unmet):
1. **trace linkage** — `agent_conversations` has no `trace_id`. Add a column + capture
   the active OTEL span's trace_id at chat time (`routes/agents/chat.rs`), persist it,
   copy into `agent_feedback.trace_id`.
2. **`LMNR_PROJECT_API_KEY`** — empty. Provision a Laminar project (map project↔tenant),
   set the key on mimir-api.
3. **Mimir OTEL export** — `OTEL_EXPORTER_OTLP_ENDPOINT` appears unset on mimir-api;
   point at `otel-collector.asgard.svc:4317`.
4. **Laminar health** — clickhouse evicted (14); fix memory pressure first or pushes drop.
5. Pin Laminar image (currently `:latest`).

Then: on feedback insert, call Laminar API to attach score/label to `trace_id`.

## RL revive (deferred — RL currently disabled by decision)

RL was disabled (decision 2026-06-06). Reviving needs MORE than this table — the whole
RL schema was never migrated. `rl_agent_self_eval.rs` / `rl_orchestrator.rs` read:
- `agent_feedback_logs`  (raw per-feedback; cols: agent_id, tenant_id, created_at, feedback_domain, quality_score)
- `agent_rl_daily_metrics` (pre-aggregated daily; cols: conversation_count, avg_quality_score, lowest_quality_domain, lowest_quality_score, improvement_opportunity_score, metric_date)
- `skill_deployment_log` (deployment monitor)

To revive: create those tables (or point RL at `agent_feedback` and build a daily
aggregation job that fills `agent_rl_daily_metrics`), add `feedback_domain` tagging,
then re-enable the scheduler behind an `ENABLE_RL_SCHEDULER` env gate (see
`Bifrost/src/main.rs:345`). Also resolve the triple-scheduler (asgard + asgard-rl×2)
and asgard-rl's missing HEIMDALL/MIMIR env before re-enabling.

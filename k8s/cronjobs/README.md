# PubMed Sync CronJobs (Wave 4B)

Free, scheduled PubMed ingestion. Replaces the disabled BigQuery pipeline (`pipeline_pubmed.py.DISABLED`).

## Why CronJob, not in-process cron?

Mimir already runs an internal cron worker (`services/cron.rs`) for `web`/`mcp` source types — it polls `data_sources.next_refresh_at` every 60s. We **kept** that for HTTP sources because it's simple and lives in the same process as the agent.

Python ingestion scripts (PubMed) are different: they're long-running, need their own resource quotas, and can't share the bridge process. K8s CronJob is the right unit:

- Independent retry / backoff / deadline
- Observable via `kubectl get cronjobs`
- Doesn't bloat the bridge pod with a Python runtime
- Matches the in-process cron's expectation that file/document/external sources are "out-of-band"

## Layout

| File | Purpose | Schedule |
|---|---|---|
| `scripts-configmap.yaml` | Mounts the Python sync scripts into pods | n/a |
| `pubmed-incremental.yaml` | Daily NCBI E-utilities sync (~MB) | `0 6 * * *` UTC |
| `pubmed-bulk.yaml` | Weekly PMC FTP bulk loader (~GB) + 20Gi PVC cache | `0 3 * * 0` UTC |

## Deploy

```bash
# 1. Seed the ConfigMap with the actual script content (kubectl --from-file inlines it)
kubectl create configmap pubmed-sync-scripts \
  --namespace asgard-services \
  --from-file=sync_pubmed_incremental.py=scripts/sync_pubmed_incremental.py \
  --from-file=sync_pubmed_pmc_bulk.py=scripts/sync_pubmed_pmc_bulk.py \
  --dry-run=client -o yaml | kubectl apply -f -

# 2. Create credentials secret (NCBI_API_KEY optional but bumps rate limit 3→10 req/s)
kubectl create secret generic pubmed-credentials \
  --namespace asgard-services \
  --from-literal=ncbi_api_key="$(vault kv get -field=ncbi_api_key secret/mimir)" \
  --from-literal=ncbi_email="ops@asgard.local" \
  --dry-run=client -o yaml | kubectl apply -f -

# 3. Apply CronJobs
kubectl apply -f k8s/cronjobs/

# 4. Verify
kubectl get cronjobs -n asgard-services
kubectl get jobs -n asgard-services -l source=pubmed
```

## Ad-hoc sync

```bash
# Trigger an immediate run (creates a one-off Job from the CronJob template)
kubectl create job --from=cronjob/pubmed-incremental-sync pubmed-now-$(date +%s) \
  -n asgard-services
```

The Mimir UI (`/data-sources`) hits `POST /sources/26/sync`, which dispatches to the same script via the `external_corpus` handler in `sync.rs`.

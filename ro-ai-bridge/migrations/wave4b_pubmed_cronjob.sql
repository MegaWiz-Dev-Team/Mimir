-- Wave 4B: Wire PubMed external_corpus source to K8s CronJobs.
--
-- Adds k8s_cronjob / k8s_namespace / k8s_bulk_cronjob fields to the PubMed
-- source's config_json so POST /sources/26/sync can trigger an immediate
-- K8s Job via `kubectl create job --from=cronjob/...` (see sync.rs).
--
-- Idempotent: uses JSON_MERGE_PATCH so existing keys are preserved.

UPDATE data_sources
SET config_json = JSON_MERGE_PATCH(
    COALESCE(config_json, JSON_OBJECT()),
    JSON_OBJECT(
        'k8s_cronjob',       'pubmed-incremental-sync',
        'k8s_namespace',     'asgard-services',
        'k8s_bulk_cronjob',  'pubmed-bulk-sync'
    )
)
WHERE name = 'PubMed abstracts'
  AND tenant_id = '__global__'
  AND source_type = 'external_corpus';

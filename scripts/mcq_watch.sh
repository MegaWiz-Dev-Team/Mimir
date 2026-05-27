#!/usr/bin/env bash
# Realtime view of medical MCQ benchmark progress (reads Mimir eval tables).
#   ./scripts/mcq_watch.sh [refresh_seconds]   (default 5; Ctrl-C to stop)
set -uo pipefail
INTERVAL="${1:-5}"
TENANT="${TENANT:-asgard_medical}"
SQL() { kubectl -n asgard-infra exec -i deploy/mariadb -- mariadb -uroot -proot mimir -B -t -e "$1" 2>/dev/null; }

while true; do
  clear
  echo "🩺 Medical MCQ Benchmark — live  ($(date '+%H:%M:%S'))   refresh ${INTERVAL}s · Ctrl-C to stop"
  echo "tenant: $TENANT"
  echo

  # per-run progress + live accuracy (eval_scores updates per item)
  SQL "SELECT
         REPLACE(r.name,'  ',' ')                                              AS run,
         r.status,
         CONCAT((SELECT COUNT(*) FROM eval_scores s WHERE s.run_id=r.id),
                ' / ', r.total_combinations)                                    AS progress,
         CONCAT(ROUND((SELECT AVG(accuracy_score) FROM eval_scores s WHERE s.run_id=r.id)*100,1),'%') AS acc,
         CONCAT(ROUND((SELECT AVG(latency_ms) FROM eval_scores s WHERE s.run_id=r.id),0),'ms')         AS avg_lat
       FROM eval_runs r
       WHERE r.tenant_id='$TENANT' AND r.variable_under_test='model'
       ORDER BY r.started_at;"

  echo
  echo "── accuracy matrix (benchmark × model, completed) ──"
  SQL "SELECT
         d.name AS benchmark,
         REPLACE(s.model_id,'mlx-community/','') AS model,
         CONCAT(ROUND(s.avg_accuracy*100,1),'%') AS acc,
         s.total_questions AS n
       FROM eval_summary s
       JOIN eval_runs r ON r.id=s.run_id
       JOIN eval_benchmark_datasets d ON d.id=JSON_UNQUOTE(JSON_EXTRACT(r.config,'\$.benchmark_dataset_id'))
       WHERE s.tenant_id='$TENANT' AND r.variable_under_test='model'
       ORDER BY d.name, s.avg_accuracy DESC;"

  running=$(ps aux | grep -c "[m]cq_eval.py")
  echo
  echo "mcq_eval processes running: ${running}"
  sleep "$INTERVAL"
done

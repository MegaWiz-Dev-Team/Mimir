# Sprint 51d — Retrospective

**Date:** 2026-05-09 (close)
**Duration:** 1 day (compressed from estimated 2-3)
**Predecessor:** Sprint 51c (judge-config asymmetry discovery)

## Goal

Resolve the judge-config asymmetry finding from Sprint 51c by:
1. **Day-1** — Lock canonical judge config in eval-harness; opt-out for historical comparison only
2. **Day-2** — Rerun gemma-4-26b broader-100 with `judgeThink=default` to get the apples-to-apples vs typhoon broader-100 + update scoreboard
3. **Day-3** — Spec the clinician-graded unsafe subset protocol (resolves the 19% unsafe-rate uncertainty for next sprint)

## What shipped

| Day | Deliverable | Reference |
|---|---|---|
| 1 | `JUDGE_THINKING` env var added to `run_healthbench_eval.py`; `gen_config["thinkingConfig"]={"thinkingBudget":0}` is the locked default | [`scripts/run_healthbench_eval.py`](../../scripts/run_healthbench_eval.py) |
| 1 | `eval_runs.config` JSON now stores `judge_thinking` + `judge_thinking_budget` per run for permanent audit trail | same file |
| 2 | gemma-4-26b broader-100 judgeThink rerun: **39.06%** | [`reports/hbp-gemma-4-26b-a4b-it-4bit-f2eeb239-n100-judgeThink-20260508T132202Z.json`](../04_evaluation_and_testing/reports/hbp-gemma-4-26b-a4b-it-4bit-f2eeb239-n100-judgeThink-20260508T132202Z.json) |
| 2 | Scoreboard 04_03 amended with actual numbers + revision note | [`04_03_HealthBench_Pro_Baseline_2026-05-04.md`](../04_evaluation_and_testing/04_03_HealthBench_Pro_Baseline_2026-05-04.md) |
| 2 | HBp explainer 04_07 updated with § 11 Judge config policy | [`04_07_HealthBench_Pro_Explainer.md`](../04_evaluation_and_testing/04_07_HealthBench_Pro_Explainer.md) |
| 2 | Blog post 03 (judge-config-asymmetry) updated with revised numbers + lesson learned section | `landing-page/src/asgard/blog/posts/03-judge-config-asymmetry.jsx` (deployed 2026-05-08) |
| 3 | Clinician-graded unsafe subset spec | [`04_08_Clinician_Graded_Unsafe_Subset_Spec.md`](../04_evaluation_and_testing/04_08_Clinician_Graded_Unsafe_Subset_Spec.md) |

## Key finding — locked-20 boost did not generalize

Going into Day-2 the working hypothesis was: "gemma's +11pp judgeThink boost on locked-20 will repeat on broader-100, putting gemma broader-100 judgeThink at ~46.81% and flipping the apples-to-apples ranking against typhoon."

Actual measurement: gemma broader-100 judgeThink = **39.06%** (only +3.25pp boost). Typhoon still leads broader-100 in **both** judge configs (+9.07pp strict, +4.13pp judgeThink). Only **locked-20 judgeThink** favors gemma.

Implications:
1. **Sprint 51b "champion swap blocked" decision still stands** — locked-20 (the historical baseline anchor) still favors gemma in judgeThink mode, and all the non-bench blockers (vendor warning, 19% unsafe at scale, bimodal accuracy) remain
2. **Don't extrapolate judge-effect deltas across anchors** — the same model can have a 11pp boost on one anchor and only 3pp on another. Sample-set composition matters more than anticipated.
3. **The dual-anchor discipline is vindicated** — single-anchor would have led us to the wrong conclusion under either hypothesis

## Resource ops note

Day-2 required ~1 hour of Heimdall MLX downtime: stopped `com.asgard.heimdall-mlx` (Qwen3.5-35B-A3B-4bit ~20 GB) to free memory for the gemma-4-26b 4-bit ~14 GB load + 100-question bench (29s/question avg = ~50 min runtime including model load). Restarted after bench completed; Bifrost agents fell back to cloud during the window.

This worked but is fragile: any sprint that needs a different local model for bench has to negotiate the same memory window. Long-term mitigation: **model hot-swap pattern** (already in place via `launchctl`) + **scheduled bench windows** (off-hours rerun jobs). See pending Vardr `/api/system` endpoint for live RAM visibility.

## What did not happen this sprint (intentional)

- **Clinician-grading execution** — only the spec doc shipped. Recruiting + scheduling 2 clinicians + 1 adjudicator is data-ops work gated on contract + budget (~฿15K floor, ~฿35K with UI tooling). Not blocked technically.
- **Eval-harness regression test** — the canonical judge config change is small (3-line conditional) but a unit test on `gen_config` shape would harden it. Logged as Sprint 52 carry-over.
- **Dashboard UI** — no Mimir UI surface yet for `judge_thinking` filter. Existing `/evaluations/diagnose` reads the config column but no faceted filter.

## What should change for next sprint

1. **Stop estimating apples-to-apples deltas without measuring** — Day-2 hypothesis was wrong by 8pp. Cheaper to just rerun (~50 min) than to extrapolate.
2. **Memory budgeting for benches** — every sprint that runs a bench should declare upfront whether it needs Heimdall-MLX downtime. Standardize the launchctl unload/load pattern with a wrapper script so the dependency is explicit.
3. **Vardr `/api/system`** — shipped 2026-05-09 as adjacent-sprint cleanup. Use it to track memory pressure during future bench windows.

## Acceptance criteria — closed

- [x] Canonical judge config locked in eval-harness with opt-out env
- [x] Apples-to-apples gemma-4-26b broader-100 judgeThink number measured (not estimated)
- [x] Scoreboard updated with revision note (no silent overwrite of historical numbers)
- [x] Blog post 03 reflects actual numbers + lesson learned
- [x] Clinician-graded unsafe subset protocol documented (data-ops-ready)

## Carry-overs

- Vardr `/api/system` endpoint shipped 2026-05-09 (was deferred during Day-2)
- Eval-harness unit test on `gen_config` shape — Sprint 52 ad-hoc
- Mimir dashboard `judge_thinking` filter — Sprint 52 ad-hoc

## Total spend

- Gemini API (gemma broader-100 judgeThink rerun): **~$0.10-0.30** (100 judge calls @ ~$0.001-0.003)
- Engineering: 1 working day

Within autonomous-spend threshold (<$1 USD per task per memory).

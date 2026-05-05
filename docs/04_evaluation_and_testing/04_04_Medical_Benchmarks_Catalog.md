# 🩺 Medical Benchmark Catalog (Mimir local mirrors)

**Date:** 2026-05-05
**Location:** `/Users/mimir/Developer/Mimir/benchmarks/medical/`
**Total size:** ~480 MB
**Downloader:** `scripts/download_medical_benchmarks.py`

Five industry-standard medical benchmarks downloaded locally for use with
Mimir's evaluation pipeline. Provides paper-comparable scoreboards across
multiple dimensions (knowledge MCQ, document Q&A, expert reasoning,
multi-turn safety).

---

## 📦 Inventory

| # | Name | Type | Train | Dev | Test | License | Source |
|---|---|---|------:|----:|-----:|---|---|
| 1 | **MedQA** | MCQ (USMLE 4-5 choice) | 10,178 | 1,272 | 1,273 | MIT | [bigbio/med_qa](https://huggingface.co/datasets/bigbio/med_qa) |
| 2 | **MedMCQA** | MCQ (Indian AIIMS/NEET 4 choice) | 182,822 | 4,183 | 6,150 | MIT | [openlifescienceai/medmcqa](https://huggingface.co/datasets/openlifescienceai/medmcqa) |
| 3 | **PubMedQA** | Y/N/Maybe over abstracts | 1,000 *(labeled)* | — | — | MIT | [qiaojin/PubMedQA](https://huggingface.co/datasets/qiaojin/PubMedQA) |
| 4 | **HealthBench** | Multi-turn convs + physician rubrics | — | — | 5,000 + 1,000 (Hard) + 3,671 (Consensus) | MIT | OpenAI Azure blob |
| 5 | **MedXpertQA** | Expert-level reasoning MCQ | — | 5+5 | 2,450 (Text) + 2,000 (MM) | (HF default) | [TsinghuaC3I/MedXpertQA](https://huggingface.co/datasets/TsinghuaC3I/MedXpertQA) |

PubMedQA also has 61,249 unlabeled and 211,269 artificial examples for
fine-tuning use; only the 1K labeled set is the standard eval split.

---

## 📋 Schema by benchmark

### 1. MedQA (`medqa/data_clean/questions/US/test.jsonl`)
```json
{
  "question": "A junior orthopaedic surgery resident is completing a carpal tunnel repair...",
  "answer":   "Tell the attending that he cannot fail to disclose this mistake",
  "options":  {"A": "...", "B": "...", "C": "...", "D": "...", "E": "..."}
}
```
Splits also exist for Mainland (Mandarin) and Taiwan (Traditional Chinese).
**Standard eval:** US English `test.jsonl` (1,273 items).

### 2. MedMCQA (`medmcqa/data/test-00000-of-00001.parquet`)
```json
{
  "id": "84f328d3-...",
  "question": "Which of the following is derived from fibroblast cells?",
  "opa": "TGF-β", "opb": "MMP2", "opc": "Collagen", "opd": "Angiopoietin",
  "cop": -1,                  // correct option (0=opa, 1=opb, 2=opc, 3=opd; -1 in test split = label hidden)
  "choice_type": "single",
  "exp": "",                  // explanation (often blank in test)
  "subject_name": "Pathology",
  "topic_name": null
}
```
**Note:** Test split has `cop=-1` (labels withheld) — use `validation` for local
scoring. Or use `train` split where labels are available.

### 3. PubMedQA (`pubmedqa/pqa_labeled/train-00000-of-00001.parquet`)
```json
{
  "pubid": 21645374,
  "question": "Do mitochondria play a role in remodelling lace plant leaves...?",
  "context": {
    "contexts": ["Programmed cell death (PCD)...", "...", "..."],
    "labels":   ["BACKGROUND", "RESULTS", "..."]
  },
  "long_answer": "yes...",
  "final_decision": "yes"      // yes / no / maybe
}
```
**Standard eval:** `pqa_labeled` (1,000 items) — physician-curated. The
`pqa_unlabeled` and `pqa_artificial` are for training only.

### 4. HealthBench (`healthbench/oss_eval.jsonl`)
```json
{
  "prompt_id": "...",
  "prompt": [{"role": "user", "content": "..."}],   // multi-turn list
  "rubrics": [
    {"criterion": "Advises the patient to find a mental health provider...",
     "points": 10,
     "tags": ["level:example", "axis:completeness"]},
    ...                                              // 10-25 criteria per example
  ],
  "ideal_completions_data": {...},
  "example_tags": [...]
}
```
**Variants:**
- `oss_eval.jsonl` — main 5,000 conversations
- `hard.jsonl` — 1,000 (frontier-model-discriminating subset)
- `consensus.jsonl` — 3,671 (physician-consensus rubrics)

**Grader (paper):** `gpt-4.1-2025-04-14`. Scoring: `achieved_pts / total_pts`,
clipped [0,1]. Reference grader code:
[github.com/openai/simple-evals/healthbench_eval.py](https://github.com/openai/simple-evals/blob/main/healthbench_eval.py).

### 5. MedXpertQA (`medxpertqa/Text/test.jsonl`)
```json
{
  "id": "Text-0",
  "question": "Which patient scenario represents the most appropriate indication...?",
  "options": {"A": "...", ..., "F": "..."},          // 5-10 distractors
  "label": "...",
  "medical_task": "Diagnosis | Treatment | ...",
  "body_system": "Musculoskeletal | ...",
  "question_type": "..."
}
```
**Two tracks:**
- `Text/` — text-only, **best fit for Eir** (no vision)
- `MM/` — multimodal (image+text); ignore until Mimir adds vision.

`MM/test.jsonl` requires the `images.zip` archive (skipped in our download).

---

## 🎯 Mapping benchmarks → Asgard use cases

| Use case | Best benchmark | Why |
|---|---|---|
| **Marketing / paper** | MedQA test | Most-cited, 1273 items fast to run, paper-comparable |
| **Score-vs-cloud** | HealthBench Hard | 1K hard items, OpenAI's own benchmark, top-of-paper "32% Hard" claim |
| **Eir RAG validation** | PubMedQA labeled | Tests whether retrieval+reading actually works (small n=1K) |
| **Reasoning ceiling** | MedXpertQA Text test | 2450 expert-grade items; perfect for Sprint 38 reasoning loop |
| **Bulk Lt. checkpoint** | MedMCQA validation | 4183 items — large enough to detect 1pp lift with significance |
| **Safety / clinical floor** | HealthBench Consensus | 3671 physician-consensus rubrics — for B2B clinical safety story |

---

## 🛠️ Integration with Mimir's `benchmark_datasets` table

Each downloaded benchmark should be registered as a row in
`benchmark_datasets` so the existing eval UI (`/evaluations`) can run them.

Suggested loader script (Sprint 40 task **B-37**, see
[03_14 sprint plan](../03_implementation_plans/03_14_Local_LLM_Optimization_Sprints.md)):

```bash
python3 scripts/load_medical_benchmarks_to_db.py \
  --benchmarks medqa,medmcqa,pubmedqa,healthbench,medxpertqa \
  --tenant_id __global__ \
  --limit-per 1000   # initial load — full sets too large for first run
```

The loader will:
1. Read each benchmark from `benchmarks/medical/<name>/`
2. Normalize each example to Mimir's eval schema:
   `{id, question, ground_truth_answer, rubric, source_id, specialty}`
3. INSERT into `benchmark_items` with FK to `benchmark_datasets`
4. Register dataset row with id `med-{benchmark}-v1` etc.

After loading, runs are triggered identically to current HealthBench-Pro
flow:
```bash
MODELS="gemini-3-pro-preview:google" \
BENCHMARK_ID="med-medqa-v1" \
python3 scripts/benchmark_all_local_models.py
```

---

## 💰 Cost / Time estimates

For one full run on `eir` agent (gemma-4-26b local):

| Benchmark | n | Inference time | Grader cost |
|---|---:|---:|---|
| MedQA test | 1,273 | ~5 hr local | $5-10 (gpt-4.1) or $1-2 (gemini-2.5-flash) |
| MedMCQA val | 4,183 | ~17 hr local | $17-30 / $3-5 |
| PubMedQA labeled | 1,000 | ~4 hr local | $4-8 / $1 |
| HealthBench main | 5,000 | ~21 hr local | $50-100 / $10-15 |
| HealthBench Hard | 1,000 | ~4 hr local | $10-20 / $2-3 |
| MedXpertQA Text test | 2,450 | ~10 hr local | $10-20 / $2-3 |

**Recommended first cut:** stratified sample of 200 from each (1,200 total)
to get a directional read before committing to full runs.

---

## 🔗 References

- [HealthBench paper (arXiv:2505.08775)](https://arxiv.org/abs/2505.08775)
- [MedQA paper (arXiv:2009.13081)](https://arxiv.org/abs/2009.13081)
- [MedMCQA paper (arXiv:2203.14371)](https://arxiv.org/abs/2203.14371)
- [PubMedQA paper (arXiv:1909.06146)](https://arxiv.org/abs/1909.06146)
- [HF Open Medical-LLM Leaderboard](https://huggingface.co/blog/leaderboard-medicalllm)
- ["Beyond the Leaderboard" (arXiv:2508.04325)](https://arxiv.org/html/2508.04325) —
  why no single benchmark is enough; recommends suite-style evaluation.

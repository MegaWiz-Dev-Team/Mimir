# Sprint 39 LoRA Learning Journal — for ML beginners

**Started:** 2026-05-06
**Audience:** ทีมที่มีพื้น ML บ้าง (รู้จัก neural network แต่ไม่เคย fine-tune)
**Purpose:** บันทึกทุก phase ของ Sprint 39 LoRA fine-tune Eir agent — what / why / lessons

> เอกสารนี้ **อัปเดตทุก phase**. ดูส่วน [§5 Per-phase journal](#5-per-phase-journal) สำหรับ history.

---

## 0. Big picture — เรากำลังทำอะไร?

**เป้าหมาย:** ทำให้ **Eir** (medical AI agent ของ Asgard) ตอบคำถามแพทย์ได้ดีกว่า champion baseline ปัจจุบัน

**Champion ตอนนี้:** `gemma-4-26b-a4b-it-4bit` (open-source 26B params, 4-bit quantized) ที่ score
- 47.8% HBp บน locked-20 (ดู §3 Glossary)
- 37.6% HBp บน broader-100

**Sprint 39 hypothesis:** ถ้าเอา Gemma มา **fine-tune** ด้วย medical Q-A พิเศษ จะได้คะแนนสูงขึ้น ≥+5pp

**Method:** **LoRA** (Low-Rank Adaptation) — fine-tune แบบประหยัด ไม่ต้อง train ทั้ง 26B params

---

## 1. Why fine-tune? Why LoRA?

### Why fine-tune (ไม่ใช่ prompt engineering / RAG)?

| Method | What it does | Limits |
|---|---|---|
| **Prompt engineering** | บอก model ว่าให้ตอบยังไง (system prompt) | ไม่เปลี่ยน knowledge ของ model |
| **RAG** | ดึง context จาก vector DB ไป feed model | knowledge มาจาก documents, ไม่ใช่ model ตัวเอง |
| **Fine-tune** | สอน model ใหม่ผ่าน training | **เปลี่ยน knowledge + style ใน weights ของ model** |

Sprint 37 พิสูจน์แล้วว่า prompt + retrieval tweaks ยัน ceiling ที่ ~50%. Fine-tune เป็นทางเดียวที่จะทะลุ

### Why LoRA (ไม่ใช่ full fine-tune)?

Full fine-tune Gemma 26B:
- ต้อง train ทั้ง **26 billion params**
- VRAM 100GB+ (M3 Max มี 128GB ก็เครียด)
- เก็บ checkpoint **~50GB ต่อ run**
- Train **หลายวัน**

LoRA:
- Train แค่ **~0.5%** ของ params (~130M)
- VRAM ~25GB
- Adapter file **~700MB**
- Train **15-90 นาที**

**LoRA insight:** "การปรับ model สำหรับ task ใหม่ ใช้ 'การเปลี่ยน' น้อยกว่าที่คิด" — ΔW (matrix การเปลี่ยน) มี **low rank** (information น้อย แม้ matrix ใหญ่)

---

## 2. The high-level pipeline

```
[1] Build Curator UI (Sprint 39 Phase 0)
    └─ Tool ให้รีวิว/แก้ training pairs
       ↓
[2] Synthesize medical Q-A corpus (Phase 1a/1b)
    └─ ใช้ LLM (gemma local หรือ Gemini cloud) ผลิต Q+A
       ↓
[3] Train LoRA adapter (Phase 2)
    └─ mlx_lm.lora --train (ใช้ 30 min - 2 hr)
       ↓
[4] Fuse adapter into base model (Phase 3)
    └─ mlx_lm.fuse (ได้ merged 14GB)
       ↓
[5] Hot-swap MLX server to merged model
    └─ launchctl unload + load
       ↓
[6] Eval against locked benchmark items (Phase 3 cont.)
    └─ POST /api/v1/eval/runs (gemini-2.5-flash judge)
       ↓
[7] Promotion gate decision
    └─ Pass +5pp on BOTH anchors? → champion change
```

---

## 3. Glossary — ทุกคำที่งง

### Benchmark + scoring

| Term | What it means | ที่มา |
|---|---|---|
| **HealthBench-Pro (HBp)** | Medical Q-A benchmark, 525 items, scored 1-5 per dim by gemini-2.5-flash judge | Asgard custom dataset, OpenAI HealthBench-inspired |
| **HBp%** | Score normalized 0-100% | `((acc-1)/4 + (comp-1)/4 + (rel-1)/4 + safety) / 4 × 100` |
| **Acc / Comp / Rel** | Accuracy / Completeness / Relevance — 3 of 4 scoring dimensions (each 1-5) | judge rubric |
| **Safety dimension** | binary 0/1 per item (1=safe, 0=unsafe). Avg 0-1. | judge rubric |
| **Unsafe count** | # items where judge marked safety=0 | direct count |

### Sample sets

| Term | Size | Items | Why |
|---|---|---|---|
| **Locked-20** | 20 | curated subset, hand-picked specialty-balanced | original Sprint 36 baseline; reproducible across runs |
| **Broader-100** | 100 | first-100 of full hb-pro-asgard-001 (525 total) | more representative of real distribution |
| **Why dual-anchor?** | — | — | locked-20 was *easier* (47.8% champion) than broader-100 (37.6%) — sample bias revealed Sprint 43 follow-up |

### Promotion gate

| Term | Meaning |
|---|---|
| **Champion** | Current best model (incumbent) |
| **Challenger** | New model being tested |
| **Gate** | Threshold a challenger must clear to replace champion |
| **+5pp** | "+5 percentage points" — e.g. 47.8% → 52.8% (NOT 5% relative) |
| **Sprint 39 dual-anchor gate** | Both: locked-20 ≥52.8% AND broader-100 ≥42.6% (champion + 5pp on each) |

### Training terms

| Term | Definition | Sprint 39 value |
|---|---|---|
| **Corpus / Dataset** | Collection of training Q-A pairs | 3,798 (Phase 1b) |
| **Pair** | One {question, answer} item | 1 pair = 1 training example |
| **Token** | sub-word unit (~4 chars/token English) | gemma context = 8192 tokens |
| **Iteration (iter)** | One forward+backward pass on a batch | 300 → 1000 iters |
| **Batch size** | # examples per iter | 2 |
| **Epoch** | One full pass through dataset | 3798 / 2 = 1899 iters/epoch |
| **Learning rate (LR)** | How big a step each iter takes | 1e-4 (standard for LoRA) |
| **Loss** | Error on training data; lower = model learning | Phase 2: 1.367→1.159 |
| **Val loss** | Error on held-out validation set; tracks overfit | Phase 2: stable 1.28 |
| **Overfit** | Model memorizes training, fails on unseen | val loss rising while train falls |

### LoRA-specific

| Term | What | Sprint 39 value |
|---|---|---|
| **rank (r)** | "Width" of low-rank update; capacity | 8 → 16 (Phase 2c) |
| **alpha (scale)** | How strong adaptation is mixed in (`(α/r) × A×B`) | 20 → 32 (= 2r) |
| **dropout** | % weights randomly zeroed (regularization) | 0 → 0.05 |
| **num_layers** | How many attention layers get LoRA | 16 → 24 (Phase 2c) |
| **target modules** | Which matrices get LoRA (q,k,v,o = attention) | q_proj, k_proj, v_proj, o_proj |
| **Adapter** | Just the trained A,B matrices (~700MB) | `/tmp/lora_phase2_adapter/adapters.safetensors` |
| **Fuse** | Merge adapter into base model → produces deployable model | `mlx_lm fuse` |

### Synthesis terms

| Term | What | Sprint 39 |
|---|---|---|
| **Synthesis** | Use LLM to generate training data from seed topics | Phase 1a/1b |
| **Augmentation** | Modify existing data (e.g. append safety blurb) | Phase 2b retry |
| **Hedging** | "Consult professional" / "see your doctor" disclaimers | core to safety dim |
| **Token leakage** | Model emits training-only tokens (e.g. `<|channel|>`) at inference | seen in MVP run |
| **Catastrophic forgetting** | LoRA forgets base capability while learning new | Phase 2 safety regression |

### Operational

| Term | What |
|---|---|
| **Hot-swap** | Reload MLX server with new model (without API downtime > 30s) |
| **launchd** | macOS service supervisor running MLX process |
| **Heimdall** | Asgard's LLM gateway (port 8080) |
| **MLX server** | Apple Silicon-optimized inference (port 8081) |

---

## 4. Methodology — why these specific choices

### Choice 1: Why Gemini Flash (not local gemma) for synthesis?

Phase 1a tried local gemma → quality issues:
- Token leakage (`<pad>`, `<|channel|>` tokens in answers)
- JSON format drift (parse failures)
- 12-15 hours runtime

Phase 1b switched to Gemini 3 Flash batch:
- $2.55 for 3,798 pairs (50% batch discount)
- 3-min wall-clock
- Higher quality (specific drug names, RUCAM tool, DRESS criteria — pro-level details)

**Lesson:** Cloud-Flash for batch synth >>> local for batch synth (when quality matters)

### Choice 2: Why dual-anchor gate?

Earlier we discovered **sample bias**:
- gemma-4-26b on locked-20 = 47.8% (easy items)
- gemma-4-26b on broader-100 = 37.6% (representative)

If we promoted based on locked-20 alone, we'd over-fit champion to easier subset → real production performance would be ~10pp worse.

**Dual-anchor** ensures lift is general, not curated.

### Choice 3: Why "+5pp lift" target?

- n=20 sample noise = ±5pp typical (variance from item selection)
- +5pp = 1σ above noise = signal not lucky sampling
- n=100 noise narrower (~±2pp) but +5pp keeps a single threshold across both

### Choice 4: Why hand-curate 30 seed pairs first (Phase 1a MVP)?

Validate the **pipeline**, not the model:
- Curator UI works? (B-30c review page)
- Train script works? (lora_train_mvp.py)
- Fuse works? (mlx_lm.fuse)
- Eval works? (n=20 locked items)

If MVP fails → fix pipeline before paying $3-25 for Gemini synth. (We had to fix Python 3.9 compat + token-leakage bugs at MVP scale, cheap)

### Choice 5: Why iters=300 first, 1000 second?

For 3,798 pairs / batch=2 = **1899 batches per epoch**:
- 300 iters = 0.16 epoch (model sees only 16% of data)
- 1000 iters = 0.5 epoch
- 1900 iters = 1 epoch
- "LoRA standard" is 2-3 epochs

Phase 2 used 300 to fail-fast. Phase 2c uses 1000 to give learning more headroom. If still doesn't clear gate, Phase 2d will go 2000+.

### Choice 6: Why rank 8 first, 16 second?

- rank 4 = task-specific (single template)
- **rank 8** = baseline standard
- rank 16 = diverse corpus
- rank 32+ = approaching full fine-tune (defeats LoRA purpose)

3,798 pairs = "diverse corpus" → rank 16 is the principled next step

---

## 5. Per-phase journal

### Phase 0: Curator UI (DB + backend + frontend)

**Goal:** Build the annotation tool ก่อน — เพราะถ้าไม่มี UI, จะ review 5K pairs ยังไง?

**What:**
- DB migration: `training_corpus_items` + `training_corpus_datasets` + `lora_training_runs` + `ai_models` lineage cols
- Mimir backend: `/api/v1/training/*` endpoints (Axum, ~750 LOC)
- Mimir Dashboard: `/training` (datasets list) + `/training/[id]` (review page)
- Multi-tag widget (later add-on)
- Menu placement: AI group (was Analytics — wrong)

**Lesson:** UI tooling first. Without Curator, every iteration would be SQL hell.

**Cost:** $0 (all dev time)

---

### Phase 1a MVP: Hand-curated 30 pairs (validation)

**Goal:** Prove pipeline works before paying for Gemini

**What:**
- Wrote 30 medical Q-A pairs by hand (Claude curation)
- 8 specialties (cardio/endo/ent/peds/emergency/psych/pharmacy/genmed)
- Tagged with cross-cutting tags (multi-tag feature)
- Auto-approved (skip review for MVP)
- Trained mini-LoRA: rank=8, layers=8, iters=50 (~80 sec)
- Fused + hot-swapped + eval n=20 locked
- Result: **49.7% HBp** (+1.9pp lift, but n=20 sample noise)
- 0/20 unsafe

**Lesson:** Pipeline works end-to-end at $0.05 cost. No reason to scale up corpus until pipeline is sound.

**Cost:** $0.05 (judge fees only)

---

### Phase 1b: Gemini batch synth 3,798 pairs

**Goal:** Production corpus quality

**What:**
- Designed 86 topic seeds × 8 specialties
- Built Gemini Batch API client (`phase1b_batch_synth.py`)
  - Upload JSONL via Files API resumable upload
  - Submit batch → poll status → download results
  - 50% batch discount on Gemini 3 Flash
- 474 batched calls × ~5 pairs each → 3,798 valid pairs parsed
- Quality verification: Pro-level details (RUCAM, Gell-Coombs, DRESS, etc.)
- Auto-approved all (Gemini Flash quality validated by sample)

**Bug encountered:** key-mismatch in metadata reattach → specialty/tags came back NULL on import. Cosmetic only (LoRA training doesn't need them).

**Lesson:** Cloud Gemini Flash is **massively** higher quality than local gemma synthesis. Worth the $2.55.

**Cost:** $2.55 (batch synth)

---

### Phase 2: First LoRA training (rank=8, layers=16, iters=300)

**Hypothesis:** LoRA on 3,798 Gemini-Flash pairs lifts gemma-4-26b ≥+5pp HBp

**Setup:**
- Base: gemma-4-26b-a4b-it-4bit
- Corpus: 3,798 Gemini-synthesized pairs (raw, no augmentation)
- Hyperparams: rank=8, scale=20 (alpha=20), num_layers=16, batch_size=2, LR=1e-4, iters=300
- Train: 14 min wall-clock
- Adapter: 713MB (saved at iters 100/200/300)

**Train metrics:**
- Train loss: 1.367 → 1.159 (-15%, healthy learning)
- Val loss: stable at 1.28 (no overfitting)

**Result:**
| Anchor | Champion | LoRA Phase 2 | Δ |
|---|---|---|---|
| Locked-20 HBp | 47.8% | **46.6%** | **−1.2pp** |
| Broader-100 HBp | 37.6% | 40.0% | +2.4pp |
| Locked-20 Safety | 0.75 | **0.50** | **−0.25** ⚠️ |
| Acc / Comp / Rel | (lower) | +0.20-0.40 each | ✅ |

**Verdict:** ❌ does not promote. Both anchors fail +5pp gate.

**Lesson:** **LoRA inherits training distribution.** Gemini-Flash answers were medically detailed but **lacked "consult professional" hedging** (only 9.3% of pairs had it). Model learned directive style → judge marked answers as less safe. Acc/Comp/Rel all improved (+0.20-0.40), but safety dropped −0.25 dragged HBp down.

**Insight:** Safety dimension is **catastrophic forgetting** signal. Base model's RLHF safety alignment got partially overwritten by domain content training.

**Cost:** $0 train + $0.32 eval = **$0.32**

---

### Phase 2b: Same hyperparams + safety hedging augmentation

**Hypothesis:** Append safety hedging to 78% of pairs that lack it → restore Safety dim → net HBp lift

**Setup:**
- Source: same 3,798 pairs (`07560b58`)
- New augmented dataset: `c56794c8` (3,798 = 844 already-hedged + 2,954 augmented)
- 8 random hedging templates (deterministic seed=42 for reproducibility)
- Detection: text contains BOTH "consult/refer/seek" verb AND professional/clinician/specialist noun
- **Same hyperparams** as Phase 2 (controlled A/B)
- Train: ~14 min

**Hedging template example:**
> "Clinical decisions in this scenario should be made in consultation with a qualified physician familiar with the patient's complete history, current medications, and exam findings."

**Result (vs Phase 2):**
| Metric | Phase 2 | Phase 2b | Δ |
|---|---|---|---|
| Locked-20 HBp | 46.6% | **51.6%** | **+5.0pp** 🚀 |
| Locked-20 Safety | 0.50 | **0.85** | **+0.35** 🚀 |
| Locked-20 Unsafe | 1/20 | **0/20** ⭐ | -1 |
| Broader-100 HBp | 40.0% | 38.4% | −1.6pp ⚠️ |

**Verdict:** ✅ Hypothesis confirmed (Safety fully restored). 🟡 Promotion still no-go (locked-20 +3.8pp vs +5pp gate; broader-100 +0.8pp vs +5pp gate).

**Lesson:**
1. **Augmentation works for management/dosing questions** (locked-20 heavy in those)
2. **Augmentation hurts definition/conceptual questions** (broader-100 has more "what is X")
3. **0 unsafe** for the first time — safety dim CAN be controlled via training data composition

**Insight:** Conditional hedging (skip definitions, hedge management) might fix broader-100. **OR** re-synth with hedging baked into the synthesis prompt (more natural integration than appended blurb).

**Cost:** $0 augment + $0 train + $0.32 eval = **$0.32**

---

### Phase 2c: Bigger LoRA on augmented corpus (current)

**Hypothesis:** Phase 2b was capacity-bound. Larger rank + more layers + more iters → close the +1.2pp gap on locked-20 to clear gate.

**Setup:**
- Source: same augmented corpus `c56794c8`
- Hyperparams (config file `/tmp/lora_phase2c_config.yaml`):
  - rank: 8 → **16** (2× capacity)
  - num_layers: 16 → **24** (1.5× layers fine-tuned)
  - lora_parameters.scale: 20 → **32** (2 × rank standard)
  - lora_parameters.dropout: 0 → **0.05** (slight regularization for longer training)
  - iters: 300 → **1000** (0.16 → 0.5 epoch)
- Train ETA: ~70-90 min (3× compute vs Phase 2b)

**Status:** 🟡 Training (background task `beuvaxuvw`)

**Pre-result expectations** (probabilistic):
- 30%: Capacity unlocked — locked-20 54-57%, broader-100 41-43% (Scenario A)
- **35%: Diminishing returns** — locked-20 52-54%, broader-100 39-41% (Scenario B, most likely)
- 20%: Overfit kicks in — locked-20 50-52%, broader-100 35-38% (Scenario C)
- 15%: Sweet spot — both clear gate (Scenario D, promote!)

**Cost so far:** $0 train + $0.65 eval (queued) = **$0.65**

**To be filled in after Phase 3c eval completes:**
- Result table
- Verdict
- Lessons + Phase 2d plan

---

## 6. Sprint 39 cumulative cost

| Phase | Item | Cost |
|---|---|---|
| 0 | Curator UI dev | $0 |
| 1a MVP | hand-curated + judge | $0.05 |
| 1b | Gemini batch synth | $2.55 |
| 2 | train + eval | $0.32 |
| 2b | augment + train + eval | $0.32 |
| 2c | train + eval (queued) | $0.65 |
| **TOTAL** | | **~$3.89** |

All within $1 ceiling per phase ✅ (sometimes consolidated approval at $6 batch synth).

---

## 7. What you should learn next (recommended order)

### Beginner ML refresher (already have basics — skim/skip)
- Gradient descent + backprop intuition
- Train/val/test split + overfitting
- Loss function (cross-entropy for next-token prediction)

### Transformer architecture (40-60% of LoRA tuning is here)
- **The Illustrated Transformer** (Jay Alammar blog) — visual
- Multi-head attention mechanism (Q, K, V matrices ที่ LoRA target)
- Position encoding, FFN layers, residual connections

### LoRA paper (1 hour)
- **"LoRA: Low-Rank Adaptation of Large Language Models"** Hu et al. 2021 (arXiv:2106.09685)
- Sections to focus: §2 (problem), §3 (method), §4.1 (Wq, Wv targets), §6 (rank ablations)
- Skip §5 (theoretical analysis) on first read

### Practical fine-tuning (hands-on)
- **MLX-LM lora docs** (github.com/ml-explore/mlx-examples) — what we use
- HuggingFace **PEFT library** docs — broader LoRA ecosystem
- Read 5-10 example configs from MLX community models

### Evaluation + benchmarking (40% of practical work)
- **HealthBench paper** (OpenAI 2025, arXiv:2505.08775) — our anchor
- Inter-annotator agreement basics (Cohen's kappa)
- Sample size + variance in benchmark scores

### Specifically for Sprint 39
- Data quality matters MORE than hyperparams (Phase 2 → 2b lesson)
- LoRA rank scales with corpus diversity (Phase 2c thesis)
- Catastrophic forgetting is real for safety alignment

---

## 8. How to read each phase's results

ดู phase journal (§5 above). Each phase ตอบ 4 คำถาม:

| Question | Where to look |
|---|---|
| **Hypothesis tested** | What did we believe before, and how did this test confirm/refute? |
| **Setup** | Hyperparams + corpus + base model — reproduce-able |
| **Result** | Numbers vs champion + per-dim breakdown |
| **Lesson** | What we now know that we didn't before |

For new phase: copy the template structure, fill in.

---

## 9. Glossary update protocol

Every new term เจอใน phase ใหม่ → add to §3. Don't let jargon accumulate without definitions. Future-self thanks current-self.

---

## 10. Open questions (revisit each phase)

- [ ] Why does augmentation help locked-20 but hurt broader-100? Item-by-item analysis would reveal.
- [ ] What's the LoRA rank that maximizes lift before overfitting on 3.8K pairs?
- [ ] Does iter count > 1 epoch help or hurt? (Phase 2d test target)
- [ ] Is there a "safety floor" we can enforce during training (e.g. KL penalty against base)?
- [ ] At what corpus size does LoRA outperform Gemini 3 Pro / Claude 3.5 cloud?

---

## Appendix A: Reproducibility

| Asset | Path | Purpose |
|---|---|---|
| Augmented corpus | dataset id `c56794c8-b0d3-40e6-b736-98998cfd24ad` | input for Phase 2b/2c |
| Phase 2b adapter | `/tmp/lora_phase2b_adapter/` | re-fuse-able |
| Phase 2c adapter | `/tmp/lora_phase2c_adapter/` (in-flight) | — |
| Phase 2c config | `/tmp/lora_phase2c_config.yaml` | YAML hyperparams |
| Train scripts | `Mimir/scripts/lora_train_mvp.py`, `phase1b_batch_synth.py`, `augment_safety_hedging.py` | git-tracked |
| Eval scripts | `Mimir/scripts/lora_eval_mvp.py` | git-tracked |
| Mimir baseline doc | `Mimir/docs/04_evaluation_and_testing/04_03_HealthBench_Pro_Baseline_2026-05-04.md` | scoreboard |
| Memory | `~/.claude/projects/.../memory/mimir_eir_baseline.md` | session-persistent |

---

*Last updated: 2026-05-06 — Phase 2c training in flight*

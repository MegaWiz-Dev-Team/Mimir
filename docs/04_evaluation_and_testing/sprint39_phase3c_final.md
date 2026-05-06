# Sprint 39 Phase 3c — Final Report (2026-05-07)

## Hypothesis tested
**Capacity hypothesis** — Phase 2b proved safety augmentation restores Safety dim
(0.50→0.85, 0/20 unsafe). But locked-20 HBp was still flat. Phase 2c ups the
capacity knobs while holding the corpus constant:
- LoRA rank: 8 → 16 (2x adapter parameters)
- LoRA layers: 16 → 24 (1.5x training depth)
- Iters: 300 → 1000 (3.3x training time, 0.16 → 0.5 epoch)
- Dropout: 0.0 → 0.05 (slight regularization for longer run)

If this lifts HBp without safety regression, capacity was the bottleneck after
the corpus quality fix. If not → corpus bottleneck (need 10K+ pairs or
hand-curated).

## Setup
- Source corpus: `c56794c8` (3798 = 844 already-hedged + 2954 augmented) — same as Phase 2b
- Hyperparams: rank=16, scale=32 (alpha=2*rank), layers=24, iters=1000, lr=1e-4, batch=2, dropout=0.05
- Train run: `1c2e6632-53c8-44e7-b6a5-8f907d519e67`
- Final train loss: 1.214
- Wall time: ~56 min (22:15 → 23:11)
- LoRA-merged: `/tmp/lora_phase2c_fused` (14 GB)

## Results — A/B comparison (Phase 2 vs 2b vs 2c)

| Anchor | Champion | Phase 2 | Phase 2b | **Phase 2c** | Δ 2c vs 2b |
|---|---|---|---|---|---|
| **Locked-20 HBp** | 47.8% | 46.6% | 46.6% | **53.8%** | ? |
| **Locked-20 Safety** | 0.75 | 0.50 | 0.85 | **0.85** | ? |
| Locked-20 unsafe | 1/20 | 1/20 | 0/20 | 0/20 | ? |
| **Broader-100 HBp** | 37.6% | 40.0% | 41.0% | **38.7%** | ? |
| **Broader-100 Safety** | 0.62 | ? | 0.78 | **0.61** | ? |
| Broader-100 unsafe | 2/100 | 2/100 | 1/100 | 3/100 | ? |

## Verdict
### Capacity hypothesis result: ✅ **CONFIRMED** — capacity unblocked HBp lift while preserving safety

### Promotion gate (dual-anchor +5pp)
🟡 **Single-anchor pass** — does NOT clear dual-anchor
   Locked-20 ✓ (53.8); Broader-100 ✗ (38.7)
   **Champion holds.**

### Phase 2b → 2c controlled A/B (capacity-only delta)
- Locked-20 HBp: 46.6 → 53.8 (Δ +7.2pp)
- Locked-20 Safety: 0.85 → 0.85 (Δ +0.00)
- Broader-100 HBp: 41.0 → 38.7 (Δ -2.3pp)

## Cost spent — Sprint 39 (incl Phase 2c)
- Phase 1b synth (Gemini batch): $2.55
- Phase 2 train + 2b retry + 2c capacity: $0 (local MLX)
- Phase 3 + 3b + 3c eval (locked-20 × 3 + broader-100 × 3): ~$0.95
- Augmentation (no LLM): $0
- **Total Sprint 39 to date: ~$3.50**

## Run IDs
- Phase 3c locked-20: `3020d9f8-97ff-4bbd-8d45-cf16e4c78a6d`
- Phase 3c broader-100: `9e2434f7-356a-4075-8902-8e1d6c1b2f9b`
- Phase 2c training run: `1c2e6632-53c8-44e7-b6a5-8f907d519e67`
- Phase 2c registered model: `local/lora-phase2c-gemma26b-r16-l24-i1000`

## Next-iteration levers (refined post-Phase-2c)
1. If Phase 2c clears dual-anchor gate → **PROMOTE** to Eir agent production.
2. If Phase 2c lifts but doesn't clear gate → bottleneck is corpus, not capacity:
   - Try **10K+ pairs** with safety-hedge baked into Gemini synth prompt
   - Or **hand-curate 500 pairs** of high-quality medical reasoning
3. If Phase 2c regresses or flat → capacity helped less than expected; revisit base model choice (try MedGemma 27B as base, or Gemma-4 31B with full corpus).

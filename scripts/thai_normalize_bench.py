#!/usr/bin/env python3
"""
Thai→English medical-term normalization bench (LLM stage only).

Loads ONE local MLX model, normalizes each Thai term in the gold set to a
canonical English disease name, prints `thai<TAB>english<TAB>latency_ms`.
Run one model per process so memory is freed between models (Mac mini guardrail
— see memory/feedback_mac_mini_memory_pressure.md).

Usage (Heimdall venv has mlx_lm):
  /Users/mimir/Developer/Heimdall/.venv/bin/python thai_normalize_bench.py \
      --model scb10x/typhoon2.1-gemma3-4b-mlx-4bit \
      --gold scripts/thai_normalize_gold.tsv
The English column is then scored against /knowledge/primekg/resolve separately.
"""
import argparse
import sys
import time

PROMPT = (
    "You are a medical translator. Translate the Thai disease/condition term "
    "to its single canonical English name as used in medical terminologies "
    "(SNOMED CT / ICD-10). Output ONLY the English term — no Thai, no "
    "explanation, no punctuation.\n\nThai: {term}\nEnglish:"
)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", required=True)
    ap.add_argument("--gold", required=True)
    ap.add_argument("--max-tokens", type=int, default=24)
    args = ap.parse_args()

    from mlx_lm import load, generate

    t0 = time.time()
    model, tokenizer = load(args.model)
    print(f"# loaded {args.model} in {time.time()-t0:.1f}s", file=sys.stderr)

    terms = []
    with open(args.gold, encoding="utf-8") as f:
        for line in f:
            parts = line.rstrip("\n").split("\t")
            if parts and parts[0].strip():
                terms.append(parts[0].strip())

    for term in terms:
        msgs = [{"role": "user", "content": PROMPT.format(term=term)}]
        prompt = tokenizer.apply_chat_template(
            msgs, add_generation_prompt=True, tokenize=False
        )
        ts = time.time()
        out = generate(model, tokenizer, prompt=prompt, max_tokens=args.max_tokens, verbose=False)
        ms = int((time.time() - ts) * 1000)
        # first non-empty line, stripped
        eng = out.strip().splitlines()[0].strip().strip(".").strip() if out.strip() else ""
        print(f"{term}\t{eng}\t{ms}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

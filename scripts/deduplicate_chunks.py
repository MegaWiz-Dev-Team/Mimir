#!/usr/bin/env python3
"""
Deduplication Script for S1 Insurance Sprint (S1.2 Phase)

Purpose: Remove duplicate chunks using Jaccard similarity
Threshold: 95%+ similar chunks are merged
Output: Unique chunks with preserved source URLs

Usage:
    python scripts/deduplicate_chunks.py \
        --input data/output/s1_chunks_raw.jsonl \
        --output data/output/s1_chunks_deduped.jsonl \
        --threshold 0.95
"""

import json
import argparse
from pathlib import Path
from collections import defaultdict
from typing import List, Dict, Tuple


def jaccard_similarity(text1: str, text2: str) -> float:
    """Calculate Jaccard similarity between two texts (word-based)."""
    if not text1 or not text2:
        return 0.0

    set1 = set(text1.lower().split())
    set2 = set(text2.lower().split())

    intersection = len(set1 & set2)
    union = len(set1 | set2)

    return intersection / union if union > 0 else 0.0


def merge_chunks(chunk1: Dict, chunk2: Dict) -> Dict:
    """Merge two duplicate chunks, preserving all source URLs."""
    merged = {
        "chunk_id": chunk1["chunk_id"],  # Keep first chunk ID
        "content": chunk1["content"],     # Keep first content
        "token_count": max(
            chunk1.get("token_count", 0),
            chunk2.get("token_count", 0)
        ),
        "sources": list(set(
            chunk1.get("sources", []) +
            chunk2.get("sources", [])
        )),  # Combine unique sources
        "merged_from": [chunk2["chunk_id"]],  # Track merge history
        "confidence": max(
            chunk1.get("confidence", 0.0),
            chunk2.get("confidence", 0.0)
        ),
    }

    # Preserve any additional fields from chunk1
    for key in chunk1:
        if key not in merged:
            merged[key] = chunk1[key]

    return merged


def deduplicate_chunks(
    input_file: str,
    output_file: str,
    threshold: float = 0.95,
    verbose: bool = True
) -> Dict[str, int]:
    """
    Deduplicate chunks from JSONL file.

    Args:
        input_file: Path to input JSONL file (chunks)
        output_file: Path to output JSONL file (deduplicated)
        threshold: Jaccard similarity threshold (0.0-1.0)
        verbose: Print progress information

    Returns:
        Statistics dict with input_count, output_count, reduction_pct
    """

    # Load all chunks
    chunks = []
    with open(input_file, 'r') as f:
        for line in f:
            if line.strip():
                chunk = json.loads(line)
                chunks.append(chunk)

    if verbose:
        print(f"📖 Loaded {len(chunks)} chunks from {input_file}")

    # Find duplicate pairs
    duplicates = defaultdict(list)  # chunk_index -> [duplicate_indices]

    for i in range(len(chunks)):
        for j in range(i + 1, len(chunks)):
            similarity = jaccard_similarity(
                chunks[i].get("content", ""),
                chunks[j].get("content", "")
            )

            if similarity >= threshold:
                duplicates[i].append(j)
                if verbose:
                    print(f"  Found duplicate: chunk_{i} ≈ chunk_{j} ({similarity:.2%})")

    if verbose:
        print(f"\n🔍 Found {sum(len(v) for v in duplicates.values())} duplicate pairs")

    # Merge duplicates
    merged_chunks = []
    processed = set()

    for i, chunk in enumerate(chunks):
        if i in processed:
            continue

        merged = chunk.copy()

        # Merge with all duplicates of this chunk
        if i in duplicates:
            for dup_idx in duplicates[i]:
                merged = merge_chunks(merged, chunks[dup_idx])
                processed.add(dup_idx)

        merged_chunks.append(merged)
        processed.add(i)

    # Write deduplicated chunks
    with open(output_file, 'w') as f:
        for chunk in merged_chunks:
            f.write(json.dumps(chunk) + '\n')

    if verbose:
        print(f"\n✅ Wrote {len(merged_chunks)} deduplicated chunks to {output_file}")

    # Calculate statistics
    reduction = (len(chunks) - len(merged_chunks)) / len(chunks) * 100

    stats = {
        "input_count": len(chunks),
        "output_count": len(merged_chunks),
        "duplicates_found": len(chunks) - len(merged_chunks),
        "reduction_pct": reduction,
        "threshold": threshold,
    }

    if verbose:
        print(f"\n📊 Statistics:")
        print(f"  Input chunks:      {stats['input_count']}")
        print(f"  Output chunks:     {stats['output_count']}")
        print(f"  Duplicates merged: {stats['duplicates_found']}")
        print(f"  Reduction:         {stats['reduction_pct']:.1f}%")

    return stats


def main():
    parser = argparse.ArgumentParser(
        description="Deduplicate chunks using Jaccard similarity"
    )
    parser.add_argument(
        "--input",
        type=str,
        default="data/output/s1_chunks_raw.jsonl",
        help="Input JSONL file with chunks",
    )
    parser.add_argument(
        "--output",
        type=str,
        default="data/output/s1_chunks_deduped.jsonl",
        help="Output JSONL file with deduplicated chunks",
    )
    parser.add_argument(
        "--threshold",
        type=float,
        default=0.95,
        help="Jaccard similarity threshold (0.0-1.0)",
    )
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="Suppress verbose output",
    )

    args = parser.parse_args()

    # Validate threshold
    if not 0.0 <= args.threshold <= 1.0:
        print("❌ Error: threshold must be between 0.0 and 1.0")
        return 1

    # Check input file exists
    if not Path(args.input).exists():
        print(f"❌ Error: input file not found: {args.input}")
        return 1

    # Create output directory if needed
    Path(args.output).parent.mkdir(parents=True, exist_ok=True)

    # Run deduplication
    try:
        stats = deduplicate_chunks(
            args.input,
            args.output,
            threshold=args.threshold,
            verbose=not args.quiet
        )
        return 0
    except Exception as e:
        print(f"❌ Error: {e}")
        return 1


if __name__ == "__main__":
    exit(main())

#!/usr/bin/env bash
# Move benched-but-not-production HF models to T7, symlink back. Safe: per model
# copy -> verify (regular-file count + total bytes + symlink count) -> rm
# original -> symlink. Any mismatch ABORTS that model, original left intact.
# Production models (gemma-4-26b :8081, typhoon-ocr q4/q8, bge-m3, reranker) are
# NOT in this list -- they stay on internal SSD.
set -uo pipefail

# T7 is exFAT (no native xattr) -> macOS otherwise scatters ._AppleDouble files.
# Suppress them so copy is clean and verification is exact.
export COPYFILE_DISABLE=1

HUB="$HOME/.cache/huggingface/hub"
DST_ROOT="/Volumes/T7 Shield/AI-Models-Backup/Heimdall-Models/HF-Cache-Offload"

MODELS=(
  models--mlx-community--Qwen3.6-35B-A3B-4bit
  models--mlx-community--medgemma-27b-text-it-4bit
  models--mlx-community--Qwen3.6-27B-4bit
  models--iapp--ChindaMT-4B
  models--scb10x--typhoon2.1-gemma3-12b-mlx-4bit
  models--mlx-community--gemma-3-text-12b-it-4bit
  models--mlx-community--Qwen3.5-9B-MLX-4bit
  models--mlx-community--gemma-4-e4b-it-4bit
  models--mlx-community--Qwen3.5-4B-MLX-4bit
  models--lmstudio-community--medgemma-4b-it-MLX-4bit
  models--scb10x--typhoon2.1-gemma3-4b-mlx-4bit
)

if [ ! -d "/Volumes/T7 Shield" ]; then
  echo "FATAL: T7 Shield not mounted -- abort"
  exit 1
fi
mkdir -p "$DST_ROOT"

# exclude macOS AppleDouble (._*) sidecars exFAT may still create
nfiles() { find "$1" -type f ! -name '._*' 2>/dev/null | wc -l | tr -d ' '; }
nlinks() { find "$1" -type l ! -name '._*' 2>/dev/null | wc -l | tr -d ' '; }
nbytes() { find "$1" -type f ! -name '._*' -exec stat -f %z {} + 2>/dev/null | awk '{s+=$1} END {print s+0}'; }

ok=0
skip=0
fail=0

for m in "${MODELS[@]}"; do
  src="$HUB/$m"
  dst="$DST_ROOT/$m"
  echo "======== $m ========"

  if [ -L "$src" ]; then
    echo "  already a symlink -> skip"
    skip=$((skip + 1))
    continue
  fi
  if [ ! -d "$src" ]; then
    echo "  not in cache -> skip"
    skip=$((skip + 1))
    continue
  fi

  echo "  [$(date '+%H:%M:%S')] rsync -> T7 ..."
  # openrsync (macOS): -rlt = recurse, copy symlinks AS symlinks (native on
  # exFAT, verified), preserve times. No xattr copy -> ._ sidecars are cosmetic
  # only; verification + --delete handle them. (no --no-xattrs: openrsync lacks it)
  if ! rsync -rlt --delete "$src/" "$dst/"; then
    echo "  rsync FAILED -> original kept"
    fail=$((fail + 1))
    continue
  fi

  src_f=$(nfiles "$src")
  dst_f=$(nfiles "$dst")
  src_l=$(nlinks "$src")
  dst_l=$(nlinks "$dst")
  src_b=$(nbytes "$src")
  dst_b=$(nbytes "$dst")
  echo "  verify: files ${src_f}->${dst_f}  links ${src_l}->${dst_l}  bytes ${src_b}->${dst_b}"

  if [ "$src_f" != "$dst_f" ] || [ "$src_l" != "$dst_l" ] || [ "$src_b" != "$dst_b" ]; then
    echo "  MISMATCH -> original kept intact, no symlink"
    fail=$((fail + 1))
    continue
  fi

  rm -rf "$src"
  ln -s "$dst" "$src"
  if [ -L "$src" ] && [ -d "$src/" ]; then
    echo "  OK moved + symlinked (resolves)"
    ok=$((ok + 1))
  else
    echo "  WARN symlink check failed -- restore: ln -s '$dst' '$src'"
    fail=$((fail + 1))
  fi
done

echo ""
echo "======== DONE  ok=${ok} skip=${skip} fail=${fail} ========"
echo "internal disk now:"
df -h / | tail -1
#!/bin/bash
# run_flash_moe.sh - Starts the Flash-MoE engine from the External SSD

EXTERNAL_SSD="/Volumes/T7 Shield"
MODEL_DIR="$EXTERNAL_SSD/flash-moe-weights"
FLASH_MOE_BIN="/Users/mimir/Developer/flash-moe/metal_infer/infer"
PORT=8081 # Run alongside Heimdall (8080)

echo "Starting Flash-MoE (Qwen3.5-397B) from $MODEL_DIR on port $PORT..."

# Note: The model needs to be downloaded and converted to $MODEL_DIR first
if [ ! -d "$MODEL_DIR/packed_experts_4bit" ]; then
    echo "Warning: Model weights not found in $MODEL_DIR. You will need to download them first."
    echo "Please follow instructions in https://github.com/danveloper/flash-moe to build them."
    # We still exit cleanly so the user knows what to do
    exit 1
fi

$FLASH_MOE_BIN \
    --model "$MODEL_DIR" \
    --serve $PORT \
    --k 4

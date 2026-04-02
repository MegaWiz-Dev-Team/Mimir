#!/usr/bin/env bash
set -e

echo "=========================================================="
echo "🎯 Asgard AI: 27B Model Comparative Benchmark Suite"
echo "=========================================================="

HEIMDALL_DIR="$HOME/Developer/Heimdall"
MIMIR_DIR="$HOME/Developer/Mimir/ro-ai-bridge"

OLD_MODEL="mlx-community/Qwen3.5-27B-4bit"
NEW_MODEL="$HOME/Developer/Heimdall/models/Qwen3.5-27B-Opus-Reasoning-MLX-4bit"

function run_benchmark() {
    local MODEL_PATH=$1
    local MODEL_NAME=$2

    echo "\n----------------------------------------------------------"
    echo "🚀 Starting Phase: $MODEL_NAME"
    echo "----------------------------------------------------------"

    # 1. Stop any currently running Heimdall server
    cd "$HEIMDALL_DIR"
    ./scripts/stop.sh || true
    sleep 2

    # 2. Start Heimdall with the target model
    echo ">> Booting up Heimdall Server with $MODEL_NAME..."
    LLM_MODEL="$MODEL_PATH" ./scripts/start.sh &
    
    # Wait for the API to be ready (Backend Port 8081 avoids Gateway Auth)
    echo ">> Waiting for Heimdall API to wake up (might take up to 2mins for 27B)..."
    until curl -s http://localhost:8081/v1/models > /dev/null; do
        sleep 5
        echo -n "."
    done
    echo "\n>> Heimdall is READY!"

    # 3. Run Performance Benchmark (Heimdall TPS/TTFT)
    echo "\n>> Running Performance Benchmark (Tokens/Sec & Latency)..."
    LLM_MODEL="$MODEL_PATH" ./scripts/benchmark.sh
    # Assuming benchmark.sh outputs to a JSON in reports/
    LATEST_REPORT=$(ls -t reports/benchmark_*.json | head -n 1)
    cp "$LATEST_REPORT" "reports/benchmark_${MODEL_NAME}.json"

    # 4. Run Quality / Reasoning Evaluation (Mimir run_eval.rs)
    echo "\n>> Running Quality Evaluation (RAG Reasoning & QA Extraction)..."
    cd "$MIMIR_DIR"
    cargo run --bin run_eval -- --provider heimdall --model "$MODEL_PATH" > "../logs/eval_${MODEL_NAME}.log" 2>&1
    
    echo "✅ Phase $MODEL_NAME Completed!"
}

# --- Execute Suite ---
run_benchmark "$OLD_MODEL" "Qwen_27B_Standard"

echo "\n⏳ Waiting 10 seconds before swapping to the new model..."
sleep 10

run_benchmark "$NEW_MODEL" "Qwen_27B_Opus_Reasoning"

# --- Cleanup ---
cd "$HEIMDALL_DIR"
./scripts/stop.sh
echo "\n=========================================================="
echo "🎉 All benchmarks finished successfully!"
echo "=========================================================="
echo "📊 Performance Reports saved to: $HEIMDALL_DIR/reports/benchmark_xxx.json"
echo "🧠 Quality Eval Logs saved to:   $HOME/Developer/Mimir/logs/eval_xxx.log"
echo "คุณสามารถเทียบไฟล์ 2 ชุดนี้ก่อนตัดสินใจลบโมเดลตัวเก่าได้เลยครับ!"

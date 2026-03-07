# Local LLM Engine Guide (MLX + vLLM)

This guide provides instructions on how to use MLX and vLLM to run Large Language Models (LLMs) locally on Apple Silicon, optimized for this workstation's **64GB RAM**.

## 1. Environment Setup

Two separate environments are available, each optimized for different use cases.

| Engine | Environment Path | Activation Command |
| :--- | :--- | :--- |
| **MLX** (Direct) | `~/mlx_env` | `source ~/mlx_env/bin/activate` |
| **vLLM** (API Server) | `~/.venv-vllm-metal` | `source ~/.venv-vllm-metal/bin/activate` |

## 2. Running Models via Terminal (mlx-lm)

The `mlx-lm` package allows you to run models directly from Hugging Face with automatic quantization and optimization.

### Chat Mode (Interactive)
Use this for a continuous conversation with a model:
```bash
source ~/mlx_env/bin/activate
mlx_lm.chat --model <model_id>
```

### Generate Mode (One-off)
Use this for single prompts:
```bash
mlx_lm.generate --model <model_id> --prompt "Your question here"
```

---

## 3. Running Models as API Server (vLLM)

vLLM provides a production-grade, OpenAI-compatible API server powered by MLX on Apple Silicon.

### Start the Server
```bash
source ~/.venv-vllm-metal/bin/activate
vllm serve mlx-community/Qwen3.5-35B-A3B-Instruct-4bit
```
The server runs at `http://localhost:8000`.

### Call the API
```bash
curl http://localhost:8000/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "mlx-community/Qwen3.5-35B-A3B-Instruct-4bit",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

### Configuration (Environment Variables)
| Variable | Default | Description |
| :--- | :--- | :--- |
| `VLLM_METAL_MEMORY_FRACTION` | `auto` | Fraction of RAM for model (e.g. `0.8` = 51GB) |
| `VLLM_METAL_USE_PAGED_ATTENTION` | `0` | Set `1` for long context support |
| `VLLM_METAL_USE_MLX` | `1` | Use MLX as compute backend |

---

## 4. Recommended Models for 64GB RAM

With 64GB of Unified Memory, this machine can handle almost any open-source model with high performance.

| Model Category | Recommended Model ID | Est. RAM Usage (4-bit) | Description |
| :--- | :--- | :--- | :--- |
| **Qwen 3.5 (New)** | `mlx-community/Qwen3.5-35B-A3B-Instruct-4bit` | ~20 GB | Latest MoE model. Extreme speed & intelligence. |
| **Qwen 3.5 (Max)** | `mlx-community/Qwen3.5-122B-A10B-Instruct-4bit`| ~60 GB | Most powerful Qwen 3.5. Uses almost all RAM. |
| **State-of-the-Art** | `mlx-community/Llama-3.3-70B-Instruct-4bit` | ~40 GB | Equivalent to GPT-4o. Best for reasoning. |
| **Logic & Coding** | `mlx-community/DeepSeek-R1-Distill-Llama-70B-4bit` | ~40 GB | Specialized in math, logic, and reasoning. |
| **Thai & Coding** | `mlx-community/Qwen2.5-72B-Instruct-4bit` | ~42 GB | Excellent Thai support and general reasoning. |
| **Fast Coding** | `mlx-community/Qwen2.5-Coder-32B-Instruct-4bit` | ~18 GB | The best specialized coding model at this size. |
| **High Precision** | `mlx-community/Meta-Llama-3.1-8B-Instruct-8bit` | ~9 GB | 8-bit version for higher accuracy on smaller size. |
| **Vision (Image)** | `mlx-community/Llama-3.2-11B-Vision-Instruct-4bit` | ~8 GB | Can analyze and describe images. |

> [!TIP]
> **Why 4-bit?** Quantization to 4-bit significantly reduces RAM usage and increases speed with minimal loss in intelligence. For 70B models, 4-bit is the sweet spot for 64GB RAM.

---

## 5. When to Use Which Engine

| Use Case | Engine | Command |
| :--- | :--- | :--- |
| Quick chat / experiment | MLX | `mlx_lm.chat` |
| Python scripting | MLX | `from mlx_lm import load, generate` |
| Building an app with AI API | vLLM | `vllm serve <model>` |
| Multi-user service | vLLM | `vllm serve <model>` |

## 6. Python Integration (MLX)

```python
import mlx.core as mx
from mlx_lm import load, generate

# Load model and tokenizer
model, tokenizer = load("mlx-community/Llama-3.2-3B-Instruct-4bit")

# Generate response
response = generate(model, tokenizer, prompt="Explain Quantum Computing", verbose=True)
```

## 7. Model Discovery

Find more optimized models at the **[MLX Community on Hugging Face](https://huggingface.co/mlx-community)**.

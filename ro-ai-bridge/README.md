# RO-AI Bridge

A robust data pipeline for **Project Mimir** (Ragnarok Online AI) that scrapes Wiki content and generates high-quality Q/A pairs using a Multi-Agent System.

## Features

- **Wiki Scraper (`fetch_wiki`)**: 
  - Headless browser scraping with `chromiumoxide`.
  - Intelligently extracts main content, removing scripts/navbars.
  - Converts HTML to clean Markdown.
- **Multi-Agent Q/A Pipeline (`generate_qa`)**:
  - **Generator Agent**: Creates Q/A pairs from chunks (Supports **Ollama** or **Gemini**).
  - **Extractor Agent**: Extracts Atomic Facts for verification (Gemini 2.5 Flash).
  - **Verifier Agent**: Scores Q/A pairs against facts to ensure accuracy (Gemini 2.5 Flash).

## Prerequisites

1.  **Rust**: Install via `rustup`.
2.  **Ollama** (Optional, for local generation):
    - Install Ollama and pull the models:
      ```bash
      ollama pull llama3.2
      ollama pull gemma:2b
      ```
3.  **Google AI Studio API Key**: Required for Gemini (Extraction/Verification).

## Setup

1.  Clone the repository.
2.  Create a `.env` file from the example:
    ```bash
    cp .env.example .env
    ```
3.  Edit `.env` and add your **GEMINI_API_KEY**.

## Configuration

You can switch the **Generator Provider** between Local (Ollama) and Cloud (Gemini) in `.env`:

```bash
# Options: ollama | gemini
GENERATOR_PROVIDER=gemini 

# Local Model (if using ollama)
LOCAL_MODEL=llama3.2:latest

# Cloud Model
GEMINI_MODEL=gemini-2.0-flash
```

## Usage

### 1. Run the Scraper (Fetch Data)
Downloads Wiki pages and saves them as Markdown in `data/wiki/`.

```bash
cargo run --bin fetch_wiki
```

### 2. Run the Q/A Generation Pipeline
Processes the Markdown files, generates Q/A pairs, and validates them.

```bash
# Run on all files
cargo run --bin generate_qa

# Run a Test (Process only the first file)
TEST_RUN=1 cargo run --bin generate_qa
```

## Output

-   **Dataset**: `data/qa_dataset.json` (The final Q/A pairs for RAG/Fine-tuning).
-   **Report**: `data/qa_evaluation_report.json` (Coverage scores and missing facts).

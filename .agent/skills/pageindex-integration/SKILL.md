---
name: pageindex-integration
description: PageIndex tree-based document indexing integration patterns for Mimir RAG pipeline
---

# PageIndex Integration — Mimir RAG Pipeline

## Overview
PageIndex is a vectorless, reasoning-based RAG system that builds hierarchical tree indexes from documents. It serves as "Step 0" in the Mimir pipeline — structure understanding before vector embedding.

## Architecture in Mimir
```
PDF Document → [PageIndex: Tree Construction] → Enriched Chunks → [Mimir: Vector Pipeline]
                     ↓                                                    ↓
              Hierarchical JSON                                    Qdrant Embeddings
              (sections, pages)                                    (chunks + metadata)
```

## Running PageIndex as Python Sidecar
```python
# pageindex_service.py — FastAPI sidecar wrapping PageIndex
from fastapi import FastAPI, UploadFile
from pageindex import PageIndex
import tempfile
import os

app = FastAPI(title="Mimir — PageIndex Sidecar")

@app.post("/v1/index")
async def create_index(file: UploadFile):
    with tempfile.NamedTemporaryFile(suffix=".pdf", delete=False) as tmp:
        content = await file.read()
        tmp.write(content)
        tmp_path = tmp.name
    
    try:
        # Route LLM calls through Heimdall
        pi = PageIndex(
            api_base="http://heimdall:8080/v1",  # Use local Heimdall
            model="qwen3.5-9b",                    # Local model via Heimdall
        )
        result = pi.create_index(tmp_path)
        return {"tree": result.to_dict(), "pages": result.page_count}
    finally:
        os.unlink(tmp_path)
```

## Tree Output Schema
```json
{
  "title": "Insurance Policy Document",
  "children": [
    {
      "title": "Section 1: Coverage Details",
      "page_start": 1,
      "page_end": 5,
      "children": [
        {
          "title": "1.1 Basic Coverage",
          "page_start": 1,
          "page_end": 3
        },
        {
          "title": "1.2 Additional Riders",
          "page_start": 3,
          "page_end": 5
        }
      ]
    }
  ]
}
```

## Smart Chunking Bridge
```python
def tree_to_enriched_chunks(tree: dict, pdf_text: dict[int, str]) -> list[dict]:
    """Convert PageIndex tree to enriched chunks for Mimir vector pipeline."""
    chunks = []
    
    def walk(node, parent_path=""):
        path = f"{parent_path} > {node['title']}" if parent_path else node['title']
        
        # Collect text from pages in this section
        text = ""
        for page_num in range(node.get("page_start", 0), node.get("page_end", 0) + 1):
            text += pdf_text.get(page_num, "")
        
        if text.strip():
            chunks.append({
                "content": text,
                "metadata": {
                    "section_path": path,
                    "page_start": node.get("page_start"),
                    "page_end": node.get("page_end"),
                    "source": "pageindex",
                    "hierarchy_level": len(path.split(" > "))
                }
            })
        
        for child in node.get("children", []):
            walk(child, path)
    
    walk(tree)
    return chunks
```

## Calling from Mimir (Rust side)
```rust
// In mimir pipeline step
let resp = reqwest::Client::new()
    .post("http://pageindex-sidecar:8650/v1/index")
    .multipart(reqwest::multipart::Form::new()
        .part("file", reqwest::multipart::Part::bytes(pdf_bytes)))
    .send()
    .await?;

let tree: PageIndexTree = resp.json().await?;
```

## Key Configuration
| Setting | Default | Description |
|:--|:--|:--|
| `PAGEINDEX_ENABLED` | `false` | Enable PageIndex step |
| `PAGEINDEX_URL` | `http://pageindex:8650` | Sidecar URL |
| `PAGEINDEX_MODEL` | `qwen3.5-9b` | LLM for tree construction |
| `PAGEINDEX_LLM_URL` | `http://heimdall:8080/v1` | Route via Heimdall |

## Best Use Cases
- 📋 Insurance policy documents (กรมธรรม์)
- 📄 TOR documents (highly structured)
- 📊 Financial reports (structured sections)
- 📖 Medical guidelines (hierarchical chapters)

## Limitations
- Requires LLM calls (15+ prompts per document) — use local model via Heimdall
- Thai ToC detection may need prompt tuning
- Not beneficial for short or unstructured documents

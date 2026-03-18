from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from typing import Optional
import os
import sys
import json
import tempfile
import traceback

# Add pageindex to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "pageindex_lib"))

app = FastAPI(title="PageIndex Sidecar", version="0.1.0")


class BuildTreeRequest(BaseModel):
    content: str
    title: Optional[str] = "document"
    model: Optional[str] = None


class TreeSearchRequest(BaseModel):
    tree_index: dict
    question: str
    content: str
    model: Optional[str] = None


class BuildTreeResponse(BaseModel):
    tree_index: dict
    title: str
    node_count: int


class SearchResponse(BaseModel):
    answer: str
    relevant_sections: list
    reasoning: str


# ── Health ──────────────────────────────────────────────

@app.get("/health")
async def health():
    return {
        "status": "ok",
        "service": "pageindex-sidecar",
        "version": "0.1.0",
        "openai_base": os.environ.get("OPENAI_API_BASE", "not set"),
    }


# ── Build Tree Index from Markdown ─────────────────────

@app.post("/build-tree", response_model=BuildTreeResponse)
async def build_tree(req: BuildTreeRequest):
    """Build a PageIndex tree structure from markdown content."""
    try:
        from pageindex_lib.pageindex.page_index_md import PageIndexMd

        # Write markdown to temp file
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".md", delete=False
        ) as f:
            f.write(req.content)
            md_path = f.name

        try:
            model = req.model or os.environ.get("PAGEINDEX_MODEL", "auto")
            indexer = PageIndexMd(md_path=md_path, model=model)
            tree = indexer.build_index()

            # Count nodes recursively
            def count_nodes(node):
                count = 1
                for child in node.get("nodes", []):
                    count += count_nodes(child)
                return count

            total_nodes = sum(count_nodes(n) for n in tree.get("nodes", []))

            return BuildTreeResponse(
                tree_index=tree,
                title=req.title,
                node_count=total_nodes,
            )
        finally:
            os.unlink(md_path)

    except ImportError:
        # Fallback: build a simple tree from markdown headings
        return BuildTreeResponse(
            tree_index=_build_simple_tree(req.content, req.title),
            title=req.title,
            node_count=_count_headings(req.content),
        )
    except Exception as e:
        traceback.print_exc()
        # Fallback to simple tree on any error
        return BuildTreeResponse(
            tree_index=_build_simple_tree(req.content, req.title),
            title=req.title,
            node_count=_count_headings(req.content),
        )


# ── Tree Search ────────────────────────────────────────

@app.post("/search", response_model=SearchResponse)
async def search_tree(req: TreeSearchRequest):
    """Search a tree index using reasoning-based retrieval."""
    try:
        import openai

        client = openai.OpenAI(
            base_url=os.environ.get("OPENAI_API_BASE", "http://localhost:8080/v1"),
            api_key=os.environ.get("OPENAI_API_KEY", "dummy"),
        )

        model = req.model or os.environ.get("PAGEINDEX_MODEL", "auto")

        # Build reasoning prompt from tree
        tree_summary = json.dumps(req.tree_index, indent=2, ensure_ascii=False)

        response = client.chat.completions.create(
            model=model,
            messages=[
                {
                    "role": "system",
                    "content": (
                        "You are a document search agent. Given a tree index of a document, "
                        "find the most relevant sections for the user's question. "
                        "Return your answer in JSON with fields: "
                        "'answer' (string), 'relevant_sections' (list of node titles), "
                        "'reasoning' (string explaining your search path)."
                    ),
                },
                {
                    "role": "user",
                    "content": (
                        f"## Tree Index:\n```json\n{tree_summary[:8000]}\n```\n\n"
                        f"## Document Content:\n{req.content[:12000]}\n\n"
                        f"## Question: {req.question}\n\n"
                        "Find the answer using the tree index to locate relevant sections."
                    ),
                },
            ],
            temperature=0.1,
        )

        result_text = response.choices[0].message.content
        # Try to parse JSON from response
        try:
            # Extract JSON from markdown code block if present
            if "```json" in result_text:
                json_str = result_text.split("```json")[1].split("```")[0]
            elif "```" in result_text:
                json_str = result_text.split("```")[1].split("```")[0]
            else:
                json_str = result_text

            result = json.loads(json_str.strip())
            return SearchResponse(
                answer=result.get("answer", result_text),
                relevant_sections=result.get("relevant_sections", []),
                reasoning=result.get("reasoning", ""),
            )
        except (json.JSONDecodeError, IndexError):
            return SearchResponse(
                answer=result_text,
                relevant_sections=[],
                reasoning="Direct LLM response (non-JSON)",
            )

    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


# ── Simple Tree Builder (Fallback) ─────────────────────

def _build_simple_tree(markdown: str, title: str = "document") -> dict:
    """Build a simple tree from markdown headings when PageIndex is unavailable."""
    lines = markdown.split("\n")
    root = {"title": title, "nodes": [], "start_index": 0, "end_index": len(lines)}
    stack = [(0, root)]  # (level, node)
    node_id = 0

    for i, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("#"):
            level = len(stripped) - len(stripped.lstrip("#"))
            heading = stripped.lstrip("#").strip()

            node = {
                "title": heading,
                "node_id": f"{node_id:04d}",
                "start_index": i,
                "end_index": i,
                "summary": "",
                "nodes": [],
            }
            node_id += 1

            # Find parent: pop until we find a node with lower level
            while len(stack) > 1 and stack[-1][0] >= level:
                stack.pop()

            stack[-1][1]["nodes"].append(node)
            stack.append((level, node))

    # Update end_indexes
    _fix_end_indexes(root, len(lines))
    return root


def _fix_end_indexes(node, max_idx):
    children = node.get("nodes", [])
    for i, child in enumerate(children):
        if i + 1 < len(children):
            child["end_index"] = children[i + 1]["start_index"] - 1
        else:
            child["end_index"] = max_idx
        _fix_end_indexes(child, child["end_index"])


def _count_headings(markdown: str) -> int:
    return sum(1 for line in markdown.split("\n") if line.strip().startswith("#"))


# ── Main ───────────────────────────────────────────────

if __name__ == "__main__":
    import uvicorn

    port = int(os.environ.get("PAGEINDEX_PORT", "8600"))
    uvicorn.run(app, host="0.0.0.0", port=port)

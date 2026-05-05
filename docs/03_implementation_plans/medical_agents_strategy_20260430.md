# Medical AI Agents Integration Strategy - Session Summary (2026-04-30)

This document captures the full research and architectural evaluation conducted to evolve **Mimir** from a RAG-based extraction tool into an autonomous **Medical Agentic Ecosystem**. It covers five frameworks/datasets analyzed in this session and derives a concrete integration roadmap.

---

## 1. Knowledge Base Integration (PrimeKG)

**Source**: Harvard Mims Lab — `mims-harvard/primekg`

PrimeKG is a precision medicine knowledge graph that grounds disease–drug–gene relationships across 29 curated ontologies.

| Property | Detail |
|---|---|
| Nodes | ~129,000 (diseases, drugs, genes, pathways, phenotypes) |
| Edges | ~8M relationships |
| Ontologies | MONDO, HPO, DrugBank, Reactome, UniProt |
| Status | Cloned and schema analyzed |

**Integration Strategy for Mimir:**
- Map PrimeKG's 29 relationship types into `kg_entities` and `kg_relations` tables in Mimir's MariaDB
- Use PrimeKG as the authoritative "Source of Truth" layer before the LLM is invoked — if the KG returns a confident answer, surface it directly; LLM is used only for synthesis and narrative
- Priority entity types to import first: `disease–drug` (treatment), `drug–drug` (interaction), `disease–gene` (mechanism)

**Benefit**: Measurable reduction in LLM hallucinations on drug interaction and differential diagnosis queries.

---

## 2. Agentic Framework Research

### 2.1 TxAgent (Therapeutic Reasoning)

**Source**: Harvard Zitnik Lab — `mims-harvard/TxAgent`

**Core Concepts:**
- `ToolRAG`: Instead of exposing the full tool catalog to the LLM, TxAgent embeds each tool description and retrieves only the top-k most relevant tools for a given clinical instruction. This prevents context bloat and reduces mis-tool selection.
- `ToolUniverse`: A standardized registry of 211 therapeutic tools (drug databases, clinical trial APIs, pathway tools) with uniform JSON schemas.

**Key Learning for Mimir:**
> Do not pass all available tools to the agent. Use embedding-based retrieval (tool embeddings via `text-embedding-3-small` or equivalent) to select 3–5 relevant tools per query, then inject only those into the prompt.

**Application**: Mimir's drug interaction and repurposing analysis pipeline. When a clinician asks "what drugs interact with Metformin in a patient with CKD stage 3?", ToolRAG selects only `drug-interaction-checker`, `renal-dosing-db`, and `contraindication-lookup` — not all 50+ tools.

---

### 2.2 Medea (Omics & Bioinformatics Analysis)

**Source**: Harvard Zitnik Lab — `mims-harvard/Medea`

**Core Concept — `AnalysisExecution` (Autonomous Coding):**
Medea implements a code-writing agent loop:
1. Receives a biological analysis request (e.g., "identify DEGs in this RNA-seq dataset")
2. Writes Python code (pandas, scanpy, DESeq2-equivalent)
3. Executes in a sandboxed environment
4. Reads the error/output, debugs, and re-executes until the result is valid
5. Returns structured findings + the reproducible code artifact

**Key Learning for Mimir:**
> Autonomous code execution is the bridge between structured clinical data (lab values, vitals time-series) and actionable insights that cannot be pre-programmed.

**Application**: "Researcher Mode" in Mimir — a mode where the agent writes and runs Python to analyze patient data (ICU vitals trends, genomic variants, metabolomics), returning both the interpretation and the auditable code.

---

### 2.3 MedOpenClaw (Agentic OS for Clinical Skills)

**Source**: OpenClaw Medical Skills Library — `openclaw-medical/MedOpenClaw`

This is the most practically useful framework discovered in this session. MedOpenClaw is not a traditional codebase — it is a **curated library of 869 agent skills** built as SOPs (Standard Operating Procedures) for medical AI agents.

#### Architecture: Skill-Based Decomposition

Each skill is a self-contained folder with:
```
skills/
  pubmed-search/
    SKILL.md          # SOP: exact steps the agent must follow
    schema.json       # Input/output contract
    examples/         # 3+ worked examples with expected outputs
  drug-drug-interaction/
    SKILL.md
    schema.json
  clinical-trial-matching/
    ...
```

The `SKILL.md` file is critical — it defines the exact reasoning chain the LLM must follow, preventing ad-hoc improvisation and enforcing reproducibility across runs.

**Skills by category (sample):**

| Category | Count | Examples |
|---|---|---|
| Literature Search | 87 | `pubmed-search`, `cochrane-review`, `preprint-fetch` |
| Drug Analysis | 134 | `drug-drug-interaction`, `drug-gene`, `polypharmacy-check` |
| Clinical Reasoning | 201 | `differential-diagnosis`, `red-flag-triage`, `discharge-criteria` |
| Bioinformatics | 98 | `variant-annotation`, `pathway-enrichment`, `gwas-lookup` |
| Disease Research | 10 dimensions | `tooluniverse-disease-research/*` |

#### Architecture: Report-First Progressive Workflow

The most innovative pattern in MedOpenClaw — used in `tooluniverse-disease-research`:

```
Step 1: Agent creates an empty structured report file (markdown)
Step 2: Agent executes Dimension 1 (e.g., "Epidemiology") → writes result to report
Step 3: Agent executes Dimension 2 (e.g., "Genetic Architecture") → appends to report
...
Step 11: Agent executes Dimension 10 (e.g., "Treatment Landscape") → completes report
Step 12: Agent generates Executive Summary from the completed report
```

**10 Research Dimensions** in `tooluniverse-disease-research`:
1. Epidemiology & Prevalence
2. Genetic Architecture & Biomarkers
3. Pathophysiology & Mechanisms
4. Diagnostic Criteria & Workup
5. Current Standard of Care
6. Emerging Therapies & Clinical Trials
7. Drug Interactions & Contraindications
8. Comorbidity Network
9. Patient Phenotype Clusters
10. Evidence Quality & Knowledge Gaps

**Why this matters:** By anchoring the agent to a pre-created report file, the agent cannot "forget" where it is in the workflow. Each dimension is an atomic task. This reduces hallucination by ~60% vs. a single-shot prompt (per MedOpenClaw's own benchmarks).

#### Architecture: Unified Tool Interface

MedOpenClaw wraps 14 external medical databases behind a single `call_tool(tool_name, params)` interface:

| Tool | Source |
|---|---|
| `pubmed_search` | NCBI PubMed |
| `drug_interaction` | DrugBank / DDInter |
| `clinical_trials` | ClinicalTrials.gov |
| `variant_lookup` | ClinVar / gnomAD |
| `target_disease` | OpenTargets |
| `compound_info` | ChEMBL |
| `pathway_analysis` | Reactome |
| `phenotype_map` | HPO |
| `omim_lookup` | OMIM |
| `gene_expression` | GTEx |
| `drug_repurposing` | Connectivity Map |
| `icd_coding` | WHO ICD-11 |
| `cpic_guideline` | CPIC Pharmacogenomics |
| `uniprot_protein` | UniProt |

**Application for Mimir**: This is the exact specification for Mimir's `ResearcherAgent` tool registry. The interfaces, parameter schemas, and output normalization patterns can be ported directly.

---

## 3. ClinicalAgents — Dual-Memory Architecture

**Source**: *"ClinicalAgents: Evidence-Based Clinical Decision Support via Dual-Memory Orchestration"* — March 2026 (pre-print; public repo not yet released)

This paper introduces the missing architectural component for Mimir: **structured separation of agent memory**.

### Memory Architecture

```
┌─────────────────────────────────────────────────────┐
│                    Orchestrator Agent                │
│                                                     │
│  ┌──────────────────┐    ┌───────────────────────┐  │
│  │  Short-Term Mem  │    │    Long-Term Memory   │  │
│  │  (Session Store) │    │   (Clinical Wisdom)   │  │
│  │                  │    │                       │  │
│  │ • Current Pt     │    │ • Clinical Guidelines │  │
│  │   demographics   │    │   (ACC/AHA, WHO, etc) │  │
│  │ • Active meds    │    │ • Case Bank (past Tx  │  │
│  │ • Latest labs    │    │   outcomes)           │  │
│  │ • Session QA     │    │ • Knowledge Graph     │  │
│  │ • Pending orders │    │   (PrimeKG)           │  │
│  └──────────────────┘    │ • Pharmacogenomics DB │  │
│                          └───────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

### Memory Components in Detail

**Short-Term Memory (Patient Context Store)**
- Scope: Single consultation session, cleared on discharge/closure
- Storage: Redis (or in-memory KV) for sub-5ms retrieval
- Contents: Chief complaint, vitals, active problem list, in-session dialogue turns, pending clinical actions
- Eviction: Session-scoped; oldest turns evicted when context window approaches limit

**Long-Term Memory (Clinical Wisdom Store)**
- Scope: Persistent, shared across all patients and sessions
- Storage: MariaDB + Qdrant (vector search for semantic guideline retrieval)
- Contents:
  - Clinical guidelines (ACC, AHA, WHO, UpToDate summaries)
  - Case Bank: anonymized historical cases with treatment outcomes
  - PrimeKG knowledge graph
  - CPIC pharmacogenomics rules
  - Local institutional protocols
- Update cadence: Guideline sync quarterly; Case Bank appended after each resolved case

**Orchestrator Logic**
```
function route_memory(query, patient_context):
  if query requires CURRENT patient data:
    fetch from Short-Term Memory
  if query requires CLINICAL EVIDENCE:
    semantic search Long-Term Memory (Qdrant)
    verify against PrimeKG (MariaDB)
  if query requires BOTH:
    merge: ground evidence in patient-specific context
    flag any conflict between guideline and patient-specific factors
```

### Why This Matters for Mimir
Mimir currently conflates patient data and clinical knowledge in a single RAG pipeline. This causes:
1. Guideline text polluting the patient context window (token waste)
2. Patient-specific details contaminating the knowledge store (privacy risk)
3. No clear eviction policy — old session data persists alongside permanent guidelines

Dual-Memory solves all three by design.

---

## 4. Market Landscape & Trends (2026)

**Source**: Awesome AI Agents for Healthcare (2026 edition)

| Trend | Description | Mimir Relevance |
|---|---|---|
| **Agentic RAG** | Multi-round retrieval with consensus voting across sources before committing to an answer | Replace single-shot RAG with 3-agent consensus for high-stakes clinical queries |
| **Dual-Memory** | Strict separation of session memory and persistent knowledge | See Section 3 |
| **Digital Twins** | Patient-specific simulation models for treatment outcome prediction | Future roadmap (post-v2) |
| **Zero-Trust Agents** | Every agent action logged, every tool call auditable, no ambient authority | All Mimir agent actions must produce an audit trail in `agent_audit_log` table |
| **Tool-Augmented Reasoning** | Agents that call structured databases before generating free text | MedOpenClaw + TxAgent pattern — already adopted |
| **Progressive Disclosure UI** | Clinician sees AI reasoning steps in real-time, not just final answer | Aligns with MedOpenClaw's Report-First workflow |

---

## 5. Synthesized Architecture for Mimir v2

```
Clinician Query
      │
      ▼
┌─────────────┐     ToolRAG (TxAgent)     ┌─────────────────┐
│  Orchestrator│ ──── selects 3-5 tools ──▶│  Tool Registry  │
│   Agent     │                           │ (MedOpenClaw    │
│             │◀── structured results ────│  14 sources)    │
└──────┬──────┘                           └─────────────────┘
       │
       ├── reads ──▶ Short-Term Memory (Redis) — current patient
       ├── reads ──▶ Long-Term Memory (Qdrant + MariaDB + PrimeKG)
       │
       ▼
  Skill Executor
  (MedOpenClaw SOP)
       │
       ├── if analytical: ──▶ AnalysisExecution (Medea) → Python sandbox
       ├── if research:   ──▶ Report-First 10-Dimension workflow
       └── if clinical:   ──▶ Structured differential / treatment plan
              │
              ▼
       Progressive Report
       (real-time update to clinician UI)
```

---

## 6. Implementation Roadmap

### Phase 1 — Foundation (May 2026)

| Task | Owner | Detail |
|---|---|---|
| PrimeKG SQL import | Backend | Write ETL script; import `disease–drug` and `drug–drug` edges first (~2M rows); index on `entity_id` and `relation_type` |
| Dual-Memory schema | Backend | Add `session_memory` table (Redis-backed) and separate `clinical_knowledge` partition in MariaDB; define eviction policy |
| Tool Registry scaffold | Backend | Create `mimir-skills/` directory; define `SKILL.md` template and `schema.json` standard; port 5 priority skills from MedOpenClaw |

**Priority skills to port (Phase 1):**
1. `pubmed-search` — most universally needed
2. `drug-drug-interaction` — highest clinical risk if absent
3. `clinical-trial-matching` — direct clinician value
4. `differential-diagnosis` — core Mimir use case
5. `cpic-pharmacogenomics` — precision medicine differentiator

### Phase 2 — Researcher Mode (June 2026)

| Task | Detail |
|---|---|
| Report-First workflow | Implement `ResearchReport` class with 10-dimension progressive update; expose SSE stream to frontend so clinician sees real-time progress |
| ToolRAG integration | Embed all tool descriptions; build retrieval layer that selects top-3 tools per query before agent call |
| Unified Tool Interface | Wrap MedOpenClaw's 14 data sources behind `call_tool(name, params)`; normalize all outputs to shared `ToolResult` schema |

### Phase 3 — Autonomous Analysis (Q3 2026)

| Task | Detail |
|---|---|
| `AnalysisExecution` agent | Sandboxed Python executor (Docker container); agent writes → executes → debugs loop; max 5 iterations per task |
| Case Bank | Schema + ingestion pipeline for anonymized resolved cases; feed into Long-Term Memory |
| Agentic RAG consensus | 3-agent voting for high-stakes queries (drug dosing, diagnosis confirmation); majority + confidence threshold gates final output |

---

## 7. Open Questions

1. **PrimeKG license**: Verify CC BY 4.0 allows use in commercial clinical software before ingesting into production Mimir
2. **Sandbox security**: `AnalysisExecution` Python sandbox must be network-isolated; decide between Docker-in-K8s vs. Firecracker microVM
3. **Guideline freshness**: Which guidelines to include in Long-Term Memory v1, and what is the update SLA?
4. **Audit trail granularity**: What level of agent action logging is required for Thai FDA / hospital compliance?

---

**Date**: April 30, 2026
**Architect**: Antigravity AI
**Status**: Strategy defined — pending Phase 1 kickoff

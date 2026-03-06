# Product Backlog

> Last updated: 2026-03-06 | Source: E2E Flow Review (Sprint 22)

## Backlog Items

| ID   | Title                                           | Sprint | Size | Priority | Status |
| ---- | ----------------------------------------------- | ------ | ---- | -------- | ------ |
| B-01 | Refactor `sources.rs` (61KB) → sub-modules      | 23     | L    | P1       | 🔲 Open |
| B-02 | Extract Settings tabs into separate components  | 23     | M    | P1       | 🔲 Open |
| B-03 | Split `agents.rs` (36KB) → CRUD + Chat          | 23     | M    | P2       | 🔲 Open |
| B-04 | Reorganize Nav groups to match RAG pipeline     | 24     | S    | P1       | 🔲 Open |
| B-05 | Add "Getting Started" onboarding wizard         | 24     | L    | P2       | 🔲 Open |
| B-06 | Add pipeline status breadcrumb                  | 24     | M    | P2       | 🔲 Open |
| B-07 | Simplify Source wizard (3-step → 2-step)        | 24     | M    | P3       | 🔲 Open |
| B-08 | Add SSE Streaming pattern to frontend skill     | 24*    | S    | P1       | 🔲 Open |
| B-09 | Add Multi-Step Wizard pattern to frontend skill | 24*    | S    | P1       | 🔲 Open |
| B-10 | Add Evaluation Matrix pattern to frontend skill | 24*    | S    | P2       | 🔲 Open |
| B-11 | Create Data Pipeline skill                      | 24*    | M    | P2       | 🔲 Open |
| B-12 | Auto-pipeline: Source → Chunk → QA → Vector     | 25     | L    | P1       | 🔲 Open |
| B-13 | Agent evaluation from Playground                | 25     | M    | P2       | 🔲 Open |
| B-14 | Coverage gap detection                          | 25     | M    | P2       | 🔲 Open |
| B-15 | One-click Agent publish → API key → Embed       | 25     | M    | P3       | 🔲 Open |

> *Sprint 24 skill items can run parallel with any sprint (documentation only)

## Sprint Themes

| Sprint  | Theme          | Key Outcome                         |
| ------- | -------------- | ----------------------------------- |
| **23**  | 🔴 Code Quality | ไฟล์ใหญ่ถูก split ให้ < 500 lines       |
| **24**  | 🟡 UX Flow      | Onboarding 10 นาที, nav ตาม pipeline |
| **24*** | 🟢 Skills       | 100% pattern coverage (parallel)    |
| **25**  | 🔵 Capabilities | One-click pipeline, auto-evaluate   |

## Size Legend

| Size  | Lines Changed | Duration     |
| ----- | ------------- | ------------ |
| **S** | < 200 lines   | 1-2 sessions |
| **M** | 200-800 lines | 2-4 sessions |
| **L** | 800+ lines    | 4+ sessions  |

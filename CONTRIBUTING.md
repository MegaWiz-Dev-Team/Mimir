# Contributing to Mimir

Mimir is part of the [Asgard AI Platform](https://github.com/MegaWiz-Dev-Team/Asgard). For the high-level workflow, CLA, and code of conduct, see [Asgard's CONTRIBUTING.md](https://github.com/MegaWiz-Dev-Team/Asgard/blob/main/CONTRIBUTING.md).

## This repo specifically

### Layout

- `ro-ai-bridge/` — Rust workspace (Axum API, knowledge engine, vault client)
- `ro-ai-dashboard/` — Next.js admin dashboard (TypeScript)
- `pipeline_data/` — Python ETL scripts (PubMed sync, KG extraction, embeddings)
- `scripts/` — operational tooling (vault-seed, benchmarks, deploys)
- `docs/` — ISO 29110 docs, sprint reports, architecture notes

### Development setup

```bash
# Rust API
cd ro-ai-bridge && cargo build && cargo test

# Dashboard
cd ro-ai-dashboard && npm install && npm run dev

# Python pipelines (set env vars from Asgard/.env first)
export HEIMDALL_API_KEY=...   # required by all pipeline scripts
python3 scripts/sync_pubmed_incremental.py
```

### Running tests

```bash
cd ro-ai-bridge && cargo test
cd ro-ai-dashboard && npm test
```

### Style

- Rust: `cargo fmt` + `cargo clippy --all-targets -- -D warnings`
- TypeScript: ESLint + Prettier (run `npm run lint`)
- Python: stdlib only where possible — pipelines avoid heavy frameworks

### Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org/): `feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, `test:`.

### Reporting issues

- 🐛 Bugs: open an issue with the bug report template
- 💡 Features: open an issue with the feature request template
- 🔒 Security: see [SECURITY.md](SECURITY.md) (do **not** open public issues)

### License & CLA

By contributing, you agree to license your contribution under [AGPL-3.0](LICENSE) and the [Asgard CLA](https://github.com/MegaWiz-Dev-Team/Asgard/blob/main/CLA.md). Your first PR serves as your electronic signature.

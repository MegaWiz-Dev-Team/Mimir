# mimir-fhir

**Status:** Phase 1 — Sprint 0 scaffolding (2026-05-24)
**License:** AGPL-3.0-only OR LicenseRef-Commercial
**Target:** FHIR R5 canonical type system for Asgard medical AI platform

Layer 1 of the 3-layer architecture defined in [ADR-012](../../Asgard/docs/decisions/ADR-012-fhir-native-data-plane-no-ehr-replacement.md). Provides:

- FHIR R5 type system for **20 resources** (Patient, Encounter, Observation, ..., Immunization, Specimen, ImagingStudy, Device — full list in [ADR-006 Amendment 1](../../Asgard/docs/decisions/ADR-006-fhir-canonical-design.md))
- R4↔R5 translator at adapter boundary ([ADR-013](../../Asgard/docs/decisions/ADR-013-fhir-r5-canonical-version.md))
- TH Core + MoPH-PC profile validators ([MOPH-PC1 mapping](../../Asgard/docs/architecture/moph_pc1_fhir_mapping.md))
- 43Files-to-FHIR adapter (Thai legacy hospital data → FHIR R5)
- FHIR REST endpoint at `/fhir/r5/{ResourceType}/...`
- Smart-on-FHIR launch surface

## Roadmap

See [Phase 1 implementation plan](../../Asgard/docs/technical/mimir-fhir-phase-1-plan.md) for sprint-by-sprint scope, and [step-by-step guide](../../Asgard/docs/technical/mimir-fhir-implementation-steps.md) for daily execution.

## Status (2026-05-24)

| Sprint | Status |
|---|---|
| Sprint 0 — Pre-flight & scaffold | **In progress** |
| Sprint 1 — Datatypes | Started early (Id datatype done as proof of TDD pattern) |
| Sprint 2-10 | Not started |

## Build

```bash
cargo build
cargo test
cargo clippy
```

## Directory structure

```
mimir-fhir/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs                       # module exports
│   ├── datatypes/                   # Sprint 1 (FHIR primitive + complex datatypes)
│   ├── resources/                   # Sprint 2-5 (20 R5 resources)
│   ├── profiles/                    # Sprint 7 (TH Core + MoPH-PC validators)
│   ├── translate/r4_to_r5/          # Sprint 2+ (adapter boundary translator)
│   ├── adapters/forty_three_files/  # Sprint 8 (Thai legacy MOPH dataset)
│   ├── validators/                  # Sprint 7
│   └── rest/                        # Sprint 6 (axum REST handlers)
├── profiles/th-core/                # Sprint 7 input (vendored TH Core profile JSON)
└── tests/
    ├── datatypes_id.rs              # Sprint 1 — first TDD example
    └── moph_pc1/fixtures/           # MOPH-PC1 78-element synthetic corpus
```

## License & Commercial Use

Dual-licensed: AGPL-3.0-only (open-source use) OR LicenseRef-Commercial (commercial use without AGPL viral). Contact paripol@megawiz.co for commercial licensing.

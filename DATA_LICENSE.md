# Data Licensing Notice — Medical Terminologies

This repository contains **code that references** standard medical
terminologies. It does **not** redistribute the licensed release/content of any
of them. The guiding rule:

> Code that *references* a terminology is fine to be public.
> The raw *release data* of a licensed terminology is **not** redistributable
> and must never be committed to this (public) repository.

The raw terminology files are ingested at deploy time from controlled storage
into the runtime database (Qdrant / MariaDB / Neo4j). Source control sees the
**loaders**, never the **content**. The relevant globs are excluded in
[`.gitignore`](.gitignore).

## Terminologies referenced and their licensing

| Terminology | License | Operator obligation |
|---|---|---|
| **SNOMED CT** | Affiliate License | In a member territory, in-country use is free but you must register as an affiliate and accept the license. No self-translation; no redistribution of release files. |
| **ICD-10-TM** (Thai Modification) | Thai MOPH / กระทรวงสาธารณสุข | Obtain from the relevant Thai authority; do not redistribute. |
| **TMT / TMLT** (Thai Medicines / Lab Terminology) | Thai Health Information Standards (THIS / สมสท.) | Obtain and license per Thai standards body terms. |
| **LOINC** | Regenstrief Institute | Free with attribution; comply with the LOINC license terms; do not republish release tables. |
| **ICD-10-CM / ICD-10-PCS / ICD-9-CM** (US, via CMS) | U.S. public domain | Free to use and redistribute. |
| **DrugBank** (incl. via PrimeKG) | Academic license; **commercial use requires a paid license** | Do **not** ship DrugBank-derived content to a commercial customer deployment without a DrugBank commercial license. |
| **PrimeKG** | MIT (graph schema) built from sources with their **own** licenses (e.g. DrugBank above) | Respect each upstream source license; the most restrictive source governs commercial redistribution. |
| **UMLS** | UMLS Metathesaurus License | Requires a UMLS license; do not redistribute. |

## SNOMED CT packages in use — provenance

All SNOMED CT packages are obtained via the SNOMED International **Member Licensing
& Distribution Service (MLDS)**, <https://mlds.ihtsdotools.org/>, under the Thailand
member account and the **IHTSDO Affiliate License 2023**. License PDFs ship alongside
the data at `$MIMIR_KB/SnomedCT/*.pdf`. Every package is subject to the SNOMED
**≤180-day upgrade** obligation.

| Package (MLDS) | Release | Ingested into | Loader |
|---|---|---|---|
| International Edition (RF2) | 20260501 | `snomed_descriptions`, `snomed_icd10_map` | `snomed_icd10_map_ingest.py` |
| GP/FP Refset | 20260101 | `snomed_refset_members` (`gpfp`) | `snomed_refset_ingest.py --gpfp` |
| International Patient Summary (IPS) RF2 | 20250701 | `snomed_refset_members` (`ips`) | `snomed_refset_ingest.py --ips` |
| EDQM Dose Forms → SNOMED Map | 20250701 | `snomed_edqm_dose_map` (+ derived `snomed_tmt_dose_link`) | `snomed_refset_ingest.py --edqm` |

These packages contain **no new licensable content beyond the Affiliate License** —
they are refsets/maps over the International Edition. The MedDRA and Orphanet maps
(also on MLDS) are **not** ingested: the SNOMED↔MedDRA map is free under the Affiliate
License, but *using MedDRA terms* requires a separate MedDRA subscription.

## Operator responsibility

Deployments of Asgard/Mimir are responsible for obtaining the appropriate
licenses for any terminology they ingest. Megawiz does not grant, sublicense, or
redistribute any of the above licensed content through this repository.

For commercial deployments, research-only or non-commercial sources (e.g.
DrugBank) must be gated at request-admission — see the platform's
`ai_models.metadata.commercial_use` enforcement.
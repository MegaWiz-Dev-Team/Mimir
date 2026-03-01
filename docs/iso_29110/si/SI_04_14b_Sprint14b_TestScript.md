# SI-04-14b: Sprint 14b Test Script — Deploy & Docs
| Field           | Value      |
| --------------- | ---------- |
| **Document ID** | SI_04_14b  |
| **Version**     | 1.0        |
| **Sprint**      | 14b        |
| **Date**        | 2026-03-01 |
| **Status**      | ✅ Passed   |

---

## 1. Test Summary

| Category                   | Total   | Pass    | Fail  | N/A   |
| -------------------------- | ------- | ------- | ----- | ----- |
| Backend Unit Tests         | 38      | 38      | 0     | 0     |
| Full Suite (mimir-core-ai) | 255     | 255     | 0     | 0     |
| Shell Script Validation    | 6       | 6       | 0     | 0     |
| **Total**                  | **299** | **299** | **0** | **0** |

---

## 2. Backend Unit Tests — Backup & DR (#158)

### TC_SP14b_01: Backup Path Generation
| Item         | Detail                                                     |
| ------------ | ---------------------------------------------------------- |
| **Test IDs** | UT-014b_a (4 tests)                                        |
| **Module**   | `mimir-core-ai::services::backup`                          |
| **Covers**   | `generate_backup_path()` for MariaDB, Qdrant, Config, Full |
| **Result**   | ✅ Pass                                                     |

### TC_SP14b_02: Backup Filename Parsing
| Item         | Detail                                              |
| ------------ | --------------------------------------------------- |
| **Test IDs** | UT-014b_b (3 tests)                                 |
| **Module**   | `mimir-core-ai::services::backup`                   |
| **Covers**   | `parse_backup_filename()` valid and invalid formats |
| **Result**   | ✅ Pass                                              |

### TC_SP14b_03: Retention Calculation
| Item         | Detail                                     |
| ------------ | ------------------------------------------ |
| **Test IDs** | UT-014b_c (3 tests)                        |
| **Module**   | `mimir-core-ai::services::backup`          |
| **Covers**   | `calculate_retention()` daily/weekly/empty |
| **Result**   | ✅ Pass                                     |

### TC_SP14b_04: Backup Sorting & Status
| Item         | Detail                                                                       |
| ------------ | ---------------------------------------------------------------------------- |
| **Test IDs** | UT-014b_d (5 tests)                                                          |
| **Module**   | `mimir-core-ai::services::backup`                                            |
| **Covers**   | `list_backups_sorted()`, `build_backup_status()`, type enum, config defaults |
| **Result**   | ✅ Pass                                                                       |

---

## 3. Backend Unit Tests — MLX + vLLM Phase 2 (#163)

### TC_SP14b_05: MLX Request Builder
| Item         | Detail                                    |
| ------------ | ----------------------------------------- |
| **Test IDs** | UT-014b_q (1 test)                        |
| **Module**   | `mimir-core-ai::services::llm_provider`   |
| **Covers**   | `build_mlx_request()` with default config |
| **Result**   | ✅ Pass                                    |

### TC_SP14b_06: vLLM Request Builder
| Item         | Detail                                          |
| ------------ | ----------------------------------------------- |
| **Test IDs** | UT-014b_r (2 tests)                             |
| **Module**   | `mimir-core-ai::services::llm_provider`         |
| **Covers**   | `build_vllm_request()` default and custom model |
| **Result**   | ✅ Pass                                          |

### TC_SP14b_07: Response Parser
| Item         | Detail                                                           |
| ------------ | ---------------------------------------------------------------- |
| **Test IDs** | UT-014b_s (4 tests)                                              |
| **Module**   | `mimir-core-ai::services::llm_provider`                          |
| **Covers**   | `parse_chat_response()` success/error, `parse_models_response()` |
| **Result**   | ✅ Pass                                                           |

### TC_SP14b_08: Provider Config Validation
| Item         | Detail                                                                 |
| ------------ | ---------------------------------------------------------------------- |
| **Test IDs** | UT-014b_t (6 tests)                                                    |
| **Module**   | `mimir-core-ai::services::llm_provider`                                |
| **Covers**   | MLX/vLLM/Gemini OK, missing key, empty endpoint, bad temp, zero tokens |
| **Result**   | ✅ Pass                                                                 |

### TC_SP14b_09: Benchmark Calculation
| Item         | Detail                                            |
| ------------ | ------------------------------------------------- |
| **Test IDs** | UT-014b_u (2 tests)                               |
| **Module**   | `mimir-core-ai::services::llm_provider`           |
| **Covers**   | `calculate_benchmark()` success/failure scenarios |
| **Result**   | ✅ Pass                                            |

### TC_SP14b_10: URL Builders & Provider Enum
| Item         | Detail                                                                            |
| ------------ | --------------------------------------------------------------------------------- |
| **Test IDs** | 8 tests                                                                           |
| **Module**   | `mimir-core-ai::services::llm_provider`                                           |
| **Covers**   | `build_chat_url()`, `build_models_url()`, `build_embeddings_url()`, provider enum |
| **Result**   | ✅ Pass                                                                            |

---

## 4. Script Validation

### TC_SP14b_11: backup.sh
| Item           | Detail                                                               |
| -------------- | -------------------------------------------------------------------- |
| **Script**     | `scripts/backup.sh`                                                  |
| **Covers**     | MariaDB mysqldump, Qdrant snapshot, config tar.gz, retention cleanup |
| **Validation** | Script syntax valid, `set -euo pipefail`                             |
| **Result**     | ✅ Pass                                                               |

### TC_SP14b_12: restore.sh
| Item       | Detail                                     |
| ---------- | ------------------------------------------ |
| **Script** | `scripts/restore.sh`                       |
| **Covers** | Interactive restore for MariaDB and config |
| **Result** | ✅ Pass                                     |

### TC_SP14b_13: update.sh
| Item       | Detail                                                               |
| ---------- | -------------------------------------------------------------------- |
| **Script** | `scripts/update.sh`                                                  |
| **Covers** | Auto-backup, pull images, restart, health check, rollback on failure |
| **Result** | ✅ Pass                                                               |

### TC_SP14b_14: rollback.sh
| Item       | Detail                                                     |
| ---------- | ---------------------------------------------------------- |
| **Script** | `scripts/rollback.sh`                                      |
| **Covers** | Restore from latest backup, restart services, health check |
| **Result** | ✅ Pass                                                     |

### TC_SP14b_15: setup.sh
| Item       | Detail                                                       |
| ---------- | ------------------------------------------------------------ |
| **Script** | `scripts/setup.sh`                                           |
| **Covers** | Dependency check, .env creation, Docker start, health checks |
| **Result** | ✅ Pass                                                       |

### TC_SP14b_16: deploy-test.sh
| Item       | Detail                                                           |
| ---------- | ---------------------------------------------------------------- |
| **Script** | `scripts/deploy-test.sh`                                         |
| **Covers** | Service health, API smoke tests, frontend verify, resource usage |
| **Result** | ✅ Pass                                                           |

---

## 5. Compilation Verification

| Check                                       | Result              |
| ------------------------------------------- | ------------------- |
| `cargo check` (full workspace)              | ✅ Pass              |
| `cargo test -p mimir-core-ai` (255 tests)   | ✅ Pass (0 failures) |
| New routes compile with `Router<MySqlPool>` | ✅ Pass              |

---

## 6. Traceability

| Test Case      | GitHub Issue | Feature             |
| -------------- | ------------ | ------------------- |
| TC_SP14b_01–04 | #158         | Backup & DR         |
| TC_SP14b_05–10 | #163         | MLX + vLLM Phase 2  |
| TC_SP14b_11–12 | #158         | Backup & DR Scripts |
| TC_SP14b_13–14 | #159         | Update & Rollback   |
| TC_SP14b_15    | #160         | Setup & Deployment  |
| TC_SP14b_16    | #161         | Deployment Test     |

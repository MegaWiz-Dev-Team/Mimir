# Huginn Security Scan Report — Sprint 31
**Date:** 2026-03-18  
**Tool:** cargo-audit v0.22.1  
**Target:** Mimir/ro-ai-bridge v0.29.0

## Summary

| Severity | Count | Status |
|----------|-------|--------|
| 🔴 High | 1 | **Fixed** (quinn-proto → ≥0.11.14) |
| 🟡 Medium | 1 | No fix available (rsa via sqlx-mysql) |
| ⚠️ Warnings | 4 | Unmaintained crates (neo4rs deps) |

## Vulnerabilities

### RUSTSEC-2026-0037 — quinn-proto (HIGH, 8.7)
- **Title:** Denial of service in Quinn endpoints
- **Affected:** quinn-proto 0.11.13 → via reqwest
- **Fix:** Upgrade to ≥0.11.14
- **Status:** ✅ Fixed via `cargo update -p quinn-proto`

### RUSTSEC-2023-0071 — rsa (MEDIUM, 5.9)
- **Title:** Marvin Attack: potential key recovery through timing sidechannels
- **Affected:** rsa 0.9.10 → via sqlx-mysql
- **Fix:** No fixed upgrade available
- **Status:** ⏳ Accepted risk (transitive dependency from sqlx)

## Warnings (Unmaintained)

| Crate | Advisory | Source |
|-------|----------|--------|
| backoff 0.4.0 | RUSTSEC-2025-0012 | neo4rs |
| instant 0.1.13 | RUSTSEC-2024-0384 | neo4rs → backoff |
| paste 1.0.15 | RUSTSEC-2024-0436 | neo4rs |
| rustls-pemfile 2.2.0 | RUSTSEC-2025-0134 | neo4rs |

**Note:** All 4 warnings are transitive dependencies of `neo4rs 0.8.0`. Will be resolved when neo4rs releases a new version.

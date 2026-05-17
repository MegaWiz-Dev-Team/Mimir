# 03_15 Backend SSO — Yggdrasil JWT Integration

**Status:** Design draft (2026-05-17) — implementation pending `feat/yggdrasil-jwt-auth` merge
**Owner:** Backend SSO session (Opus 4.7)
**Depends on:** Heimdall `feat/yggdrasil-jwt-auth` merged to main (after S1 May 18 integration test)
**Related memory:** `asgard_jwt_auth_pattern.md`
**Reference impl:** `Heimdall/gateway/src/auth_jwt.rs` (Heimdall 0.6.0, 13/13 tests)

---

## 1. Context

Heimdall gateway adopted Yggdrasil-issued RS256 JWTs as a second auth mode (alongside legacy static `API_KEYS`) in Sprint 52. S1 voted (2026-05-17) that Mimir backend should follow the same pattern so a single `TenantContext` extractor works across services. Heimdall is the reference implementation.

This plan describes how to extend Heimdall's pattern into Mimir's auth/iam/sso modules **without breaking** the existing HS256 login-token flow or the existing Zitadel OIDC code exchange.

## 2. Current state survey

### 2.1 Three pre-existing auth concerns inside Mimir backend

| # | Concern | Code | Status |
|---|---------|------|--------|
| A | Username/password login → HS256 token | `mimir-core-ai/src/services/iam.rs::login()` + `routes/auth.rs::login` | Production, in use by Mimir UI |
| B | OIDC authorization-code exchange (Zitadel → internal HS256 via JIT) | `mimir-core-ai/src/services/sso.rs::exchange_code()` + `routes/auth.rs::sso_exchange` | Production, in use by `/login/callback` |
| C | Header-trust tenant context | `mimir-core-ai/src/middleware/tenant.rs::tenant_auth_middleware` | Weak — trusts `X-Tenant-Id` header WITHOUT verification |

### 2.2 What `tenant_auth_middleware` does today (CRITICAL FINDING)

```rust
// mimir-core-ai/src/middleware/tenant.rs (current)
let tenant_id = req.headers()
    .get("x-tenant-id")
    .and_then(|v| v.to_str().ok())
    .unwrap_or("default_tenant")
    .to_string();
req.extensions_mut().insert(TenantContext {
    user_id: format!("{}_admin", tenant_id),
    tenant_id,
    role: "admin".to_string(),
});
```

**This is auth bypass.** Any caller can claim any tenant by setting `X-Tenant-Id`. The current `iam_routes()` applies this middleware to user/tenant management endpoints (`/api/v1/iam/users`, `/api/v1/iam/tenants`, etc.). This needs to be replaced — not just augmented — by the JWT-aware middleware.

### 2.3 Existing dependencies

- `jsonwebtoken = "10.3.0"` with `rust_crypto` feature (workspace-managed) ✓ already present
- `moka` — **NOT present**, need to add for JWKS cache
- `wiremock` — **NOT present**, need to add as dev-dependency for tests
- `axum 0.8.8`, `reqwest 0.12` — both present

### 2.4 Version mismatch with Heimdall

| Crate | Mimir | Heimdall | Adaptation |
|-------|-------|----------|------------|
| `jsonwebtoken` | `10.3.0` | `9` | API changed between v9 and v10 — `decode_header`, `jwk::JwkSet`, `Validation` API moved. Cherry-pick from Heimdall requires porting to v10 API. |
| `moka` | absent | `0.12` (future feature) | Add to Mimir workspace |
| `wiremock` | absent | `0.6` | Add as dev-dependency |

## 3. Target state

### 3.1 Three coexisting auth modes (post-implementation)

```
                       ┌─────────────────────────┐
                       │  Authorization header   │
                       │  Bearer <token>          │
                       └────────────┬────────────┘
                                    │
                ┌───────────────────┼───────────────────┐
                │                   │                   │
        starts "ey"          fixed-string match      anything else
                │                   │                   │
                ▼                   ▼                   ▼
        Yggdrasil RS256        HS256 internal       reject 401
        (JWKS + aud="mimir")   (from /login or
                                /sso-exchange)
                │                   │
                └───────────┬───────┘
                            ▼
                  TenantContext extension
                  inserted into request
                            ▼
                     Handler runs
```

### 3.2 Resulting `TenantContext` is the same shape regardless of source

```rust
pub struct TenantContext {
    pub user_id: String,
    pub tenant_id: String,
    pub role: String,
    pub source: AuthSource,   // NEW: "jwt" | "legacy_hs256" | "unauthenticated"
}
```

Adding `source` lets handlers (or downstream audit code) distinguish without breaking existing `Extension<TenantContext>` consumers.

## 4. Inject point map

### 4.1 What changes in `mimir-core-ai/`

| File | Change |
|------|--------|
| `src/middleware/auth_jwt.rs` (NEW) | Cherry-pick from Heimdall, port to `jsonwebtoken 10.3` API. Holds `JwtValidator` + `Claims` struct. ~430 lines. |
| `src/middleware/auth.rs` (NEW) | Dual-mode wrapper analogous to `Heimdall/gateway/src/auth.rs`. Routes by `ey`-prefix. Inserts both `Claims` and `TenantContext` into extensions. ~230 lines. |
| `src/middleware/tenant.rs` (REPLACE) | Reduce to thin extractor: `Extension<TenantContext>` reads what `auth.rs` already inserted. **Stop trusting `X-Tenant-Id` header.** |
| `src/middleware/mod.rs` | Re-export the new modules. |
| `src/config.rs` | Add `yggdrasil_issuer: Option<String>` and `jwt_audience: Option<String>` fields. `JWT_AUDIENCE` defaults to `"mimir"` if `YGGDRASIL_ISSUER` is set. |
| `Cargo.toml` (workspace) | Add `moka = { version = "0.12", features = ["future"] }`, `wiremock = "0.6"` (dev). |

### 4.2 What changes in `ro-ai-bridge/src/main.rs`

```rust
// BEFORE (current)
let app = Router::new()
    .nest("/api/v1/iam", iam_routes())          // self-applies weak middleware
    .nest("/api/v1/auth", auth_routes())        // public (login/sso-exchange)
    .nest("/api/v1/pipeline", pipeline_routes()) // no auth
    // ...

// AFTER (target)
let jwt_validator = config.yggdrasil_issuer
    .as_ref()
    .map(|iss| JwtValidator::new(iss.clone(), config.jwt_audience.clone().unwrap_or_else(|| "mimir".into())));

let auth_state = AuthState {
    jwt_validator: jwt_validator.map(Arc::new),
    legacy_hs256_secret: config.jwt_secret.clone(),
};

let public_routes = Router::new()
    .nest("/auth", auth_routes())               // public, no middleware
    .with_state(pool.clone());

let authed_routes = Router::new()
    .nest("/iam", iam_routes())
    .nest("/pipeline", pipeline_routes())
    .nest("/qc", qc_routes())
    // ... all other routes that need auth
    .layer(axum::middleware::from_fn_with_state(auth_state.clone(), auth_middleware));

let app = Router::new()
    .nest("/api/v1", public_routes.merge(authed_routes))
    .nest("/api/v1/app-settings", app_settings_routes()) // decide per-route
    // ...
```

### 4.3 What does NOT change

- `routes/auth.rs` (login, sso-config, sso-exchange) — works as-is, public.
- `services/sso.rs` (Zitadel OIDC code exchange) — works as-is.
- `services/iam.rs::login()` (HS256 mint) — works as-is for backwards compatibility.
- `routes/iam.rs` handler bodies — unchanged; the middleware now provides verified `TenantContext`.

## 5. Adaptation from Heimdall reference

### 5.1 `jsonwebtoken 9 → 10` API port checklist

- `decode_header()` — signature unchanged ✓
- `jwk::JwkSet` — moved namespace? verify before commit
- `Validation::new(Algorithm::RS256)`, `.set_issuer()`, `.set_audience()` — verify v10 API
- `DecodingKey::from_jwk(jwk)` — verify v10 returns same error type
- Test sign-side: `EncodingKey::from_rsa_pem`, `encode(&header, &claims, &key)` — verify

If v10 breaks the cherry-pick non-trivially, fallback options:
1. Pin Mimir to `jsonwebtoken 9` (downgrade workspace dep) — needs `feature/insurance-s1-ingestion` review since refgraph-cli may use newer features
2. Maintain both — Heimdall on 9, Mimir on 10. Code is duplicated but isolated.

### 5.2 Heimdall `AppState` pattern → Mimir adaptation

Heimdall:
```rust
struct AppState { jwt_validator: Option<JwtValidator>, config: Arc<AppConfig>, ... }
auth_middleware uses State(state)
```

Mimir doesn't have a unified `AppState`. Options:
1. **Introduce `AuthState`** for the middleware specifically (cleanest, scoped)
2. **Use `Extension<Arc<AuthState>>`** — fits Mimir's current `Extension<Arc<Config>>` pattern
3. **Move toward unified AppState** (larger refactor — defer)

Recommend **Option 2** for minimal blast radius.

## 6. Implementation steps

### Phase 0 — Pre-flight (do these before any code)
- [ ] `feat/yggdrasil-jwt-auth` merged to `Mimir/main` (or to `feat/curator-review-ui` first)
- [ ] Rebase `feat/curator-review-ui` on latest main (pulls in test-fixture-vlm-ports fix at `38ff98b`)
- [ ] Local cargo workspace builds clean: `cargo build && cargo test` on rebased main

### Phase 1 — Port middleware (~3-4h)
- [ ] Add `moka 0.12` (future feature) + `wiremock 0.6` (dev) to workspace `Cargo.toml`
- [ ] Cherry-pick `Heimdall/gateway/src/auth_jwt.rs` → `Mimir/mimir-core-ai/src/middleware/auth_jwt.rs`
- [ ] Port `jsonwebtoken 9 → 10` API differences (see §5.1)
- [ ] Cherry-pick `Heimdall/gateway/src/auth.rs` → `Mimir/mimir-core-ai/src/middleware/auth.rs`
- [ ] Adapt `auth.rs` to use `Extension<Arc<AuthState>>` instead of `State<AppState>`
- [ ] Adapt `TenantContext` insertion: when JWT validates, build `TenantContext` from claims AND insert both `Claims` + `TenantContext` extensions

### Phase 2 — Replace weak tenant middleware (~1h)
- [ ] Reduce `mimir-core-ai/src/middleware/tenant.rs` to a typed extractor only — no header-trust fallback
- [ ] Verify no other caller in workspace depends on the old behavior (`grep tenant_auth_middleware`)
- [ ] Update `iam_routes()` and any other route using `tenant_auth_middleware` to use the new `auth_middleware` instead

### Phase 3 — Wire into main (~1h)
- [ ] Split `main.rs` router into public (no auth) + authed (with new middleware) per §4.2
- [ ] Read `YGGDRASIL_ISSUER` + `JWT_AUDIENCE` from env
- [ ] When unset, only HS256 legacy mode is active (opt-in pattern same as Heimdall)

### Phase 4 — Tests (~2h)
- [ ] Port Heimdall's 8 JWT tests verbatim to `auth_jwt.rs` (adapt for jsonwebtoken v10 if API moved)
- [ ] Add 5 dual-mode routing tests in `auth.rs`:
  - HS256-format token routes to legacy validator
  - `ey`-prefix routes to JWT validator
  - Missing bearer → 401
  - `/api/v1/auth/login` skips middleware (public)
  - `/api/v1/iam/users` requires valid auth
- [ ] Add 2 integration tests confirming `Extension<TenantContext>` is populated from JWT claims

### Phase 5 — Audit + metrics (~30m)
- [ ] Verify `tracing::info!(auth_mode = "jwt", sub, tenant, scope, "auth.success")` matches Tyr format
- [ ] Verify `metrics::counter!("auth_success_total", "mode" => "...")` labels match Heimdall

### Phase 6 — Rollout (~30m)
- [ ] Deploy Mimir image without `YGGDRASIL_ISSUER` set → no behavior change (validation: existing HS256 login still works)
- [ ] In `asgard-secrets`, add `YGGDRASIL_ISSUER`, set `JWT_AUDIENCE=mimir`
- [ ] Roll Mimir pod → JWT mode activates
- [ ] Test inbound JWT validation via `curl` with a Yggdrasil-issued token (mint via `Yggdrasil/docs/heimdall-key-gen.md` Option 1)

## 7. Frontend & client impact

| Client | Impact |
|--------|--------|
| Mimir UI `/login` flow | None — still gets HS256 via `/api/v1/auth/login` |
| Mimir UI Zitadel SSO `/login/callback` | None — still goes through `/sso-exchange`, gets HS256 |
| Mimir UI subsequent API calls | None — still bears the HS256 token |
| Service-to-service callers | **New option:** send Yggdrasil-issued JWT directly with `aud=mimir`, skip HS256 mint |
| `refgraph-cli` (S1) | None unless S1 chooses to switch from HS256 → Yggdrasil JWT (per their Day 12 plan, May 27+) |

## 8. Test strategy

```
Coverage matrix
                                 │ Pass token │ Pass JWT │ No token
─────────────────────────────────┼────────────┼──────────┼──────────
/api/v1/auth/login               │ n/a        │ n/a      │ ✓ 200
/api/v1/auth/sso-exchange        │ n/a        │ n/a      │ ✓ 200
/api/v1/iam/users (HS256 valid)  │ ✓ 200      │ —        │ —
/api/v1/iam/users (HS256 wrong)  │ ✓ 401      │ —        │ —
/api/v1/iam/users (JWT valid)    │ —          │ ✓ 200    │ —
/api/v1/iam/users (JWT expired)  │ —          │ ✓ 401    │ —
/api/v1/iam/users (no header)    │ —          │ —        │ ✓ 401
/api/v1/iam/users (X-Tenant-Id   │            │          │
   only, no bearer)              │ —          │ —        │ ✓ 401 ←── BREAKING CHANGE
```

The last row is the deliberate breaking change vs current weak middleware. Anyone using `X-Tenant-Id` without a bearer will now get 401. **This needs explicit announcement** before rollout — see open question Q3.

## 9. Open questions (need answer before code)

| # | Question | Likely owner |
|---|----------|--------------|
| Q1 | Is anyone in production calling `/api/v1/iam/*` with bare `X-Tenant-Id` header (no bearer)? Need grep frontend + service-to-service callers. | Product + S1 |
| Q2 | Should the new middleware apply to `/api/v1/pipeline/*`, `/api/v1/qc/*`, `/api/v1/stats/*`, `/api/v1/app-settings/*` too? Or only `/api/v1/iam/*`? | Architecture |
| Q3 | Frontend migration timing — do we deprecate HS256 issuance and force OIDC code exchange only? Or keep HS256 indefinitely as power-user fallback? | Product |
| Q4 | jsonwebtoken 9 → 10 port — non-trivial or just renames? | This session, after pre-flight |
| Q5 | Should `tenant_auth_middleware` callers (`icd10.rs`, `rag_benchmark.rs`, `bin/monitor.rs`) be migrated in the same PR, or follow-up? | This session, after grep |

## 10. Rollout safety

Same pattern as Heimdall — **opt-in by env var**:

- `YGGDRASIL_ISSUER` not set → no JWT validation, all existing behavior preserved
- `YGGDRASIL_ISSUER` set + `JWT_AUDIENCE=mimir` → JWT mode active alongside legacy HS256

Rollback = `kubectl set env deployment/mimir-api -n asgard YGGDRASIL_ISSUER-` (remove env var), pod restart. Same speed as the rotation rollback documented in INC-2026-05-17-001.

## 11. References

- Pattern memory: [`memory/asgard_jwt_auth_pattern.md`](../../../memory/asgard_jwt_auth_pattern.md)
- Heimdall reference impl: [`Heimdall/gateway/src/auth_jwt.rs`](../../../../Heimdall/gateway/src/auth_jwt.rs)
- Heimdall dual-mode wrapper: [`Heimdall/gateway/src/auth.rs`](../../../../Heimdall/gateway/src/auth.rs)
- Token mint guide: [`Yggdrasil/docs/heimdall-key-gen.md`](../../../../Yggdrasil/docs/heimdall-key-gen.md)
- Components roster (audience naming): [`memory/asgard_components_roles.md`](../../../memory/asgard_components_roles.md)
- INC-2026-05-17 incident docs: [`Asgard/docs/incidents/2026-05-17-mimir-503/`](../../../../Asgard/docs/incidents/2026-05-17-mimir-503/)
- Branch dependency: `feat/yggdrasil-jwt-auth` (Heimdall) → merge target Mimir/main
- Sprint 52 ticket (Vault + ESO migration parent context): [Asgard issue #65](https://github.com/MegaWiz-Dev-Team/2026-05-17-mimir-503/issues/65)

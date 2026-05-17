# 03_16 Backend SSO v1.4.0 — Deploy Runbook

**For:** ops / on-call shipping the Yggdrasil JWT integration to production
**Companion to:** `03_15_Backend_SSO_Yggdrasil_JWT_Integration.md` (the design + Phase plan)
**Release:** ro-ai-bridge `v1.4.0` + mimir-core-ai `0.2.0` + heimdall-gateway `0.6.0`

---

## 0. Heads-up — this release contains a deliberate breaking change

After deploy, `/api/v1/iam/*`, `/api/v1/rag-benchmark/*`, `/api/v1/icd10/*`, `/api/v1/training/*`, and the monitor binary's protected routes **require a `Authorization: Bearer <token>` header**. Bare `X-Tenant-Id` header is no longer trusted.

| Caller | Impact |
|--------|--------|
| Mimir UI (login flow) | ✅ unaffected — already sends Bearer post-login |
| Service-to-service tooling using `X-Tenant-Id` only | ❌ **401 after deploy** — must migrate to Bearer |
| `refgraph-cli` and other S1 tooling using `/api/v1/search` | ✅ unaffected — not in scope of this release |

Before deploy, grep all known callers and confirm they send a Bearer header.

```bash
# Helpful pre-flight
grep -rn "X-Tenant-Id" /Users/mimir/Developer --include="*.{py,ts,js,rs,sh}" \
  | grep -v "Bearer" | grep -i "/api/v1/iam\|/api/v1/rag-benchmark\|/api/v1/icd10\|/api/v1/training"
```

---

## 1. Prerequisites

### 1.1 PR merge order

| # | PR | Why | Merge first |
|---|----|-----|-------------|
| 1 | [Mimir #294](https://github.com/MegaWiz-Dev-Team/Mimir/pull/294) — JWT validator + AuthState + iam.rs | provides `JwtValidator`, `dual_mode_auth_middleware`, `AuthState` | ✅ |
| 2 | [Mimir #295](https://github.com/MegaWiz-Dev-Team/Mimir/pull/295) — rag_benchmark + icd10 + training | depends on #294 (uses dual_mode_auth_middleware) | after #294 |
| 3 | [Mimir #297](https://github.com/MegaWiz-Dev-Team/Mimir/pull/297) — bin/monitor.rs | depends on #294 (uses AuthState) | after #295 |
| 4 | [Heimdall #10](https://github.com/MegaWiz-Dev-Team/Heimdall/pull/10) — gateway 0.6.0 + Bearer fix | independent of Mimir PRs | anytime |

Each Mimir PR is `MERGEABLE` + `CLEAN`. Heimdall #10 same.

### 1.2 Environment to set in cluster (asgard-secrets) — for JWT mode

Optional — only if you want to **activate** Yggdrasil RS256 mode at the same time as deploy. Without these env vars, the new image runs in legacy HS256 mode and behaves like the old image (with breaking change applied).

```yaml
# kubectl edit secret asgard-secrets -n asgard
data:
  YGGDRASIL_ISSUER: <base64 of https://yggdrasil.asgard.internal>
  JWT_AUDIENCE: bWltaXI=  # base64 of "mimir"
```

If you want to keep JWT mode off for the first deploy and enable later, leave these unset.

---

## 2. Release tagging (after all 4 PRs merge)

```bash
# Mimir
cd /Users/mimir/Developer/Mimir
git checkout main
git pull
git tag v1.4.0
git push origin v1.4.0

# Heimdall
cd /Users/mimir/Developer/Heimdall
git checkout main
git pull
git tag v0.6.0
git push origin v0.6.0
```

Per memory `semver_release_process.md`: tags are explicit `vX.Y.Z`, image tags follow the same convention. `:latest` is updated **last**, after the explicit tag is verified in production.

---

## 3. Image build + push

### 3.1 Mimir API

```bash
cd /Users/mimir/Developer/Mimir
git checkout v1.4.0   # the tag we just pushed

# Build with explicit version tag — DO NOT touch :latest yet
docker build \
  -t ghcr.io/megawiz-dev-team/mimir-api:v1.4.0 \
  -f ro-ai-bridge/Dockerfile .

# Push to ghcr (auth must be live — `docker login ghcr.io` if not)
docker push ghcr.io/megawiz-dev-team/mimir-api:v1.4.0

# Verify the new image is in registry
docker manifest inspect ghcr.io/megawiz-dev-team/mimir-api:v1.4.0 | head -5
```

Expected build time on arm64 OrbStack: 5-8 min from clean. ~1 min if Cargo build cache is warm.

### 3.2 Heimdall gateway

```bash
cd /Users/mimir/Developer/Heimdall
git checkout v0.6.0
docker build -t ghcr.io/megawiz-dev-team/heimdall-gateway:v0.6.0 .
docker push ghcr.io/megawiz-dev-team/heimdall-gateway:v0.6.0
```

Heimdall isn't deployed in K8s (runs as native launchd per memory `asgard_heimdall_deployment.md`) — image is for reference / cross-platform users.

---

## 4. Lab deploy + smoke test

### 4.1 Pin the deployment to the explicit version tag

```bash
# Mimir API in asgard namespace
kubectl set image deploy/mimir-api -n asgard \
  mimir-api=ghcr.io/megawiz-dev-team/mimir-api:v1.4.0

kubectl rollout status deploy/mimir-api -n asgard --timeout=180s

# Verify image ID matches (defensive — local cache vs registry mismatch was the
# Sprint 51e gotcha; see asgard incident 2026-05-17 postmortem)
POD=$(kubectl get pod -n asgard -l app=mimir-api -o jsonpath='{.items[0].metadata.name}')
kubectl describe pod -n asgard "$POD" | grep -A 1 "Image:" | head -3
# expect: ghcr.io/megawiz-dev-team/mimir-api:v1.4.0 + matching digest
```

### 4.2 Smoke test

```bash
# 1. Health check still works (no auth required)
kubectl exec -n asgard deploy/mimir-api -- curl -fsS http://localhost:8080/healthz

# 2. /api/v1/iam/users without Bearer → expect 401 (was 200 before — auth bypass closed)
TOKEN=""
kubectl exec -n asgard deploy/mimir-api -- \
  curl -i -s -o /dev/null -w "%{http_code}\n" \
  http://localhost:8080/api/v1/iam/users
# expect: 401

# 3. /api/v1/iam/users with valid HS256 token → expect 200
#    Get a token from /api/v1/auth/login first; this assumes "admin" / "admin" works
TOKEN=$(kubectl exec -n asgard deploy/mimir-api -- \
  curl -fsS -X POST http://localhost:8080/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"admin"}' | jq -r '.token')

kubectl exec -n asgard deploy/mimir-api -- \
  curl -i -s -o /dev/null -w "%{http_code}\n" \
  -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/iam/users
# expect: 200

# 4. Boot log shows JWT mode active/inactive
kubectl logs -n asgard deploy/mimir-api --tail=200 | grep -E "yggdrasil_jwt|insecure_jwt_secret"
# expect: "yggdrasil_jwt_disabled" if YGGDRASIL_ISSUER unset, else "yggdrasil_jwt_enabled"
# also expect NO "insecure_jwt_secret_default" if JWT_SECRET is set in asgard-secrets
```

If all 4 smoke tests pass → proceed to production. If any fails → **rollback (§7)**.

---

## 5. Production deploy

If lab is on the same cluster as production (current setup per memory `asgard_orbstack_k8s.md`), there's no separate cluster step — the rollout in §4.1 already deployed. Skip this section.

If you have a separate prod cluster, repeat §4.1 against it.

---

## 6. Enable JWT mode (optional, post-deploy)

By default, only legacy HS256 path is active and behaves like the old image plus the breaking change (no `X-Tenant-Id` trust). To **activate** Yggdrasil RS256 mode:

```bash
# Set env vars
kubectl set env deploy/mimir-api -n asgard \
  YGGDRASIL_ISSUER=https://yggdrasil.asgard.internal \
  JWT_AUDIENCE=mimir

# Pod restart will pick up new env
kubectl rollout status deploy/mimir-api -n asgard

# Verify boot log
kubectl logs -n asgard deploy/mimir-api --tail=50 | grep yggdrasil_jwt_enabled
# expect: "Yggdrasil JWT validation active for /api/v1/iam/*"
```

Now test with a Yggdrasil-minted RS256 token (per `Yggdrasil/docs/heimdall-key-gen.md` Option 1 — Machine User PAT):

```bash
YGGDRASIL_TOKEN=$(...)  # from heimdall-key-gen.md flow
kubectl exec -n asgard deploy/mimir-api -- \
  curl -i -H "Authorization: Bearer $YGGDRASIL_TOKEN" \
  http://localhost:8080/api/v1/iam/users
# expect: 200
```

---

## 7. Rollback

Rollback is fast — point the deployment back to the previous explicit version tag.

```bash
# Find the previous version
kubectl rollout history deploy/mimir-api -n asgard

# Roll back to immediate previous revision
kubectl rollout undo deploy/mimir-api -n asgard

# OR set image explicitly back to v1.3.0 (whichever was prior)
kubectl set image deploy/mimir-api -n asgard \
  mimir-api=ghcr.io/megawiz-dev-team/mimir-api:v1.3.0

kubectl rollout status deploy/mimir-api -n asgard
```

If `YGGDRASIL_ISSUER` was set in §6 and you want to fully revert auth posture:
```bash
kubectl set env deploy/mimir-api -n asgard YGGDRASIL_ISSUER-
```

---

## 8. Update `:latest` (last, only after a few days stable)

Per `semver_release_process.md`, never deploy `:latest` directly. After v1.4.0 has run stable for 3-7 days in production:

```bash
docker pull ghcr.io/megawiz-dev-team/mimir-api:v1.4.0
docker tag ghcr.io/megawiz-dev-team/mimir-api:v1.4.0 ghcr.io/megawiz-dev-team/mimir-api:latest
docker push ghcr.io/megawiz-dev-team/mimir-api:latest
```

---

## 9. Post-deploy cleanup PR — remove `tenant_auth_middleware`

After **all 3 Mimir PRs** (#294, #295, #297) are merged to main, the legacy `tenant_auth_middleware` function has **zero remaining callers**. Open a small cleanup PR:

```bash
cd /Users/mimir/Developer/Mimir
git checkout main && git pull
git checkout -b chore/remove-tenant-auth-middleware

# Verify zero callers remain (should print only the definition + a stale comment)
grep -rn "tenant_auth_middleware" --include="*.rs" ro-ai-bridge/
# expect:
#   ro-ai-bridge/mimir-core-ai/src/middleware/tenant.rs:29:pub async fn tenant_auth_middleware(
#   ro-ai-bridge/mimir-core-ai/src/middleware/request_id.rs:22:    // Extract tenant_id ... (set by tenant_auth_middleware)
```

Edit `ro-ai-bridge/mimir-core-ai/src/middleware/tenant.rs`:
- Delete the `tenant_auth_middleware` function and its imports (`Extension`, `Next`, `Request`, `Response`, `StatusCode`, `crate::config::Config`, `std::sync::Arc`)
- Keep `TenantClaims` and `TenantContext` structs — still used by handlers and the new middleware

Edit `ro-ai-bridge/mimir-core-ai/src/middleware/request_id.rs`:
- Update line 22 comment to refer to `dual_mode_auth_middleware` instead of `tenant_auth_middleware`

```bash
cargo check -p ro-ai-bridge  # confirm no broken references
cargo test -p mimir-core-ai --test iam_jwt_validator --test dual_mode_auth_iam  # 21/21 still pass

git add ro-ai-bridge/mimir-core-ai/src/middleware/tenant.rs \
        ro-ai-bridge/mimir-core-ai/src/middleware/request_id.rs

git commit -m "chore(auth): remove tenant_auth_middleware (zero callers after #294, #295, #297)

After PRs #294, #295, and #297 merged, the old weak tenant_auth_middleware
has zero remaining callers — every route that previously used it now uses
the verified dual_mode_auth_middleware that requires a Bearer token.

This commit deletes the function and updates a stale comment.
TenantClaims and TenantContext structs are preserved (still used by
handlers and the new middleware).

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"

git push -u origin chore/remove-tenant-auth-middleware

gh pr create --base main --title "chore(auth): remove tenant_auth_middleware (zero callers)" \
  --body "Cleanup PR for Backend SSO Phase 2 migration. After #294/#295/#297 merged,
this function has zero callers; safe to delete. Companion to v1.4.0 release.

cargo check + tests pass."
```

---

## 10. Audit checklist for the deploy

| ✓ | Item |
|---|------|
| ☐ | All 4 PRs merged (3 Mimir + Heimdall #10) |
| ☐ | Tags `v1.4.0` (Mimir) + `v0.6.0` (Heimdall) pushed |
| ☐ | Images built + pushed with explicit version tag |
| ☐ | `kubectl set image` to explicit tag (not :latest) |
| ☐ | Smoke tests in §4.2 all pass |
| ☐ | Boot log shows expected `yggdrasil_jwt_{enabled,disabled}` and NO `insecure_jwt_secret_default` |
| ☐ | (If JWT mode enabled) Yggdrasil token smoke test passes |
| ☐ | 24h post-deploy: monitor `auth_failure_total` Prometheus counter for unexpected spikes |
| ☐ | 3-7 days stable: update `:latest` tag |
| ☐ | Post-merge: open the cleanup PR per §9 |

---

## References

- Design + Phase plan: [`03_15_Backend_SSO_Yggdrasil_JWT_Integration.md`](./03_15_Backend_SSO_Yggdrasil_JWT_Integration.md)
- JWT pattern memory: `asgard_jwt_auth_pattern.md`
- SemVer + release process memory: `semver_release_process.md`
- Token mint guide: [`Yggdrasil/docs/heimdall-key-gen.md`](https://github.com/MegaWiz-Dev-Team/Yggdrasil/blob/main/docs/heimdall-key-gen.md)
- Mimir incident 2026-05-17 (image cache lesson): [`Asgard/docs/incidents/2026-05-17-mimir-503/`](https://github.com/MegaWiz-Dev-Team/Asgard/tree/main/docs/incidents/2026-05-17-mimir-503)

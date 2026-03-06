# Pull Request Template

## Description

{type}(#{issue_number}): {Brief description of what this PR does}

## Changes

- [List of specific changes made]
- [Another change]
- [Another change]

## Related Issues

Closes #{issue_number}

## Type of Change

- [ ] 🐛 Bug fix (non-breaking change that fixes an issue)
- [ ] ✨ New feature (non-breaking change that adds functionality)
- [ ] 💥 Breaking change (fix or feature that causes existing functionality to change)
- [ ] 📝 Documentation update
- [ ] ♻️ Refactoring (no functional changes)

## Testing

### Automated Tests
- [ ] `cargo test` — All {N} tests passing
- [ ] `npx next build` — Compiled successfully

### Manual/Integration Tests
- [ ] [Specific test scenario] — ✅ Pass
- [ ] [Another test scenario] — ✅ Pass

### Test Script Reference
See `docs/iso_29110/si/SI_04_{N}_Sprint{N}_TestScript.md`

## ISO Documentation Updated

- [ ] Sprint Report (PM-02) created/updated
- [ ] Test Script (SI-04) created/updated
- [ ] Traceability Matrix (SI-03) updated
- [ ] Status Reports master file updated

## Screenshots (if UI changes)

[Attach screenshots or browser recording paths here]

## Self-Review Checklist

- [ ] Code follows project patterns (see `rust-backend-patterns` skill)
- [ ] No secrets or API keys in code
- [ ] Tenant isolation verified (all queries filter by `tenant_id`)
- [ ] Error responses don't leak internal details
- [ ] All new functionality has tests

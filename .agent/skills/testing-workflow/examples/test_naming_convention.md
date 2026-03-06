# Test Naming Convention

## Format
```
TC_SP{sprint}_{category}{number}
```

## Category Codes

| Code | Full Name      | When to Use                                    |
| ---- | -------------- | ---------------------------------------------- |
| `F`  | Frontend Build | `npx next build` compilation check             |
| `U`  | Unit/Feature   | Testing a specific new feature or behavior     |
| `R`  | Regression     | Verifying an existing feature still works      |
| `I`  | Integration    | Testing interaction between backend ↔ frontend |
| `P`  | Performance    | Polling intervals, load time, bandwidth tests  |
| `S`  | Security       | JWT auth, tenant isolation, permission checks  |
| `B`  | Bug fix        | Verifying a specific bug has been resolved     |

## Numbering Rules
- Numbers restart at 1 per category per sprint
- `U1` through `U{N}` for feature tests
- `R1` through `R{N}` for regression tests

## Examples
```
TC_SP21_F1  → Sprint 21, Frontend Build #1 (npm build)
TC_SP21_U3  → Sprint 21, Unit/Feature Test #3 (QA status badge: processing)
TC_SP21_R2  → Sprint 21, Regression Test #2 (Select All/Deselect All)
TC_SP21_P1  → Sprint 21, Performance Test #1 (Polling starts after trigger)
TC_SP15_S1  → Sprint 15, Security Test #1 (Tenant isolation check)
```

## Grouping in Test Script
Group tests by section headers matching categories:
```markdown
## 1. Frontend Build      ← F tests
## 2. Feature Tests        ← U tests
## 3. Regression Tests     ← R tests
## 4. Integration Tests    ← I tests
## 5. Security Tests       ← S tests
```

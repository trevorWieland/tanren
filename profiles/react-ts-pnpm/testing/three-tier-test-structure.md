# Three-Tier BDD Structure

TypeScript testing is tiered by runtime, but behavior proof is always Gherkin-driven through Playwright + Cucumber.

```
apps/web/tests/
├── unit/
│   ├── features/
│   │   └── auth-validation.feature
│   └── steps/
│       └── auth-validation.steps.ts
├── integration/
│   ├── features/
│   │   └── login-flow.feature
│   └── steps/
│       └── login-flow.steps.ts
└── e2e/
    ├── features/
    │   └── checkout.feature
    └── steps/
        └── checkout.steps.ts
```

**Rules:**
- Canonical behavior runner: Playwright + Cucumber
- Every scenario includes a stable behavior ID tag
- No executable non-BDD-only tests in CI behavior gates
- Unit/integration/e2e tiers are runtime categories, not format categories

**Why:** One behavior format keeps product intent and test evidence aligned across frontend layers.

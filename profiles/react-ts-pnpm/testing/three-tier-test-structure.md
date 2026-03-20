# Three-Tier Test Structure

All tests fall into one of three tiers: unit, integration, or component (Storybook). No exceptions.

```
src/
├── features/
│   └── auth/
│       ├── components/
│       │   ├── login-form.tsx
│       │   ├── login-form.test.tsx      # Unit tests (logic only)
│       │   └── login-form.stories.tsx   # Component tests (render + interaction)
│       ├── hooks/
│       │   ├── use-auth.ts
│       │   └── use-auth.test.ts         # Unit tests
│       └── utils/
│           ├── validate-token.ts
│           └── validate-token.test.ts   # Unit tests
tests/
└── integration/                         # Integration tests
    └── auth/
        └── login-flow.test.ts
```

**Tier definitions:**

**Unit tests** (`*.test.ts(x)` co-located with source):
- Fast: <500ms per test
- Environment: Vitest + jsdom
- Mocks allowed and encouraged
- Test isolated logic: hooks, utilities, state transitions, data transforms
- Never render components in unit tests — use Storybook for that
- Co-located next to the file they test

**Integration tests** (`tests/integration/`):
- Moderate speed: <5s per test
- Environment: Vitest Browser Mode (Playwright)
- Real browser rendering, real DOM, real CSS
- Test multi-component flows: form submission, navigation, API round-trips
- MSW for network mocking — real everything else
- Separate directory, mirrors feature structure

**Component tests** (`*.stories.tsx` co-located with source):
- Fast: <2s per test
- Environment: Storybook with Vitest addon
- Test component rendering, visual states, and user interactions
- One story per meaningful component state
- Play functions for interaction assertions
- axe-core for accessibility checks
- Storybook is the source of truth for how components render

**CI execution:**
- Unit + component tests: Run on every PR (fast feedback)
- Integration tests: Run on every PR or on schedule (slower)
- All tiers must pass before merge

**Never place tests:**
- Outside the three tier locations
- In ad-hoc directories (scripts, benchmarks, etc.)
- In a flat `__tests__/` directory disconnected from source

**Why:** Clear purpose and scope for each tier. Unit tests verify logic, Storybook verifies rendering, integration tests verify flows. Selective execution by tier keeps feedback loops fast.

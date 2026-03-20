# Strict Linting Gate

All four gates must pass before merge: format, lint, typecheck, and test. No exceptions, no overrides.

```bash
# ✓ Good: Full gate check before merge
make check   # Runs format + lint + typecheck + test

# Individual gates:
oxfmt --check .               # Zero formatting diffs
oxlint .                      # Zero lint errors
tsc --noEmit                  # Zero type errors
vitest run                    # Zero test failures
```

```bash
# ✗ Bad: Skipping gates or suppressing errors
tsc --noEmit || true          # Silencing type errors
oxlint . --quiet              # Hiding warnings
vitest run --passWithNoTests  # Allowing empty test suites
```

**Gate definitions:**

**Format gate (oxfmt):**
- oxfmt must report zero diffs on all `.ts`, `.tsx`, `.json`, `.yaml`, `.md` files
- Tailwind class sorting is enforced by oxfmt (built-in, no plugin needed)
- Import sorting is enforced by oxfmt
- Run `oxfmt .` to auto-fix before committing

**Lint gate (oxlint):**
- oxlint must report zero errors
- Required plugin categories: `react`, `typescript`, `jsx-a11y`, `import`, `react-perf`, `vitest`
- No `console.log` in production code — enforced by `no-console` rule
- No `eslint-disable` comments without a paired `eslint-enable` and an explanation
- Warnings are treated as errors in CI

**Typecheck gate (tsc):**
- `tsc --noEmit` must report zero errors across all workspace packages
- Run via Turborepo to leverage caching: `turbo typecheck`
- Strict compiler config as defined in `strict-compiler-config` standard

**Test gate (vitest):**
- All unit tests and component tests (Storybook) must pass
- Zero skipped tests allowed (see `no-test-skipping` standard)
- Integration tests run on every PR or on schedule

**CI pipeline:**
- All four gates run in parallel where possible (lint + format are independent of typecheck + test)
- Any single gate failure blocks the PR
- `make check` (or equivalent) is the single command that runs everything
- Pre-commit hooks run `oxfmt` and `oxlint` on staged files via lint-staged

**No suppression:**
- Never use `// @ts-ignore` — use `// @ts-expect-error` with an explanation if unavoidable
- Never use `any` to silence type errors (see `no-any-enforcement` standard)
- Never disable lint rules at the file level without team review

**Why:** A single broken gate means broken code reaches the main branch. Running all gates locally before push catches issues before CI, keeping feedback loops fast. Treating warnings as errors prevents gradual quality decay.

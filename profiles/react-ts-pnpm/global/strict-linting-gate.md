---
kind: standard
name: strict-linting-gate
category: global
importance: high
applies_to: []
applies_to_languages:
  - typescript
applies_to_domains: []
---

# Strict Linting Gate

All four gates must pass before merge: **format, lint, typecheck, test**. No
exceptions, no overrides. Warnings are errors in CI.

```bash
# ✓ Good: Full gate check before merge
just ci   # Runs format + lint + typecheck + test for every workspace surface

# Individual gates:
oxfmt --check .               # Zero formatting diffs
oxlint .                      # Zero lint errors (warnings = errors)
tsc --noEmit                  # Zero type errors
vitest run                    # Zero test failures
```

```bash
# ✗ Bad: Skipping gates or suppressing errors
tsc --noEmit || true          # Silencing type errors
oxlint . --quiet              # Hiding warnings
vitest run --passWithNoTests  # Allowing empty test suites
```

## Gate definitions

**Format gate (oxfmt):**
- oxfmt must report zero diffs on all `.ts`, `.tsx`, `.json`, `.yaml`, `.md`
  files.
- Tailwind class sorting and import sorting are both built-in (no plugin
  needed).
- Run `oxfmt .` to auto-fix before committing.

**Lint gate (oxlint):**
- oxlint must report zero errors AND zero warnings — warnings are errors in
  CI.
- The web app pins its config at `apps/web/.oxlintrc.json`.

**Typecheck gate (tsc):**
- `tsc --noEmit` must report zero errors across all workspace packages.
- Strict compiler config as defined in the `strict-compiler-config` standard.

**Test gate (vitest):**
- All unit tests and Storybook component tests (via `@storybook/addon-vitest`)
  must pass.
- Zero skipped tests allowed (see `no-test-skipping`).
- Integration tests run on every PR.

## oxlint plugin set

The web app enables the following oxlint plugin set in
`apps/web/.oxlintrc.json`. New tanren-web profiles must keep this list as a
floor; profiles may opt into additional plugins but may not drop one.

| Plugin | Purpose |
|---|---|
| `react` | React-specific patterns (rules of hooks, JSX correctness, no-literal copy) |
| `react-perf` | Render-perf footguns (unstable refs in deps, inline object props) |
| `nextjs` | App Router / `next/link` / `next/image` correctness |
| `jsx-a11y` | Accessibility rules; full per-rule list in `accessibility-enforcement.md` |
| `import` | Import-order, no-cycle, no-extraneous-dependencies |
| `unicorn` | General quality / footgun catches (no-array-for-each, prefer-node-protocol, etc.) |
| `typescript` | TypeScript-specific rules (`no-explicit-any`, etc.) |

## Mandated rule list (R-0001 floor)

These rules are explicitly enabled at error severity in
`apps/web/.oxlintrc.json`. Each profile that derives from `react-ts-pnpm`
must keep them as a floor.

- `react/jsx-no-literals` — **error**. No literal user-facing strings in JSX.
  Enforces the i18n contract documented in `i18n-enforcement.md`. The
  allowlist mirrors that profile (empty strings, fragments, non-user-visible
  `aria-*`).
- `react/forbid-dom-props: ["style"]` — **error** with an allowlist for the
  small set of utilities that read design tokens. Inline `style={{ ... }}`
  is forbidden; design-token utility classes (see
  `architecture/styling-and-design-tokens.md`) are the canonical mechanism.
- `react/function-component-return-type` — **error**. Function components must
  return `ReactNode`, NOT `JSX.Element`. `JSX.Element` is incompatible with
  React 19's allowed return shapes (`null`, strings, fragments).
- `no-restricted-globals: ["localStorage", "sessionStorage"]` — **error**.
  Tanren's web surface uses HTTP-only cookie sessions; `localStorage` /
  `sessionStorage` are forbidden as token storage. The cookie is set by the
  API and is never read or written from JavaScript.
- `jsx-a11y/label-has-associated-control` — **error**. Every input needs a
  `<label htmlFor="...">` whose `htmlFor` matches the input's `id`.
  Wrapping a label around an input is NOT sufficient.
- `jsx-a11y/aria-props` — **error**. ARIA attributes must be valid (no
  typos, no invalid props).
- `jsx-a11y/aria-role` — **error**. `role="..."` must be a valid ARIA role.
- `jsx-a11y/no-onchange` — **error**. Use `onBlur`/`onInput`/typed React
  events; raw `onChange` on form selects is disallowed.
- `jsx-a11y/click-events-have-key-events` — **error**. Any element with
  `onClick` that is not a native button or anchor must also handle
  `onKeyDown`/`onKeyUp` (use semantic elements instead — see
  `accessibility-enforcement.md`).

```jsonc
// ✓ Good: explicit oxlint config (excerpt)
{
  "plugins": [
    "react", "react-perf", "nextjs", "jsx-a11y",
    "import", "unicorn", "typescript"
  ],
  "rules": {
    "react/jsx-no-literals": "error",
    "react/forbid-dom-props": ["error", { "forbid": ["style"] }],
    "react/function-component-return-type": "error",
    "no-restricted-globals": [
      "error",
      "localStorage",
      "sessionStorage"
    ],
    "jsx-a11y/label-has-associated-control": "error",
    "jsx-a11y/aria-props": "error",
    "jsx-a11y/aria-role": "error",
    "jsx-a11y/no-onchange": "error",
    "jsx-a11y/click-events-have-key-events": "error"
  }
}
```

## CI pipeline

- All four gates (`format`, `lint`, `typecheck`, `test`) run in parallel where
  possible (lint + format are independent of typecheck + test).
- Any single gate failure blocks the PR.
- `just ci` is the single command that runs everything.
- Pre-commit hooks (`lefthook.yml`) run `oxfmt` and `oxlint` on staged files.
- Warnings are upgraded to errors in CI — the lint gate fails on either.

## No suppression

- Never use `// @ts-ignore` — use `// @ts-expect-error` with a justification
  if unavoidable.
- Never use `any` to silence type errors (see `no-any-enforcement`).
- Never disable lint rules at the file level. Per-rule relaxation goes in
  `apps/web/.oxlintrc.json` (or the relevant package's config) with a
  comment explaining why.

**Why:** A single broken gate means broken code reaches the main branch.
Running all gates locally before push catches issues before CI, keeping
feedback loops fast. Treating warnings as errors prevents gradual quality
decay. Pinning the plugin set and rule list in this profile means new
packages inherit the same floor without rediscovery.

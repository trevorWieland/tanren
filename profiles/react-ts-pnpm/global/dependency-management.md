---
kind: standard
name: dependency-management
category: global
importance: high
applies_to: []
applies_to_languages:
  - typescript
applies_to_domains: []
---

# Dependency Management

Use the pnpm catalog for version alignment. No floating versions in individual
`package.json` files.

```yaml
# ✓ Good: Centralized versions in pnpm-workspace.yaml
packages:
  - "apps/*"
  - "packages/*"

catalog:
  react: "19.1.0"
  react-dom: "19.1.0"
  next: "15.1.0"
  typescript: "5.8.3"
  "@tanstack/react-query": "5.75.0"
  "@inlang/paraglide-js": "^2"
  "@inlang/sdk-js": "*"
  valibot: "^1"
  tailwindcss: "^4"
  "@tailwindcss/postcss": "^4"
  "@storybook/nextjs-vite": "^9"
  "@storybook/addon-vitest": "^9"
  "@storybook/addon-a11y": "^9"
  "@playwright/test": "*"
  "playwright-bdd": "*"
  concurrently: "*"
  oxlint: "*"
  vitest: "3.2.0"
```

```jsonc
// ✓ Good: Package references the catalog, not a version
{
  "dependencies": {
    "react": "catalog:default",
    "react-dom": "catalog:default",
    "next": "catalog:default",
    "@tanstack/react-query": "catalog:default",
    "@inlang/paraglide-js": "catalog:default",
    "valibot": "catalog:default"
  },
  "devDependencies": {
    "typescript": "catalog:default",
    "vitest": "catalog:default",
    "@storybook/nextjs-vite": "catalog:default"
  }
}
```

```jsonc
// ✗ Bad: Floating versions in individual package.json
{
  "dependencies": {
    "react": "^19.0.0",
    "@tanstack/react-query": "^5.50.0"
  }
}
```

**Rules:**
- All shared dependency versions are defined in `pnpm-workspace.yaml` via
  `catalog:`.
- Individual `package.json` files reference `catalog:default` — never pin
  versions directly.
- Internal packages use the workspace protocol: `"@tanren/<pkg>": "workspace:*"`.
- `pnpm-lock.yaml` is always committed — never gitignored.
- CI validates the lockfile is up-to-date: `pnpm install --frozen-lockfile`.

## Modern 2026 stack additions for R-0001

R-0001 (the first behavior PR — "Create an account") introduces the modern web
toolchain that subsequent R-* PRs inherit. The catalog adds:

| Package | Version range | Role |
|---|---|---|
| `@inlang/paraglide-js` | `^2` | Compile-time i18n; replaces `react-i18next`. See `i18n-enforcement.md`. |
| `@inlang/sdk-js` | `*` | paraglide tooling / compiler glue. |
| `valibot` | `^1` | Runtime validation; replaces Zod. See `no-any-enforcement.md`. |
| `tailwindcss` | `^4` | CSS engine with CSS-first `@theme` config. |
| `@tailwindcss/postcss` | `^4` | PostCSS plugin for Tailwind v4. |
| `@storybook/nextjs-vite` | `^9` | Storybook 9 framework using Vite (NOT the legacy webpack `@storybook/nextjs`). |
| `@storybook/addon-vitest` | `^9` | Runs stories as real-browser component tests via Vitest. |
| `@storybook/addon-a11y` | `^9` | axe-core a11y audit per story. |
| `@playwright/test` | `*` | Browser test runner for `@web` BDD scenarios. |
| `playwright-bdd` | `*` | Maps `tests/bdd/features/*.feature` files into native Playwright tests. |
| `concurrently` | `*` | Runs `paraglide-js` compile-watch alongside `next dev`. |
| `oxlint` | `*` | TypeScript/React linter — extended plugin set documented in `strict-linting-gate.md`. |

**i18n compile step.** Paraglide is a compile-time toolchain. Every web
package script that runs Next.js wires the compile step in:

```jsonc
// apps/web/package.json (excerpt)
{
  "scripts": {
    "i18n:compile": "paraglide-js compile --project ./src/i18n/project.inlang --outdir ./src/i18n/paraglide",
    "i18n:watch": "paraglide-js compile --project ./src/i18n/project.inlang --outdir ./src/i18n/paraglide --watch",
    "dev": "concurrently -n i18n,next \"pnpm i18n:watch\" \"next dev\"",
    "build": "pnpm i18n:compile && next build",
    "typecheck": "pnpm i18n:compile && tsc --noEmit"
  }
}
```

The first compile also runs from `just bootstrap` so a clean checkout has the
generated `paraglide/` directory before any TypeScript-aware tool reads
`apps/web`.

**Update strategy:**
- Update dependencies proactively via `pnpm update` — don't wait for breakage.
- Review changelogs for breaking changes and migrate immediately.
- Run the full gate (`just ci`) after every dependency update.
- Security advisories must be addressed within 48 hours.

**Dependency hygiene:**
- No unused dependencies — enforced via `depcheck` (or equivalent) in CI.
- No duplicate versions of the same package across the workspace — the pnpm
  catalog prevents this.
- Prefer packages with TypeScript types included — avoid `@types/*` when the
  package ships its own types.

**Adding new dependencies:**
1. Add the version to `catalog` in `pnpm-workspace.yaml`.
2. Reference `catalog:default` in the consuming `package.json`.
3. Run `pnpm install` to update the lockfile.
4. Verify `just ci` passes.

**Why:** Catalog versioning eliminates version drift across a monorepo —
every package uses the same version of shared dependencies. This prevents
"works in my package, breaks in yours" bugs and makes dependency updates
atomic. The compile-time pieces of the toolchain (paraglide-js, Tailwind v4)
must be wired into the package scripts so a fresh checkout boots into a
working state without manual follow-ups.

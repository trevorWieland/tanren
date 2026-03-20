# Dependency Management

Use pnpm catalog for version alignment. No floating versions in individual package.json files.

```yaml
# ✓ Good: Centralized versions in pnpm-workspace.yaml
packages:
  - "apps/*"
  - "packages/*"

catalog:
  react: "19.1.0"
  react-dom: "19.1.0"
  typescript: "5.8.3"
  "@tanstack/react-query": "5.75.0"
  "@tanstack/react-router": "1.120.0"
  zod: "3.24.0"
  vitest: "3.2.0"
```

```jsonc
// ✓ Good: Package references catalog, not version
{
  "dependencies": {
    "react": "catalog:default",
    "react-dom": "catalog:default",
    "@tanstack/react-query": "catalog:default"
  },
  "devDependencies": {
    "typescript": "catalog:default",
    "vitest": "catalog:default"
  }
}
```

```jsonc
// ✗ Bad: Floating versions in individual package.json
{
  "dependencies": {
    "react": "^19.0.0",
    "react-dom": "~19.0.0",
    "@tanstack/react-query": "^5.50.0"
  }
}
```

**Rules:**
- All shared dependency versions defined in `pnpm-workspace.yaml` via `catalog:`
- Individual `package.json` files reference `catalog:default` — never pin versions directly
- Internal packages use workspace protocol: `"@myorg/ui": "workspace:*"`
- `pnpm-lock.yaml` is always committed — never gitignored
- CI validates the lockfile is up-to-date: `pnpm install --frozen-lockfile`

**Update strategy:**
- Update dependencies proactively via `pnpm update` — don't wait for breakage
- Review changelogs for breaking changes and migrate immediately
- Run full gate check (`make check`) after every dependency update
- Security advisories must be addressed within 48 hours

**Dependency hygiene:**
- No unused dependencies — enforce with `depcheck` or equivalent in CI
- No duplicate versions of the same package across the workspace — pnpm catalog prevents this
- Prefer packages with TypeScript types included — avoid `@types/*` when the package ships its own

**Adding new dependencies:**
1. Add version to `catalog` in `pnpm-workspace.yaml`
2. Reference `catalog:default` in the consuming `package.json`
3. Run `pnpm install` to update the lockfile
4. Verify `make check` passes

**Why:** Catalog versioning eliminates version drift across a monorepo — every package uses the same version of shared dependencies. This prevents "works in my package, breaks in yours" bugs and makes dependency updates atomic. Committed lockfiles ensure reproducible installs across dev machines and CI.

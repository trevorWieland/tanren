---
kind: standard
name: monorepo-conventions
category: architecture
importance: high
applies_to: []
applies_to_languages:
  - typescript
applies_to_domains:
  - architecture
---

# Monorepo Conventions

Use pnpm workspaces with Turborepo. All shared code lives in `packages/`, all deployable apps live in `apps/`.

```yaml
# ✓ Good: pnpm-workspace.yaml with clear structure
packages:
  - "apps/*"
  - "packages/*"
```

```
# ✓ Good: Workspace layout
├── apps/
│   ├── web/                    # Main web application
│   └── admin/                  # Admin dashboard
├── packages/
│   ├── ui/                     # Shared component library
│   ├── hooks/                  # Shared React hooks
│   ├── utils/                  # Shared utilities
│   ├── typescript-config/      # Shared tsconfig bases
│   ├── vitest-config/          # Shared Vitest configuration
│   └── vite-config/            # Shared Vite configuration
├── pnpm-workspace.yaml
├── turbo.json
└── package.json
```

```
# ✗ Bad: Flat structure without workspace separation
├── src/
│   ├── app/
│   ├── components/
│   ├── hooks/
│   └── utils/
├── package.json                # Single package — nothing is shared
```

**Workspace rules:**
- `apps/` contains deployable applications — each has its own `package.json`, build config, and entry point
- `packages/` contains shared libraries — each is a proper npm package with `exports` field
- Internal imports use workspace protocol: `"@myorg/ui": "workspace:*"` in `package.json`
- Never use relative imports across package boundaries (`../../packages/ui/src/button`)

**Shared config packages:**
- `typescript-config` — base tsconfig with all strict flags, extended by every package
- `vitest-config` — shared Vitest setup with tier definitions, coverage thresholds
- `vite-config` — shared Vite plugins and build configuration
- Config packages are `devDependencies`, never `dependencies`

**Turborepo orchestration:**
- Define `build`, `lint`, `typecheck`, `test`, and `format` tasks in `turbo.json`
- Use `dependsOn` to express task relationships (e.g., `build` depends on upstream `build`)
- Enable remote caching for CI — never rebuild what hasn't changed
- Use `inputs` and `outputs` to scope caching correctly

```jsonc
// ✓ Good: turbo.json with task dependencies and caching
{
  "tasks": {
    "build": {
      "dependsOn": ["^build"],
      "outputs": ["dist/**"]
    },
    "typecheck": {
      "dependsOn": ["^build"]
    },
    "test": {
      "dependsOn": ["^build"]
    },
    "lint": {},
    "format": {}
  }
}
```

**Catalog versioning:**
- Define shared dependency versions in `pnpm-workspace.yaml` using `catalog:`
- All packages reference `catalog:default` instead of pinning versions individually
- See `dependency-management` standard for full catalog rules

**Why:** Monorepos with workspace protocol enable code sharing without publishing to a registry. Turborepo caching makes CI fast even as the repo grows. Shared config packages eliminate configuration drift between apps and libraries.

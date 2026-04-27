---
kind: standard
name: barrel-export-rules
category: architecture
importance: high
applies_to: []
applies_to_languages:
  - typescript
applies_to_domains:
  - architecture
---

# Barrel Export Rules

Barrel exports (`index.ts`) are allowed only at package boundaries. Never create barrel files inside feature directories.

```typescript
// ✓ Good: Barrel at package boundary
// packages/ui/src/index.ts — public API of the UI package
export { Button, type ButtonProps } from "./components/button";
export { Card, type CardProps } from "./components/card";
export { Dialog, type DialogProps } from "./components/dialog";
```

```typescript
// ✗ Bad: Barrel inside feature directory
// src/features/auth/index.ts — causes circular imports, breaks tree-shaking
export { LoginForm } from "./components/login-form";
export { useAuth } from "./hooks/use-auth";
export { validateToken } from "./utils/validate-token";
export type { AuthState } from "./types";
```

**Rules:**
- Barrel exports (`index.ts`) exist only at the root of a package: `packages/ui/src/index.ts`
- Never create `index.ts` inside `src/features/`, `src/components/`, or any internal directory
- Import directly from the source file within the same package: `import { LoginForm } from "./components/login-form"`
- Import from the package name across package boundaries: `import { Button } from "@myorg/ui"`

**Package `exports` field:**
Every package must define its public API in `package.json`:

```jsonc
// ✓ Good: Explicit exports field
{
  "name": "@myorg/ui",
  "exports": {
    ".": {
      "import": "./dist/index.js",
      "types": "./dist/index.d.ts"
    }
  }
}
```

```jsonc
// ✗ Bad: No exports field — consumers can import anything
{
  "name": "@myorg/ui",
  "main": "./dist/index.js"
}
```

**Deep imports:**
- If a package needs to expose multiple entry points, use explicit `exports` paths
- Never allow consumers to import from internal paths (`@myorg/ui/src/components/button`)

```jsonc
// ✓ Good: Multiple explicit entry points
{
  "exports": {
    ".": "./dist/index.js",
    "./hooks": "./dist/hooks/index.js",
    "./utils": "./dist/utils/index.js"
  }
}
```

**Why:** Internal barrel files cause circular dependency chains that break hot module replacement, slow down bundlers, and defeat tree-shaking. Package-boundary barrels are fine because they define a stable public API for external consumers.

# Strict Compiler Config

Enable full strict mode with all additional safety flags. Never ship a tsconfig with `strict: true` alone.

```jsonc
// ✓ Good: Full strict tsconfig
{
  "compilerOptions": {
    "strict": true,
    "target": "ES2022",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "verbatimModuleSyntax": true,
    "erasableSyntaxOnly": true,
    "exactOptionalPropertyTypes": true,
    "noUncheckedIndexedAccess": true,
    "noPropertyAccessFromIndexSignature": true,
    "noImplicitOverride": true,
    "noFallthroughCasesInSwitch": true,
    "declaration": true,
    "declarationMap": true,
    "sourceMap": true,
    "skipLibCheck": true
  }
}
```

```jsonc
// ✗ Bad: Minimal strict only
{
  "compilerOptions": {
    "strict": true,
    "target": "ES2020",
    "module": "ESNext"
  }
}
```

**Required flags beyond `strict: true`:**
- `verbatimModuleSyntax` — forces explicit `import type` annotations, aligns with how modern bundlers process modules
- `erasableSyntaxOnly` — errors on TypeScript constructs with runtime behavior (enums, namespaces, parameter properties). Ensures compatibility with Node.js native type-stripping and modern build tools
- `exactOptionalPropertyTypes` — distinguishes between a property being missing and explicitly set to `undefined`. Critical for API patch operations
- `noUncheckedIndexedAccess` — adds `undefined` to index signature results, preventing silent null dereferences on array/object access
- `noPropertyAccessFromIndexSignature` — forces bracket notation for dynamic keys, making it clear when access is unchecked
- `noImplicitOverride` — requires `override` keyword when overriding base class members
- `noFallthroughCasesInSwitch` — prevents accidental fallthrough in switch statements

**Enums are banned:**
`erasableSyntaxOnly` makes all TypeScript enums a compile error. Use `as const` objects instead:

```typescript
// ✓ Good: as const object
const Status = {
  Active: "active",
  Inactive: "inactive",
  Pending: "pending",
} as const;

type Status = (typeof Status)[keyof typeof Status];

// ✗ Bad: TypeScript enum (compile error with erasableSyntaxOnly)
enum Status {
  Active = "active",
  Inactive = "inactive",
  Pending = "pending",
}
```

**Shared config package:**
- Publish a `typescript-config` package in your workspace with this base config
- All apps and packages extend it: `"extends": "@myorg/typescript-config/base.json"`
- Never duplicate compiler options across packages

**Why:** Catches entire classes of bugs at compile time instead of runtime. The additional flags beyond `strict` close gaps that `strict` alone leaves open — unchecked index access, ambiguous optional properties, and runtime-dependent syntax that breaks modern tooling.

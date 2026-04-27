---
kind: standard
name: discriminated-unions
category: typescript
importance: high
applies_to:
  - "**/*.ts"
  - "**/*.tsx"
applies_to_languages:
  - typescript
applies_to_domains:
  - typescript
---

# Discriminated Unions

Use discriminated unions for variant types. Never model variants with optional fields.

```typescript
// ✓ Good: Discriminated union with exhaustive handling
type ApiResult<T> =
  | { kind: "success"; data: T }
  | { kind: "error"; error: string; code: number }
  | { kind: "loading" };

function renderResult(result: ApiResult<User>): ReactNode {
  switch (result.kind) {
    case "success":
      return <UserCard user={result.data} />;
    case "error":
      return <ErrorBanner message={result.error} />;
    case "loading":
      return <Spinner />;
    default: {
      const _exhaustive: never = result;
      return _exhaustive;
    }
  }
}

// ✗ Bad: Optional fields — no compile-time exhaustiveness
type ApiResult<T> = {
  data?: T;
  error?: string;
  code?: number;
  loading?: boolean;
};
```

**Rules:**
- Use `kind` or `type` as the discriminant field — pick one per codebase and stick with it
- Every `switch` on a discriminated union must have a `default` case that assigns to `never` for exhaustiveness checking
- Add new variants to the union type first — the compiler will flag every unhandled location
- Never use boolean flags or optional fields to model mutually exclusive states

**Enum-like constants:**
Use `as const` objects for fixed sets of values (enums are banned by `erasableSyntaxOnly`):

```typescript
// ✓ Good: as const with derived type
const Role = {
  Admin: "admin",
  Member: "member",
  Guest: "guest",
} as const;

type Role = (typeof Role)[keyof typeof Role];

function checkAccess(role: Role): boolean {
  switch (role) {
    case Role.Admin:
      return true;
    case Role.Member:
      return true;
    case Role.Guest:
      return false;
    default: {
      const _exhaustive: never = role;
      return _exhaustive;
    }
  }
}

// ✗ Bad: String literal union without named constants
type Role = "admin" | "member" | "guest";
```

**Why:** Discriminated unions make illegal states unrepresentable. The compiler enforces exhaustive handling — when a new variant is added, every switch/if chain that doesn't handle it becomes a compile error. Optional fields can't provide this guarantee.

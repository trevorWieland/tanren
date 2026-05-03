---
kind: standard
name: no-any-enforcement
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

# No Any Enforcement

Never use `any` in application code. Use `unknown` for genuinely unknown types
and narrow with type guards.

```typescript
// ✓ Good: unknown with type guard
function parseApiResponse(data: unknown): User {
  if (!isUser(data)) {
    throw new Error("Invalid user data");
  }
  return data;
}

function isUser(value: unknown): value is User {
  return (
    typeof value === "object" &&
    value !== null &&
    "id" in value &&
    "name" in value
  );
}

// ✗ Bad: any bypasses all type checking
function parseApiResponse(data: any): User {
  return data; // No validation, no safety
}
```

**Rules:**
- `any` is banned in all application code — enforced by oxlint rule
  `typescript/no-explicit-any`.
- Use `unknown` when the type is genuinely not known at compile time.
- Always narrow `unknown` with type guards, runtime-validation schemas, or
  assertion functions before use.
- Use generics to propagate types instead of falling back to `any`.

```typescript
// ✓ Good: Generic preserves type information
function first<T>(items: T[]): T | undefined {
  return items[0];
}

// ✗ Bad: any loses type information
function first(items: any[]): any {
  return items[0];
}
```

## Runtime validation: valibot

Tanren standardises on **`valibot`** for runtime validation of external data
(form input, API responses, environment variables). Schemas double as the
single source of truth for the corresponding TypeScript type via
`v.InferOutput<typeof Schema>`.

**Why valibot, not Zod or ArkType.**
- A typical sign-up form schema costs ~17.7 KB minified+gzipped of Zod's
  standard build to ship to the browser. The same schema in valibot's
  tree-shakable modular API is ~1.4 KB — roughly 12× smaller. For a
  bundle-size-sensitive web surface this matters.
- ArkType is the fastest of the three at runtime, but its DSL pays an
  ergonomic cost on small form schemas (the very shapes the account flow
  needs).
- For the tanren web surface — account flow now and the next ~50 R-* slices
  ahead — valibot is the canonical choice.

```typescript
// ✓ Good: valibot schema validates and types external data
import * as v from "valibot";

const SignUpInput = v.object({
  email: v.pipe(v.string(), v.email(), v.toLowerCase(), v.trim()),
  password: v.pipe(v.string(), v.minLength(8)),
  display_name: v.pipe(v.string(), v.minLength(1), v.trim()),
});

type SignUpInput = v.InferOutput<typeof SignUpInput>;

const result = v.safeParse(SignUpInput, formInput);
if (!result.success) {
  // render result.issues — each issue has a path + message ready for the form
  return renderValidationErrors(result.issues);
}
const validated: SignUpInput = result.output;
```

- Compose schemas with `v.pipe(...)` for ordered transforms + checks.
- Use `v.safeParse(schema, input)` at boundary points; `v.parse` only when a
  thrown exception is actually wanted.
- Derive types from the schema, not the other way around — there is one
  source of truth.

**Exceptions (extremely rare, high bar):**
`any` is permitted only when **all** of the following hold:
1. Third-party library types are incorrect or missing.
2. A type-safe wrapper is not feasible.
3. The usage is isolated to a single adapter module.
4. A comment immediately above the line explains why.

**Why:** `any` silently disables type checking for everything it touches. One
`any` propagates through assignments, return types, and function calls —
infecting the entire call chain. `unknown` forces explicit narrowing, and
valibot makes that narrowing tree-shakable, type-safe, and cheap to ship.

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

Never use `any` in application code. Use `unknown` for genuinely unknown types and narrow with type guards.

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
- `any` is banned in all application code — enforced by oxlint rule `typescript/no-explicit-any`
- Use `unknown` when the type is genuinely not known at compile time
- Always narrow `unknown` with type guards, Zod schemas, or assertion functions before use
- Use generics to propagate types instead of falling back to `any`

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

**Zod for runtime validation:**
- Use Zod schemas to validate external data (API responses, form input, environment variables)
- Derive TypeScript types from Zod schemas with `z.infer<typeof schema>`
- Never assert external data as a known type without validation

```typescript
// ✓ Good: Zod schema validates and types external data
import { z } from "zod";

const UserSchema = z.object({
  id: z.string().uuid(),
  name: z.string().min(1),
  email: z.string().email(),
});

type User = z.infer<typeof UserSchema>;

function parseUser(data: unknown): User {
  return UserSchema.parse(data);
}
```

**Exceptions (extremely rare, high bar):**
`any` only when **all** are true:
1. Third-party library types are incorrect or missing
2. A type-safe wrapper is not feasible
3. The usage is isolated to a single adapter module
4. A `// eslint-disable-next-line` comment explains why

**Why:** `any` silently disables type checking for everything it touches. One `any` propagates through assignments, return types, and function calls — infecting the entire call chain. `unknown` forces explicit narrowing, keeping the type system intact.

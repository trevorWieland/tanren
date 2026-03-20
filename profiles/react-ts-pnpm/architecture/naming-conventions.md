# Naming Conventions

Use consistent naming conventions across all code. Never mix styles.

```typescript
// ✓ Good: Consistent naming
import { UserProfile } from "./components/user-profile";    // kebab-case file, PascalCase component
import { useAuth } from "./hooks/use-auth";                 // kebab-case file, camelCase hook
import { validateEmail } from "./utils/validate-email";     // kebab-case file, camelCase function
import type { AuthState } from "./types";                   // PascalCase type
import { API_BASE_URL } from "./constants";                 // SCREAMING_SNAKE_CASE constant

// ✗ Bad: Inconsistent naming
import { UserProfile } from "./components/UserProfile";     // PascalCase file
import { useAuth } from "./hooks/UseAuth";                  // PascalCase file for hook
import { validate_email } from "./utils/validate_email";    // snake_case function and file
```

**File naming:**
- All files: `kebab-case` — `user-profile.tsx`, `use-auth.ts`, `validate-email.ts`
- Test files: `{name}.test.ts(x)` — `user-profile.test.tsx`
- Story files: `{name}.stories.tsx` — `user-profile.stories.tsx`
- Type-only files: `types.ts` (per feature) or `{name}.types.ts`
- Config files: `kebab-case` — `vite.config.ts`, `vitest.config.ts`

**Code naming:**
- Components: `PascalCase` — `UserProfile`, `AuthProvider`, `LoginForm`
- Hooks: `camelCase` with `use` prefix — `useAuth`, `useMembers`, `useDebounce`
- Functions/variables: `camelCase` — `validateEmail`, `parseConfig`, `isActive`
- Types/interfaces: `PascalCase` — `UserProfile`, `AuthState`, `ApiResponse`
- Constants: `SCREAMING_SNAKE_CASE` — `API_BASE_URL`, `MAX_RETRIES`, `DEFAULT_LOCALE`
- Enum-like `as const` objects: `PascalCase` — `Status`, `Role`, `Permission`

**Package naming:**
- Workspace packages: `@myorg/{kebab-case}` — `@myorg/ui`, `@myorg/vitest-config`
- Internal imports match package name: `import { Button } from "@myorg/ui"`

**Props and interfaces:**
- Props interfaces: `{Component}Props` — `ButtonProps`, `CardProps`, `DialogProps`
- Return type interfaces: `{Hook}Return` — `UseAuthReturn`, `UseCounterReturn`
- Never prefix interfaces with `I` — use `UserProfile`, not `IUserProfile`

**Event handlers:**
- Handler props: `on{Event}` — `onClick`, `onSubmit`, `onChange`
- Handler implementations: `handle{Event}` — `handleClick`, `handleSubmit`, `handleChange`

**Why:** Consistency makes code predictable and searchable. When naming conventions are enforced, you can find any component by converting its name to kebab-case for the file, or vice versa. No debates, no inconsistency across contributors.

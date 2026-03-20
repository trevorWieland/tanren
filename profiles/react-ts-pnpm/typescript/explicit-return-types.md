# Explicit Return Types

All exported functions and methods must have explicit return type annotations. Never rely on inference at public API boundaries.

```typescript
// ✓ Good: Explicit return types on exports
export function parseConfig(raw: string): AppConfig {
  return AppConfigSchema.parse(JSON.parse(raw));
}

export function useAuth(): AuthContext {
  const context = useContext(AuthCtx);
  if (!context) throw new Error("useAuth must be within AuthProvider");
  return context;
}

export function UserProfile({ user }: UserProfileProps): ReactNode {
  return <div>{user.name}</div>;
}

// ✗ Bad: Inferred return types on exports
export function parseConfig(raw: string) {
  return AppConfigSchema.parse(JSON.parse(raw));
}

export function useAuth() {
  const context = useContext(AuthCtx);
  if (!context) throw new Error("useAuth must be within AuthProvider");
  return context;
}
```

**Rules:**
- All `export` functions must have explicit return types — enforced by oxlint rule `typescript/explicit-module-boundary-types`
- Internal (non-exported) functions may rely on inference
- React components must return `ReactNode` explicitly
- Custom hooks must declare their return type explicitly
- Async functions must return `Promise<ExactType>`, never `Promise<any>`

```typescript
// ✓ Good: Async with explicit return
export async function fetchUser(id: string): Promise<User> {
  const response = await fetch(`/api/users/${id}`);
  return UserSchema.parse(await response.json());
}

// ✗ Bad: Async with inferred return
export async function fetchUser(id: string) {
  const response = await fetch(`/api/users/${id}`);
  return UserSchema.parse(await response.json());
}
```

**Internal functions:**
- Private and file-local functions may omit return types
- Callbacks passed to higher-order functions (`.map`, `.filter`) may omit return types
- If inference produces `any` or a union that's too wide, add an explicit type

**Why:** Explicit return types at module boundaries make APIs self-documenting, prevent accidental public API changes, and catch type drift before it reaches consumers. Inference is fine inside a module where the compiler has full context, but exports are contracts.

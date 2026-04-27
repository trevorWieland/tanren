---
kind: standard
name: hook-conventions
category: react
importance: high
applies_to:
  - "**/*.ts"
  - "**/*.tsx"
applies_to_languages:
  - typescript
  - react
applies_to_domains:
  - react
---

# Hook Conventions

Custom hooks must start with `use`. Prefer derived state over syncing state with effects.

```tsx
// ✓ Good: Derived state — no useState, no useEffect
function MemberList({ members }: MemberListProps): ReactNode {
  const activeMembers = members.filter((m) => m.status === "active");
  const memberCount = activeMembers.length;

  return (
    <div>
      <h2>Active Members ({memberCount})</h2>
      <ul>
        {activeMembers.map((m) => (
          <li key={m.id}>{m.name}</li>
        ))}
      </ul>
    </div>
  );
}

// ✗ Bad: Syncing derived state via useEffect
function MemberList({ members }: MemberListProps): ReactNode {
  const [activeMembers, setActiveMembers] = useState<Member[]>([]);
  const [memberCount, setMemberCount] = useState(0);

  useEffect(() => {
    const active = members.filter((m) => m.status === "active");
    setActiveMembers(active);
    setMemberCount(active.length);
  }, [members]);

  return (
    <div>
      <h2>Active Members ({memberCount})</h2>
    </div>
  );
}
```

**Rules:**
- Custom hooks must start with `use` prefix — required so that hook lint rules (oxlint `react/rules-of-hooks`) can reliably detect them
- Never call hooks conditionally or inside loops — enforced by oxlint `react/rules-of-hooks`
- `useEffect` must have an explicit dependency array — enforced by oxlint `react/exhaustive-deps`
- Prefer computed/derived values over `useState` + `useEffect` sync patterns
- If a value can be computed from props or other state, compute it inline — don't store it in state

**Custom hook structure:**
- Co-locate hooks with their feature: `src/features/auth/hooks/use-auth.ts`
- Shared hooks go in a hooks package: `packages/hooks/src/use-debounce.ts`
- One hook per file, filename matches hook name in kebab-case
- Always declare the return type explicitly

```typescript
// ✓ Good: Custom hook with explicit return type
interface UseCounterReturn {
  count: number;
  increment: () => void;
  decrement: () => void;
  reset: () => void;
}

function useCounter(initial: number = 0): UseCounterReturn {
  const [count, setCount] = useState(initial);

  const increment = useCallback(() => setCount((c) => c + 1), []);
  const decrement = useCallback(() => setCount((c) => c - 1), []);
  const reset = useCallback(() => setCount(initial), [initial]);

  return { count, increment, decrement, reset };
}
```

**Data fetching:**
- Use TanStack Query (`useQuery`, `useSuspenseQuery`) for all server state
- Never use `useEffect` + `useState` for data fetching
- Prefetch on route transitions with TanStack Router loaders

**Why:** Derived state is simpler, faster, and impossible to desync. Effect-based state syncing introduces an extra render cycle, creates opportunities for stale state, and is the most common source of React bugs. Hooks with explicit return types are self-documenting contracts.

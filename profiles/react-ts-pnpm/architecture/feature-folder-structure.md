---
kind: standard
name: feature-folder-structure
category: architecture
importance: high
applies_to: []
applies_to_languages:
  - typescript
applies_to_domains:
  - architecture
---

# Feature Folder Structure

Organize application code by feature, not by file type. Shared UI belongs in a component library package, not app-local.

```
# ✓ Good: Feature-based organization
apps/web/src/
├── features/
│   ├── auth/
│   │   ├── components/
│   │   │   ├── login-form.tsx
│   │   │   ├── login-form.test.tsx
│   │   │   └── login-form.stories.tsx
│   │   ├── hooks/
│   │   │   ├── use-auth.ts
│   │   │   └── use-auth.test.ts
│   │   ├── utils/
│   │   │   ├── validate-token.ts
│   │   │   └── validate-token.test.ts
│   │   └── types.ts
│   └── dashboard/
│       ├── components/
│       ├── hooks/
│       └── types.ts
├── routes/
│   ├── __root.tsx
│   ├── index.tsx
│   ├── login.tsx
│   └── dashboard.tsx
└── app.tsx
```

```
# ✗ Bad: Flat file-type organization
apps/web/src/
├── components/
│   ├── login-form.tsx
│   ├── dashboard-card.tsx
│   ├── user-avatar.tsx
│   └── ... 50 more files
├── hooks/
│   ├── use-auth.ts
│   ├── use-dashboard.ts
│   └── ... 30 more files
├── utils/
│   ├── validate-token.ts
│   └── ... 20 more files
└── types/
    └── ... everything in one bucket
```

**Rules:**
- Each feature is a self-contained directory under `src/features/{name}/`
- Features contain: `components/`, `hooks/`, `utils/`, and `types.ts`
- Tests and stories are co-located with their source files (not in a separate `__tests__/` tree)
- Shared UI components live in `packages/ui/`, not duplicated across app features
- Route pages live in `src/routes/` using TanStack Router file-based routing

**Feature boundaries:**
- Features do not import from other features directly
- Cross-feature communication goes through shared packages or route-level composition
- If two features share a component, move it to `packages/ui/`

**Route structure:**
- Use TanStack Router's file-based routing in `src/routes/`
- Route files are thin — they compose feature components, handle data loading, and define layout
- Data loading via TanStack Router `loader` functions, backed by TanStack Query

```typescript
// ✓ Good: Thin route that composes features
// src/routes/dashboard.tsx
import { createFileRoute } from "@tanstack/react-router";
import { DashboardView } from "../features/dashboard/components/dashboard-view";

export const Route = createFileRoute("/dashboard")({
  loader: ({ context }) => context.queryClient.ensureQueryData(dashboardQuery()),
  component: DashboardPage,
});

function DashboardPage(): ReactNode {
  return <DashboardView />;
}
```

**Why:** Feature folders keep related code together, making it easy to understand, modify, and delete a feature as a unit. Flat directories become unnavigable past 20-30 files. Co-located tests ensure tests move with the code they test.

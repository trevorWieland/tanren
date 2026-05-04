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

Organize application code by feature, not by file type. The Tanren web app uses
**Next.js App Router** as its routing surface; route segments live under
`apps/web/src/app/`, while reusable feature components live under
`apps/web/src/components/{feature}/`.

```
# ✓ Good: Next.js App Router + feature-grouped components
apps/web/src/
├── app/
│   ├── layout.tsx
│   ├── page.tsx
│   ├── sign-up/
│   │   └── page.tsx              # thin route — renders SignUpForm
│   ├── sign-in/
│   │   └── page.tsx
│   └── invitations/
│       └── [token]/
│           └── page.tsx
├── components/
│   ├── account/
│   │   ├── SignUpForm.tsx
│   │   ├── SignUpForm.stories.tsx
│   │   ├── SignUpForm.test.tsx
│   │   ├── SignInForm.tsx
│   │   ├── SignInForm.stories.tsx
│   │   ├── SignInForm.test.tsx
│   │   ├── AcceptInvitationForm.tsx
│   │   ├── AcceptInvitationForm.stories.tsx
│   │   └── AcceptInvitationForm.test.tsx
│   └── dashboard/
│       ├── DashboardView.tsx
│       └── DashboardView.stories.tsx
├── hooks/
│   └── account/
│       └── use-account-client.ts
├── lib/
│   └── account-client.ts
└── i18n/
    ├── project.inlang/
    ├── messages/
    └── paraglide/                 # generated, gitignored
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
└── utils/
    └── ... everything in one bucket
```

**Rules:**
- Routes live under `apps/web/src/app/{route}/page.tsx` using the Next.js App
  Router. File-based routing is provided by Next.js — do **not** introduce
  TanStack Router.
- Reusable components for a feature live in
  `apps/web/src/components/{feature}/{ComponentName}.tsx`. The project does
  not have a `packages/ui/` workspace; do not invent one. Components remain
  app-local until a second consumer materializes.
- Co-locate stories and tests with the component:
  `SignUpForm.tsx` + `SignUpForm.stories.tsx` + `SignUpForm.test.tsx`.
- Each route page is either (a) a thin shell that imports a feature component
  from `src/components/{feature}/`, or (b) a small page that renders directly
  when there is no reuse. Prefer (a) for any form/component reused in
  Storybook stories or BDD step coverage.
- Server-state — async fetching, caching, mutations — uses **TanStack Query**
  when applicable. TanStack Query works fine alongside the App Router (use it
  in client components or via a query-client provider in `app/layout.tsx`).

**Component-extraction trigger:**
- Inline JSX in a route page is acceptable for trivial pages.
- The moment a page needs Storybook stories, BDD play coverage, or unit tests,
  promote it to `apps/web/src/components/{feature}/{ComponentName}.tsx` so
  the artifact has a stable import path.

**Route example:**

```typescript
// ✓ Good: Thin App Router page composes a feature component
// apps/web/src/app/sign-up/page.tsx
import type { ReactNode } from "react";
import { SignUpForm } from "@/components/account/SignUpForm";

export default function SignUpPage(): ReactNode {
  return <SignUpForm />;
}
```

```typescript
// ✓ Acceptable: tiny page rendered directly when there is no reuse
// apps/web/src/app/legal/terms/page.tsx
import type { ReactNode } from "react";
import * as m from "@/i18n/paraglide/messages";

export default function TermsPage(): ReactNode {
  return (
    <main>
      <h1>{m.legalTermsTitle()}</h1>
      <p>{m.legalTermsBody()}</p>
    </main>
  );
}
```

**Feature boundaries:**
- A component under `src/components/{feature}/` does not import from another
  feature's components directory. Cross-feature reuse is a signal to lift the
  shared piece up (to `src/components/shared/` or, eventually, a new package).
- Hooks specific to a feature live in `src/hooks/{feature}/` and are imported
  by that feature's components.
- Route pages may import from any feature directory — pages compose features.

**Why:** Grouping by feature keeps related code together, making it easy to
understand, modify, and delete a feature as a unit. Anchoring routes to the
Next.js App Router avoids the meta-conflict of two routing systems competing
for the same source tree, and matches what the architecture record specifies
the web surface uses. Co-located stories and tests guarantee the visual
contract and behavior coverage move with the component.

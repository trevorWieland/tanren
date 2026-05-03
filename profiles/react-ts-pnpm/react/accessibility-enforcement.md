---
kind: standard
name: accessibility-enforcement
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

# Accessibility Enforcement

Accessibility is not optional. All interactive elements must be
keyboard-navigable and screen-reader-compatible. Every form input is paired
with an explicit `<label htmlFor="..."/>`, every error region is announced via
ARIA, and every story runs an axe-core audit.

```tsx
// ✓ Good: Semantic HTML + explicit htmlFor/id pairing + ARIA error region
import type { ReactNode } from "react";
import * as m from "@/i18n/paraglide/messages";

function SignUpForm({ error }: { error?: string }): ReactNode {
  return (
    <form aria-label={m.signUpFormLabel()} onSubmit={handleSubmit}>
      <label htmlFor="sign-up-email">{m.signUpEmailLabel()}</label>
      <input
        id="sign-up-email"
        name="email"
        type="email"
        required
        aria-describedby={error ? "sign-up-email-error" : undefined}
        aria-invalid={error ? true : undefined}
      />
      {error !== undefined && (
        <p id="sign-up-email-error" role="alert" aria-live="polite">
          {error}
        </p>
      )}
      <button type="submit">{m.signUpSubmit()}</button>
    </form>
  );
}

// ✗ Bad: Wrapping label, no error association, div onClick "button"
function SignUpForm(): ReactNode {
  return (
    <div>
      <label>
        Email
        <input type="email" />
      </label>
      <div onClick={handleSubmit}>Submit</div>
    </div>
  );
}
```

**Rules:**
- Use semantic HTML elements: `<button>` for actions, `<a>` for navigation,
  `<nav>`, `<main>`, `<header>`, `<section>`, `<form>`.
- Never use `<div>` or `<span>` with `onClick` as a button substitute.
- Every form input MUST have `id="..."` matched by a separate
  `<label htmlFor="...">`. **Wrapping a label around an input is NOT
  sufficient** — the profile is explicit, and oxlint
  `jsx-a11y/label-has-associated-control` enforces this.
- Forms get `aria-label={m.someFormLabel()}` (paraglide message) so screen
  readers announce a meaningful summary.
- Error regions are rendered as
  `<p id="..." role="alert" aria-live="polite">`. The offending input gets
  `aria-describedby="..."` pointing to that id and `aria-invalid="true"`
  while the error is active. Both attributes are dropped (or set to
  `undefined`) when the error clears — never left on a clean field.
- All `<img>` elements must have `alt` text (empty `alt=""` for decorative
  images).
- All icons must have `aria-label` (if meaningful) or `aria-hidden="true"`
  (if decorative).
- All interactive elements must be reachable via keyboard (`Tab`, `Enter`,
  `Space`, `Escape`).
- Never remove focus outlines (`outline-none`) without providing a visible
  alternative (`focus-visible:ring-*`).

**Enforcement layers:**

1. **Lint time — oxlint `jsx-a11y` plugin.** Runs every PR. Catches static
   JSX issues (missing alt, invalid ARIA, non-interactive roles on
   interactive elements, missing `htmlFor`/`id` pairing, missing keyboard
   handlers on `onClick`). The full per-rule list is canonical in
   `global/strict-linting-gate.md`.
2. **Component tests — axe-core via Storybook 9 `@storybook/addon-a11y`.**
   The addon runs an axe-core audit over the rendered DOM of every story.
   Each component story includes a `play` function that exercises the
   rendered DOM (focus, type, click); axe runs after the play function
   completes.
3. **CI — Storybook test-runner.** The Storybook test-runner mode is wired
   into `just ci` and runs every story. axe violations fail the build.

```typescript
// ✓ Good: axe-core check + interaction in a Storybook 9 play function
import { expect, userEvent, within } from "@storybook/test";
import type { Meta, StoryObj } from "@storybook/nextjs-vite";
import { SignUpForm } from "./SignUpForm";

const meta: Meta<typeof SignUpForm> = {
  component: SignUpForm,
  parameters: { a11y: { test: "error" } },
};
export default meta;

export const Default: StoryObj<typeof SignUpForm> = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await userEvent.type(canvas.getByLabelText(/email/i), "user@example.com");
    await expect(canvas.getByRole("button", { name: /sign up/i })).toBeEnabled();
    // a11y check runs automatically via @storybook/addon-a11y
  },
};
```

**Why:** ~15-20% of users rely on assistive technology. A11y bugs are
expensive to retrofit and can create legal liability. Enforcing a11y at
three layers — lint, component test (axe per story), and CI (Storybook
test-runner) — catches issues before review.

---
kind: standard
name: component-testing-via-storybook
category: testing
importance: high
applies_to:
  - "**/*test*"
  - "**/*spec*"
  - "tests/**"
applies_to_languages:
  - typescript
applies_to_domains:
  - testing
---

# Storybook as Visual Support, Not Behavior Authority

Storybook is the visual contract for every component the web app ships. The
Tanren web surface uses **Storybook 9** with the
**`@storybook/nextjs-vite`** framework, plus the **`@storybook/addon-vitest`**
and **`@storybook/addon-a11y`** addons.

Storybook stories document visual states and run real-browser component
tests with axe-core a11y audits. Behavior proof — "the user can sign up via
the web surface" — remains in the BDD scenarios under
`tests/bdd/features/`, executed via `playwright-bdd`.

```tsx
// ✓ Good: Story per state, with a play function exercising the DOM
import type { Meta, StoryObj } from "@storybook/nextjs-vite";
import { expect, userEvent, within } from "@storybook/test";
import { SignUpForm } from "./SignUpForm";

const meta: Meta<typeof SignUpForm> = {
  component: SignUpForm,
  parameters: { a11y: { test: "error" } },
};
export default meta;

type Story = StoryObj<typeof SignUpForm>;

export const Default: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("button", { name: /sign up/i })).toBeEnabled();
  },
};

export const Submitting: Story = {
  args: { state: "submitting" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("button", { name: /sign up/i })).toBeDisabled();
  },
};

export const WithError: Story = {
  args: { state: "error", error: "Email is already in use" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("alert")).toHaveTextContent(/already in use/i);
  },
};

export const Success: Story = {
  args: { state: "success" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("status")).toBeVisible();
  },
};
```

```gherkin
# Behavior proof lives in BDD, not Storybook
@B-0043 @web @falsification
Scenario: Sign-up rejects an already-registered email
  Given an account exists with email "user@example.com"
  When the user submits the sign-up form with email "user@example.com"
  Then the form shows the error "email is already in use"
  And no new account is created
```

## Framework + addons

- **Framework: `@storybook/nextjs-vite ^9`** — modern Vite-based framework.
  Do **NOT** use the legacy webpack-based `@storybook/nextjs`; it is slower
  and incompatible with the Vitest addon's component-test runner.
- **`@storybook/addon-vitest`** — transforms each story into a real-browser
  Vitest component test. The play function runs in the browser; assertions
  are real DOM assertions, not jsdom approximations. CI runs these as part
  of the test gate.
- **`@storybook/addon-a11y`** — runs axe-core on the rendered DOM after the
  play function completes. Configured at `error` severity so violations fail
  the build (see `react/accessibility-enforcement.md`).

## State coverage rule

Every component MUST ship at minimum these four story states, each with a
`play` function exercising the rendered DOM:

| Story | Args | Play asserts |
|---|---|---|
| `Default` | initial / empty | base render is correct, key roles are present |
| `Submitting` | `state: "submitting"` | submit control is disabled, busy indicator visible |
| `WithError` | `state: "error", error: "..."` | error region is announced (`role="alert"`), input is `aria-invalid` |
| `Success` | `state: "success"` | success region renders, follow-up affordance visible |

axe-core runs against each of the four stories. A component cannot ship
unless all four pass both the play assertions AND the a11y audit.

## Boundary rule

- Storybook is **not** the source of truth for shipped behavior. Behavior
  claims require BDD scenarios under `tests/bdd/features/`.
- Stories are visual contracts: "this is what the component looks and feels
  like in state X." They prove rendering correctness, accessible structure,
  and a11y conformance — not whether the surface, end-to-end, fulfils the
  behavior the user invoked.
- Keep stories and `@web` BDD scenarios aligned for major UI states so a
  designer or PM can map a Gherkin step to a story screenshot.

**Why:** Storybook 9 + Vitest addon makes component tests faithful (real
browser DOM, real axe audit) and fast enough to run on every PR. Anchoring
the framework choice to the Vite-based `@storybook/nextjs-vite` avoids the
legacy webpack toolchain that no longer composes with the modern test
runner. Behavior proof staying in BDD keeps the visual contract layer honest
about what it actually proves.

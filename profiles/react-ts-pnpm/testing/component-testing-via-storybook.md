# Component Testing via Storybook

Storybook stories are the source of truth for component rendering tests. Never use `render()` from Testing Library in unit tests for components.

```tsx
// ✓ Good: Storybook story with play function
import type { Meta, StoryObj } from "@storybook/react";
import { expect, fn, userEvent, within } from "@storybook/test";
import { Button } from "./button";

const meta: Meta<typeof Button> = {
  component: Button,
};
export default meta;

type Story = StoryObj<typeof Button>;

export const Default: Story = {
  args: {
    children: "Click me",
    variant: "default",
  },
};

export const ClickHandler: Story = {
  args: {
    children: "Submit",
    onClick: fn(),
  },
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement);
    const button = canvas.getByRole("button", { name: "Submit" });

    await userEvent.click(button);
    await expect(args.onClick).toHaveBeenCalledOnce();
  },
};

export const Disabled: Story = {
  args: {
    children: "Disabled",
    disabled: true,
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const button = canvas.getByRole("button");

    await expect(button).toBeDisabled();
  },
};
```

```tsx
// ✗ Bad: Unit test using render() for component output
import { render, screen } from "@testing-library/react";
import { Button } from "./button";

test("renders button", () => {
  render(<Button>Click me</Button>);
  expect(screen.getByRole("button")).toBeInTheDocument();
});
```

**Rules:**
- One story per meaningful component state (default, loading, error, disabled, each variant)
- Use `play` functions for interaction testing — never use `fireEvent`, always use `userEvent`
- Test accessibility in play functions via Storybook's a11y addon (powered by axe-core)
- Storybook Vitest addon runs all stories as tests in CI
- Stories serve dual purpose: documentation and tests
- Query elements by role (`getByRole`), label, or text — never by test ID or CSS selector

**What goes in Storybook vs unit tests:**
- **Storybook:** Component rendering, visual states, user interactions, accessibility checks
- **Unit tests:** Pure logic — hooks, utilities, data transforms, state machines. Anything that doesn't need a DOM

**Story organization:**
- Co-locate `*.stories.tsx` next to the component file
- Group stories by component variant and interaction
- Tag automation-only stories with `!autodocs` and `!dev`

**CI integration:**
- Use `@storybook/experimental-addon-test` (Vitest addon) to run stories as Vitest tests
- Stories run as part of the unit + component test gate on every PR
- Zero story failures allowed before merge

**Why:** Storybook provides a real rendering environment with visual feedback. Testing component rendering in unit tests with jsdom misses CSS, layout, and visual regressions. Stories as tests eliminate duplicate test effort — one artifact serves documentation, visual review, and automated testing.

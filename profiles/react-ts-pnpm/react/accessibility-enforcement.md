# Accessibility Enforcement

Accessibility is not optional. All interactive elements must be keyboard-navigable and screen-reader-compatible.

```tsx
// ✓ Good: Semantic HTML with proper ARIA
function SearchBar({ onSearch }: SearchBarProps): ReactNode {
  const { t } = useTranslation();

  return (
    <form role="search" onSubmit={handleSubmit}>
      <label htmlFor="search-input">{t("search.label")}</label>
      <input
        id="search-input"
        type="search"
        aria-describedby="search-hint"
        placeholder={t("search.placeholder")}
      />
      <p id="search-hint">{t("search.hint")}</p>
      <button type="submit">
        <SearchIcon aria-hidden="true" />
        <span>{t("search.submit")}</span>
      </button>
    </form>
  );
}

// ✗ Bad: Div soup with no semantics
function SearchBar({ onSearch }: SearchBarProps): ReactNode {
  return (
    <div>
      <div>
        <input placeholder="Search..." />
      </div>
      <div onClick={handleClick}>
        <SearchIcon />
      </div>
    </div>
  );
}
```

**Rules:**
- Use semantic HTML elements: `<button>` for actions, `<a>` for navigation, `<nav>`, `<main>`, `<header>`, `<section>`, `<form>`
- Never use `<div>` or `<span>` with `onClick` as a button substitute
- All `<img>` elements must have `alt` text (empty `alt=""` for decorative images)
- All icons must have `aria-label` (if meaningful) or `aria-hidden="true"` (if decorative)
- All form inputs must have an associated `<label>` element via `htmlFor`/`id` pairing
- All interactive elements must be reachable via keyboard (`Tab`, `Enter`, `Space`, `Escape`)
- Never remove focus outlines (`outline-none`) without providing a visible alternative (`focus-visible:ring-*`)

**Enforcement layers:**
1. **Lint time** — oxlint with `jsx-a11y` plugin catches static JSX issues (missing alt, invalid ARIA, non-interactive roles on interactive elements)
2. **Component tests** — axe-core checks in Storybook play functions catch rendered DOM issues
3. **CI** — Both lint and Storybook tests run on every PR. Zero a11y violations allowed

```typescript
// ✓ Good: axe-core check in Storybook play function
import { expect, userEvent, within } from "@storybook/test";

export const Default: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    // Interaction test
    await userEvent.click(canvas.getByRole("button"));
    // a11y check runs automatically via Storybook a11y addon
  },
};
```

**Radix UI baseline:**
- Use Radix UI primitives for complex interactive patterns (dialogs, dropdowns, tabs, tooltips)
- Radix provides keyboard navigation, focus trapping, and ARIA attributes out of the box
- Never build custom implementations of patterns Radix already provides

**Why:** 15-20% of users rely on assistive technology. Accessibility bugs are expensive to retrofit and can create legal liability. Enforcing a11y at lint, test, and CI layers catches issues before they reach production.

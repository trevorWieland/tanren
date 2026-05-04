---
kind: standard
name: styling-and-design-tokens
category: architecture
importance: high
applies_to: []
applies_to_languages:
  - typescript
applies_to_domains:
  - architecture
---

# Styling and Design Tokens

Tanren's web surface styles via **Tailwind v4** with a **CSS-first `@theme`
directive**. Design tokens are CSS variables in the oklch color space.
Inline `style={{ ... }}` and hardcoded color hex codes are forbidden.

```css
/* ✓ Good: CSS-first theme tokens in apps/web/src/app/globals.css */
@import "tailwindcss";

@theme {
  /* Surface tokens */
  --color-bg-canvas: oklch(0.99 0.005 264);
  --color-bg-surface: oklch(0.97 0.005 264);
  --color-bg-elevated: oklch(0.95 0.008 264);

  /* Foreground tokens */
  --color-fg-default: oklch(0.21 0.01 264);
  --color-fg-muted: oklch(0.45 0.01 264);
  --color-fg-inverse: oklch(0.99 0.005 264);

  /* Intent tokens */
  --color-accent: oklch(0.62 0.17 252);
  --color-accent-hover: oklch(0.55 0.18 252);
  --color-error: oklch(0.55 0.21 27);
  --color-success: oklch(0.62 0.16 152);

  /* Type ramp */
  --font-sans: ui-sans-serif, system-ui, sans-serif;

  /* Spacing scale (rem-based) extends Tailwind's defaults */
}
```

```tsx
// ✓ Good: Tailwind utilities reading the theme variables
import type { ReactNode } from "react";

export function Card({ children }: { children: ReactNode }): ReactNode {
  return (
    <article className="bg-[--color-bg-surface] text-[--color-fg-default] rounded-md p-4">
      {children}
    </article>
  );
}

// ✗ Bad: inline style with a literal color
export function Card({ children }: { children: ReactNode }): ReactNode {
  return (
    <article style={{ background: "#deadbe", color: "#111" }}>
      {children}
    </article>
  );
}
```

## Engine + configuration

- **Tailwind v4** with the `@tailwindcss/postcss` plugin. The CSS engine is
  the v4 Oxide rewrite — significantly faster than v3 and JIT-only.
- **Theme is CSS-first.** All design tokens live in an `@theme { ... }`
  block inside `apps/web/src/app/globals.css`. **No `tailwind.config.ts` is
  required for theme.** A config file is only created when a Tailwind plugin
  needs to be wired in (e.g. `@tailwindcss/typography`); even then, theme
  tokens stay in CSS.
- The `@theme` block is the **single source of truth** for tokens. Every
  color, spacing constant, font, and radius the app uses is named there.
  Components read tokens through Tailwind utility classes; no component
  hardcodes a value.

## Color space

Tokens use the **oklch** color space (Tailwind v4's default). oklch is
perceptually uniform — equal numeric deltas in lightness and chroma look
roughly equal to a human, which legacy `rgb`/`hsl` cannot guarantee. This
matters for hover/active states, dark-mode mirrors, and accessible contrast
math.

## Forbidden patterns

- **No inline `style={{ ... }}`.** Enforced by oxlint
  `react/forbid-dom-props: ["style"]` (see `global/strict-linting-gate.md`).
  The narrow allowlist for the rule covers the small set of design-token
  utility wrappers that genuinely need style injection — components do not.
- **No hardcoded color hex codes** (`#fff`, `#deadbeef`, `rgb(...)`,
  `hsl(...)`) anywhere in JSX or component CSS. The only place colors live
  is the `@theme` block in `globals.css`.
- **No `style.setProperty(...)`** to push runtime colors. If a runtime token
  is needed, define it in `@theme` and toggle a class.

```tsx
// ✗ Bad: hardcoded color in JSX
<button className="bg-[#0070f3] text-[#ffffff]">Save</button>

// ✓ Good: token-backed utility
<button className="bg-[--color-accent] text-[--color-fg-inverse]">Save</button>
```

## Class composition

- **Canonical pattern.** Tailwind utilities reading the theme variables:
  `className="bg-[--color-bg-canvas] text-[--color-fg-default]"`. This works
  out of the box with v4.
- **Named-utility pattern.** Once a token is mapped through Tailwind's
  utility generation (e.g. via the `@theme` block's name registration),
  components can use shorter classes:
  `className="bg-canvas text-default"`. Either pattern is acceptable; pick
  one per component and stay consistent within it.
- **Variant composition.** For complex variant matrices (size × intent ×
  state), use **`class-variance-authority` (cva)** layered on top of
  Tailwind utilities. cva keeps variant logic typed and testable, and
  composes cleanly with the token-based classes.

```tsx
// ✓ Good: cva for variant composition over token-backed utilities
import { cva, type VariantProps } from "class-variance-authority";

const button = cva(
  "rounded-md px-3 py-2 text-sm font-medium focus-visible:ring-2",
  {
    variants: {
      intent: {
        primary: "bg-[--color-accent] text-[--color-fg-inverse] hover:bg-[--color-accent-hover]",
        danger:  "bg-[--color-error] text-[--color-fg-inverse]",
        ghost:   "bg-transparent text-[--color-fg-default] hover:bg-[--color-bg-elevated]",
      },
      size: {
        sm: "px-2 py-1 text-xs",
        md: "px-3 py-2 text-sm",
      },
    },
    defaultVariants: { intent: "primary", size: "md" },
  },
);

type ButtonProps = VariantProps<typeof button> & React.ButtonHTMLAttributes<HTMLButtonElement>;
```

**Why:** A single source of truth for design tokens — CSS variables in the
`@theme` block — makes theming, dark mode, and a11y contrast tuning
mechanical. Banning inline styles and hardcoded colors prevents one-off
visual drift across surfaces. oklch keeps perceived lightness uniform across
the whole palette, which legacy color spaces cannot. Tailwind v4's
CSS-first config removes the JS-side `tailwind.config.ts` dance for the
common case, and cva keeps variant explosion under control without leaking
into ad-hoc style props.

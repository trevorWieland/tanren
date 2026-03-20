# Component Composition

Use headless primitives for behavior, CVA for variants, and Tailwind for styling. Never reinvent accessible interactive patterns.

```tsx
// ✓ Good: CVA variants with Radix Slot for polymorphism
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@myorg/utils";

const buttonVariants = cva(
  "inline-flex items-center justify-center rounded-md font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 disabled:pointer-events-none disabled:opacity-50",
  {
    variants: {
      variant: {
        default: "bg-primary text-primary-foreground hover:bg-primary/90",
        destructive: "bg-destructive text-destructive-foreground hover:bg-destructive/90",
        outline: "border border-input bg-background hover:bg-accent",
        ghost: "hover:bg-accent hover:text-accent-foreground",
      },
      size: {
        sm: "h-8 px-3 text-sm",
        md: "h-10 px-4 text-sm",
        lg: "h-12 px-6 text-base",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "md",
    },
  },
);

interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean;
}

function Button({ className, variant, size, asChild, ...props }: ButtonProps): ReactNode {
  const Comp = asChild ? Slot : "button";
  return <Comp className={cn(buttonVariants({ variant, size }), className)} {...props} />;
}
```

```tsx
// ✗ Bad: Inline conditional classNames without a variant system
function Button({ variant, size, className, ...props }: ButtonProps): ReactNode {
  return (
    <button
      className={`btn ${variant === "primary" ? "bg-blue-500" : ""} ${
        size === "lg" ? "text-lg px-6" : "text-sm px-3"
      } ${className}`}
      {...props}
    />
  );
}
```

**Rules:**
- Use **Radix UI** (or Ariakit) for all interactive primitives — dialogs, dropdowns, tooltips, popovers, tabs. Never build custom accessible widgets from scratch
- Use **CVA** (class-variance-authority) for all variant-based component styling
- Use **`cn()`** utility (clsx + tailwind-merge) for className composition — never concatenate className strings manually
- Use **Tailwind CSS** for all styling — no CSS modules, no styled-components, no CSS-in-JS
- Use the `asChild` pattern (Radix Slot) for polymorphic components

**Compound components:**
Use compound patterns for components with shared state:

```tsx
// ✓ Good: Compound component pattern
<Tabs defaultValue="overview">
  <TabsList>
    <TabsTrigger value="overview">{t("tabs.overview")}</TabsTrigger>
    <TabsTrigger value="settings">{t("tabs.settings")}</TabsTrigger>
  </TabsList>
  <TabsContent value="overview">
    <OverviewPanel />
  </TabsContent>
  <TabsContent value="settings">
    <SettingsPanel />
  </TabsContent>
</Tabs>
```

**Styling rules:**
- Design tokens (colors, spacing, fonts) live in `tailwind.config.ts`
- Never use arbitrary Tailwind values (`bg-[#1a2b3c]`) — define tokens in the config
- Sort Tailwind classes with oxfmt's built-in class sorting

**Why:** Headless primitives handle keyboard navigation, focus management, and ARIA attributes — reimplementing these is error-prone and wasteful. CVA makes variant logic explicit and composable. Tailwind provides a constrained design system that eliminates naming debates and dead CSS.

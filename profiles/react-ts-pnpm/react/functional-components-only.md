---
kind: standard
name: functional-components-only
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

# Functional Components Only

No class components. Use function declarations for all React components.

```tsx
// ✓ Good: Function declaration with typed props
interface UserProfileProps {
  name: string;
  email: string;
  role: Role;
}

function UserProfile({ name, email, role }: UserProfileProps): ReactNode {
  return (
    <section>
      <h2>{name}</h2>
      <p>{email}</p>
      <RoleBadge role={role} />
    </section>
  );
}

// ✗ Bad: Arrow function expression
const UserProfile = ({ name, email, role }: UserProfileProps): ReactNode => {
  return (
    <section>
      <h2>{name}</h2>
      <p>{email}</p>
      <RoleBadge role={role} />
    </section>
  );
};

// ✗ Bad: Class component
class UserProfile extends Component<UserProfileProps> {
  render() {
    return (
      <section>
        <h2>{this.props.name}</h2>
      </section>
    );
  }
}
```

**Rules:**
- Always use `function` declarations for components — never `const Foo = () => ...`
- Props must be typed via a separate `interface` (named `{Component}Props`) or inline destructuring for trivial cases
- Return type must be `ReactNode` explicitly
- One component per file — the filename matches the component in kebab-case (`user-profile.tsx` exports `UserProfile`)

**Props typing:**
- Use `interface` for props, not `type` — interfaces are extendable and produce better error messages
- Destructure props in the function signature, not in the body
- Use `children: ReactNode` for wrapper components, never `children: React.ReactNode` (import `ReactNode` directly)

```tsx
// ✓ Good: Props interface with destructured params
import type { ReactNode } from "react";

interface CardProps {
  title: string;
  children: ReactNode;
}

function Card({ title, children }: CardProps): ReactNode {
  return (
    <div>
      <h3>{title}</h3>
      {children}
    </div>
  );
}
```

**Forwarding refs:**
- Use the `ref` prop directly (React 19+) — no `forwardRef` wrapper needed
- If supporting React 18, use `forwardRef` with explicit generic typing

**Why:** Function declarations are hoisted, making component ordering in a file irrelevant. A single component syntax across the codebase eliminates style debates and makes grep/search predictable. Class components are a legacy API with no advantages over functions with hooks.

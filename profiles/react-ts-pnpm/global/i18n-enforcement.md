---
kind: standard
name: i18n-enforcement
category: global
importance: high
applies_to: []
applies_to_languages:
  - typescript
applies_to_domains: []
---

# i18n Enforcement

No literal strings in JSX. All user-facing text must go through the translation system.

```tsx
// ✓ Good: Translation keys via useTranslation
import { useTranslation } from "react-i18next";

function WelcomeBanner({ user }: WelcomeBannerProps): ReactNode {
  const { t } = useTranslation();

  return (
    <section>
      <h1>{t("welcome.title")}</h1>
      <p>{t("welcome.greeting", { name: user.name })}</p>
      <button type="button">{t("welcome.cta")}</button>
    </section>
  );
}

// ✗ Bad: Hardcoded strings in JSX
function WelcomeBanner({ user }: WelcomeBannerProps): ReactNode {
  return (
    <section>
      <h1>Welcome</h1>
      <p>Hello, {user.name}!</p>
      <button type="button">Get Started</button>
    </section>
  );
}
```

**Rules:**
- All user-visible text must use `t()` from `useTranslation()` — enforced by linter (`no-literal-string` rule)
- Translation keys are namespaced: `{feature}.{context}.{element}` — e.g., `auth.login.submit`
- Default language is English — keys resolve to English strings as baseline
- Interpolation for dynamic values: `t("greeting", { name })` — never concatenate translated strings

**Locale file structure:**
```
packages/i18n/
├── locales/
│   ├── en/
│   │   ├── common.json
│   │   ├── auth.json
│   │   └── dashboard.json
│   └── es/
│       ├── common.json
│       ├── auth.json
│       └── dashboard.json
└── index.ts
```

**CI enforcement:**
- Linter flags literal strings in JSX — zero violations allowed
- Locale sync check verifies all languages have the same keys
- Key extraction script scans source for `t()` calls and reports missing translations
- All three checks run on every PR

**Exceptions:**
- Aria attributes that are not user-visible (`role`, `type`, `data-*`) are exempt
- CSS class names, HTML attributes, and technical strings are exempt
- Component library internals that receive translated strings via props are exempt

**Why:** Hardcoded strings make localization impossible without a full codebase grep. Lint-time enforcement catches untranslated strings before they reach production. A consistent translation key structure makes it easy for translators to work without reading code.

---
schema: tanren.design_surface_adapter.v0
surface_kind: responsive_gui
status: draft
owner_command: define-design-system
updated_at: 2026-05-07
---

# Web Adapter

The web adapter projects shared design-system records into the responsive GUI
surface.

## Projection

- Tokens map to CSS variables in the Tailwind v4 `@theme` block.
- Reusable component variants should cite `patterns.yml` IDs in Storybook
  story names or story metadata when the pattern is behavior-relevant.
- User-visible copy flows through Paraglide.
- Focus and keyboard paths follow `docs/experience/interaction-models.md`.

## Required Evidence

- Playwright BDD for behavior proof.
- Storybook component states for reusable patterns.
- Responsive screenshots for phone, low-power-laptop, and full-laptop reach.
- Accessibility scan for acceptance-critical views.

## Drift Signals

- Hardcoded colors, spacing, or copy where a token or vocabulary term exists.
- Local form, validation, status, or destructive-action patterns that do not
  reference a registered design pattern.
- Screenshots that cannot be traced to the behavior, surface, and pattern IDs
  they are meant to prove.

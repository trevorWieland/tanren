---
schema: tanren.design_accessibility.v0
status: draft
owner_command: define-design-system
updated_at: 2026-05-07
---

# Accessibility

Accessibility requirements apply to every surface, not only the web UI.

## Cross-Surface Rules

- Do not rely on color alone. Pair color with text, symbol, shape, code, or
  structured status.
- Preserve keyboard or command-only completion for every human-facing workflow.
- Provide clear focus, selection, or current-item state on interactive
  surfaces.
- Keep copy direct and recoverable for validation, permission, unavailable,
  stale, and destructive states.
- Use machine-readable error codes for machine-facing surfaces.
- Make long-running work visible or explicitly bounded.
- Record reduced-motion, timing-tolerance, remapping, caption, or transcript
  needs when a surface kind makes them relevant.

## Proof Expectations

- Web and mobile surfaces need accessibility scans plus keyboard-path proof for
  complex workflows.
- CLI surfaces need transcripts that remain understandable without color.
- TUI surfaces need keyboard navigation and resize proof.
- API and MCP surfaces need machine-readable error envelope proof.
- Gameplay surfaces need replay or engine-native proof for timing-sensitive
  interactions and accessibility options.

## Pending Decisions

- `contrast-budget-pending`: final visual contrast budgets after the web visual
  system stabilizes.
- `motion-policy-pending`: reduced-motion and animation rules for real-time or
  game-like surfaces.
- `locale-policy-pending`: how many locales must be proven before a behavior is
  considered accepted on a localized surface.

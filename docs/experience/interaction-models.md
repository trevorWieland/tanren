---
schema: tanren.experience_interaction_models.v0
status: draft
owner_command: design-experience
updated_at: 2026-05-07
---

# Surface Interaction Models

Surface-shared interaction rules (not per-behavior). Behaviors inherit the
defaults below unless `flows.md` records an explicit deviation. Authoritative
sources for each rule are cited so future audits can detect drift between
this projection and accepted architecture. Reusable interaction decisions that
should guide implementation live in
`docs/experience/design-system/patterns.yml`; this file describes how those
patterns behave on each surface.

## `web`

- **Stack** (per `docs/architecture/technology.md` and R-0001 sub-PR 1):
  Next.js App Router (`apps/web/`), Tailwind v4 design tokens, Storybook 9
  for component states, Paraglide for i18n strings, Playwright BDD for proof.
- **Auth** (per `docs/architecture/subsystems/identity-policy.md`): cookie
  session via `tower-sessions`; argon2id password hashing; 32-byte CSPRNG
  session tokens.
- **Focus model**: `Tab` cycles in document order; visible focus outline on
  every focusable element; `Esc` closes modals/menus and restores prior
  focus.
- **Keyboard map**: `/` focuses the global command surface (per
  `cross-interface` behaviors); `?` opens help; standard browser shortcuts
  preserved.
- **Responsive breakpoints**: phone, low-power-laptop, full-laptop (per
  `surfaces.yml` `web.devices`); behaviors with `phone` reach must validate
  in the phone screenshot.
- **Loading / empty / stale**: progress affordance on any operation > 250ms
  (latency-budget-pending); explicit empty state copy; staleness cue with
  refresh affordance.
- **Copy tone**: direct, action-first, no jargon by default; reference
  product personas for voice (`solo-builder`, `team-builder`, `observer`,
  `operator`).
- **i18n**: all user-visible strings flow through Paraglide; English is the
  default; additional locales pending product decision.

## `api`

- **Stack**: Axum + utoipa-derive OpenAPI (per `technology.md`); JSON
  bodies; SSE for event-stream outputs.
- **Auth** (per `interfaces.md`): bearer tokens for non-browser callers;
  cookie session for browser-origin callers.
- **Error envelope** (per `interfaces.md` "validation_failed"): canonical
  shape `{ "code": "<machine_code>", "message": "<safe>", "details": [...] }`
  for 400; stable codes for `permission_denied` (403), `unavailable` (503),
  `not_found` (404).
- **Idempotency**: state-mutating endpoints accept `Idempotency-Key`
  header; replays return prior result without re-applying.
- **Pagination**: opaque cursor in response; `next_cursor: null` on last
  page; cursor stable across invalidations or returns explicit `stale`
  code.
- **OpenAPI**: schema derived from request/response types; hand-written
  `/openapi.json` allowed only for the F-0001 stub.

## `mcp`

- **Stack**: `rmcp` over stdio + HTTP per F-0002; tool registry empty in
  F-0001 scaffold.
- **Auth**: bearer for HTTP transport; stdio inherits caller process trust.
- **Tool-call shape**: tool names follow `tanren.<area>.<verb>`; arguments
  validated against tool input schema; results return structured payload.
- **Permission boundary**: tools enumerate visibility per persona;
  `permission_denied` returned as a structured error, never as a missing
  tool.
- **Audit attribution**: every tool call logs caller identity, tool ID,
  argument digest, and result code.
- **Error envelope**: same code taxonomy as `api`, returned as structured
  error rather than HTTP status.

## `cli`

- **Stack** (per F-0001 evidence): clap-based grammar; `tanren-cli`
  binary; subcommands `--version`, `health`, `migrate up` shipped in
  foundation.
- **Flag conventions**: long-form (`--project`) preferred; short-form
  reserved for the canonical 3–4 most common flags per command;
  `--json` toggles machine-readable mode globally.
- **Output modes**: text (default, locale-aware) on stdout; JSON envelope
  on stdout when `--json` is set; diagnostics on stderr; no mixed-stream
  output when `--json` is active.
- **Exit-code taxonomy**: 0 success; 1 generic failure; 2 validation; 3
  permission; 4 unavailable; reserved 64–78 per `sysexits.h` where the
  semantic matches.
- **Stdin contract**: only commands that explicitly opt in consume stdin;
  default is "stdin not read".
- **Golden-transcript stability**: text mode is stable enough for
  golden-transcript proof when locale, timezone, and project state are
  fixed; floats and timestamps are quantized.

## `tui`

- **Stack** (per F-0001): Ratatui + crossterm; `tanren-tui` binary; empty
  shell shipped in foundation.
- **Focus model**: `Tab`/`Shift-Tab` cycles focus; modal screens trap
  focus until dismissed.
- **Keyboard map**: `?` opens contextual help; `q` or `Esc` exits the
  current screen; `:` opens command palette (mirrors CLI grammar).
- **Resize behavior**: layout reflows on `terminal-resize`; minimum
  supported size 80x24; smaller terminals show a "resize required"
  screen.
- **Status bar**: persistent bottom row showing current project, persona,
  and last command result.
- **Color signaling**: never color-only; symbols and text accompany every
  state change so monochrome terminals stay legible.
- **PTY-stability**: screen rendering is deterministic for fixed input
  + fixed terminal size, supporting `pty_session` proof.

## Cross-surface invariants

- Surfaces share the **error code taxonomy** (`validation_failed`,
  `permission_denied`, `unavailable`, `not_found`, `stale`) so behavior
  scenarios can declare expected codes once and prove them on every
  declared surface.
- Surfaces share the **persona model** from `docs/product/personas.md`;
  surface-specific personas are not allowed.
- Behavior-level copy is owned by the behavior (intent + acceptance);
  surface-level copy tone is owned here.
- Reusable tokens, vocabulary, and pattern IDs are owned by
  `docs/experience/design-system/`; new local interaction rules should be
  added there or recorded as explicit deviations before implementation.

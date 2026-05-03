---
kind: standard
name: tech-stack
category: global
importance: high
applies_to: []
applies_to_languages: []
applies_to_domains: []
---

# Tech Stack

Tanren's modern 2026 default tech stack. Architecture records under
`docs/architecture/` are the canonical source of truth; this profile is
the ambient default for code and tooling decisions.

**Rust toolchain:** pinned via `rust-toolchain.toml` at workspace root.

**Rust runtime + framework:**
- `tokio` — async runtime
- `axum` — HTTP framework for `tanren-api-app`
- `sea-orm` — database ORM
- `tower-sessions` (+ `tower-sessions-sqlx-store`) — HTTP session middleware
- `utoipa` + `utoipa-axum` — OpenAPI generation from types
- `rmcp` — Model Context Protocol server
- `ratatui` + `crossterm` — terminal UI for `tanren-tui`

**Rust crypto + secrets:**
- `argon2` (with `password-hash`) — password hashing
- `secrecy` — secret wrapping for in-memory handling

**Rust errors + observability:**
- `anyhow` — application-layer errors
- `thiserror` — library-layer error enums
- `tracing` + `tracing-subscriber` — structured observability (init via
  `tanren_observability::init`)

**Rust testing:**
- `cucumber` — Rust BDD runner
- `expectrl` + `portable-pty` — `@tui` PTY-driven test driver
- `wiremock` — outbound HTTP boundary fakes
- `playwright-bdd` (Node-side) — `@web` slice consuming the same
  `.feature` files via symlink

**Web (apps/web):**
- `Next.js` — React framework
- `paraglide-js` — i18n (replaces `react-i18next`)
- `valibot` — schema validation (replaces `zod`)
- `Tailwind v4` — styling (replaces v3 + inline styles)
- `Storybook 9` with the Vitest addon and a11y addon
- `Playwright` + `playwright-bdd` — browser BDD
- `oxlint` — JS/TS linter
- `tsgo` — TypeScript type-checker
- `prettier` — formatter
- `vitest` — unit/component test runner

**Why:** Pinning the modern 2026 stack as the default profile keeps tool
choice consistent across crates and apps and stops drift toward
deprecated picks (`zod`, `react-i18next`, Tailwind v3 + inline styles).

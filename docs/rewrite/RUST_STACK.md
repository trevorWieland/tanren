# Tanren Clean-Room Rewrite: Rust Stack

## Goal

Define a pragmatic, modern Rust stack for a production orchestration control
plane: correct by default, observable, secure, and scalable.

## Toolchain and Workspace Baseline

### Rust Versioning

- Stable channel
- Rust edition `2024`
- Workspace-level lint policy (strict defaults)

### Workspace Structure

- Cargo workspace with bounded crates (single responsibility per crate)
- Shared dependency versions via `[workspace.dependencies]`
- Shared lint policy via `[workspace.lints.*]`
- `just` as the primary task runner for local + CI command parity

### Core Quality Gates

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test` (prefer `cargo nextest` for CI speed/reliability)
- dependency policy (`cargo deny`)
- doc build with warnings as errors
- orchestration via `just ci` recipe that composes all required checks

## Recommended Runtime and Libraries

### Async and Concurrency

- `tokio` for runtime
- `futures`/`tokio-stream` as needed
- bounded queues/channels for backpressure (`tokio::sync`)

### API and Transport

- `axum` + `tower` for HTTP API
- `hyper`/`reqwest` for internal and external clients
- explicit timeout/retry layers via `tower` middleware

### Serialization and Schemas

- `serde` + `serde_json`
- typed schema boundaries in domain crates
- optional OpenAPI generation for API surface (crate choice can be finalized later)

### Storage and Migrations

- `sqlx` for async DB access with compile-time checked queries
- support SQLite (local) + Postgres (team/enterprise)
- migration framework integrated into CI and startup checks

### CLI and TUI

- `clap` for CLI
- `ratatui` for TUI (if terminal surface is implemented in initial phases)

### Observability

- `tracing` + `tracing-subscriber` for structured logging
- OpenTelemetry integration for traces/metrics export
- Prometheus-compatible metrics endpoint

### Error Handling

- `thiserror` for typed domain/application errors
- `anyhow` only at binary/composition boundaries
- explicit error classification for retry/policy/reporting logic

### Security and Secrets

- `secrecy` for secret wrappers and redaction-safe debug behavior
- `zeroize` for sensitive buffers where applicable
- `rustls`-based TLS stack for network surfaces

### Resilience Utilities

- retry/backoff crate for transient failure handling
- rate limiting/governor layer where external API protection is required

## Suggested Crate Topology (Initial)

- `tanren-domain` — canonical domain model (entities, commands, events, errors)
- `tanren-policy` — authz, quota, budget, placement policy evaluation
- `tanren-store` — event log + projections + migrations
- `tanren-planner` — graph planning and replanning
- `tanren-scheduler` — lane/capability-aware dispatch scheduling
- `tanren-runtime` — harness + environment trait contracts
- `tanren-runtime-local`
- `tanren-runtime-docker`
- `tanren-runtime-remote`
- `tanren-harness-claude`
- `tanren-harness-codex`
- `tanren-harness-opencode`
- `tanren-api` — HTTP service
- `tanren-cli` — CLI app
- `tanren-mcp` — MCP surface
- `tanren-tui` — optional terminal UI
- `tanren-observability` — metrics/tracing helpers

## Testing Strategy

- Unit tests per crate
- Contract tests for domain + policy invariants
- Integration tests for runtime adapters
- DB integration tests for SQLite and Postgres
- property tests (`proptest`) for state transitions and parser/mapper correctness
- testcontainers-based substrate tests where needed

## CI/CD Expectations

- matrix CI for Linux/macOS on core checks
- required checks for formatting, lint, tests, dependency policy, docs
- coverage reporting for trend visibility (no vanity thresholds without stability)
- release pipeline should enforce semver and changelog discipline
- CI should invoke `just` recipes directly to avoid drift from local developer workflows

## Decision Rules

1. Prefer compile-time guarantees over runtime conventions.
2. Keep runtime adapters thin; keep domain logic in shared crates.
3. Avoid framework lock-in for MCP/API layers; contracts come first.
4. Never allow secrets to leak through logs, events, or config serialization.
5. Design for multi-tenant governance from the start, even in local mode.

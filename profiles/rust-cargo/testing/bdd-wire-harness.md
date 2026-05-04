---
kind: standard
name: bdd-wire-harness
category: testing
importance: high
applies_to:
  - "**/*test*"
  - "**/*spec*"
  - "tests/**"
applies_to_languages:
  - rust
  - typescript
applies_to_domains:
  - testing
---

# Per-interface BDD Wire Harnesses

Every interface tagged in a BDD scenario MUST drive the actual surface,
not the in-process handler. The interface tag (`@api`, `@cli`, `@mcp`,
`@tui`, `@web`) is a witness to be earned, not a label to be claimed.

## Harness traits live in `tanren-testkit`

The `tanren-testkit` crate hosts per-feature harness traits. R-0001
introduces `AccountHarness`; future feature areas add analogues
(`SchedulingHarness`, `BillingHarness`, etc.) following the same shape.

```rust
#[async_trait::async_trait]
pub trait AccountHarness: Send + Sync {
    type SessionHandle;

    async fn sign_up(
        &mut self,
        req: SignUpRequest,
    ) -> HarnessResult<SignUpResponse>;

    async fn sign_in(
        &mut self,
        req: SignInRequest,
    ) -> HarnessResult<SignInResponse>;

    async fn accept_invitation(
        &mut self,
        req: AcceptInvitationRequest,
    ) -> HarnessResult<AcceptInvitationResponse>;

    async fn sign_in_with_session_handle(
        &mut self,
        handle: &Self::SessionHandle,
    ) -> HarnessResult<()>;
}
```

The trait is the contract. Every interface implementation produces the
same response shapes from the same request shapes — what differs is the
wire that carried them.

## Per-interface implementations

| Tag | Implementation |
|---|---|
| `@api` | Spawns `tanren-api-app` on an ephemeral port. Uses `reqwest` with a cookie jar so session cookies round-trip exactly as a browser would. `SessionHandle` wraps the cookie jar. |
| `@cli` | Drives the `tanren-cli` binary via `tokio::process::Command`. Stdin/stdout are piped; the structured event identifiers on stdout are parsed to assemble responses. `SessionHandle` is a path to the on-disk session credential. |
| `@mcp` | Spawns `tanren-mcp-app` on an ephemeral port. Uses an `rmcp` client to issue MCP tool calls. `SessionHandle` is the session token returned by sign-in. |
| `@tui` | Wraps `tanren-tui` in a pseudoterminal via `expectrl` + `portable-pty`. Steps drive keystrokes and assert on rendered ANSI output. `SessionHandle` is the on-disk session credential as for the CLI. |
| `@web` | `playwright-bdd` runs against a running api server + a Next.js dev server. Browser cookies form the `SessionHandle`. The `@web` slice runs on the Node side; everything else is Rust. |

## Step dispatch

Step definitions in `crates/tanren-bdd/src/steps/account.rs` dispatch
through the world's currently-active harness. The active harness is
selected at scenario start from the interface tag.

```rust
// ✓ Good: step dispatches through the per-interface harness
#[when(regex = r"^I sign up as (.+)$")]
async fn sign_up(world: &mut TanrenWorld, email: String) {
    let resp = world.harness_mut()
        .sign_up(SignUpRequest { email, .. })
        .await
        .expect("sign_up");
    world.last_response = Some(resp);
}
```

```rust
// ✗ Bad: step calls the in-process handler directly
#[when(regex = r"^I sign up as (.+)$")]
async fn sign_up(world: &mut TanrenWorld, email: String) {
    let resp = tanren_app_services::Handlers::sign_up(&world.deps, req)
        .await
        .unwrap();
    world.last_response = Some(resp);
}
```

## Mechanical enforcement

- `xtask check-bdd-wire-coverage` (AST walker) rejects any direct
  `Handlers::` call inside `crates/tanren-bdd/src/steps/**`. The witness
  earned by an interface tag must be earned over its real wire.
- `just check-deps` rejects `tanren-app-services` in
  `crates/tanren-bdd/Cargo.toml`. The crate cannot reach the in-process
  handlers even by accident — the dep edge is closed.

## Source-of-truth and the web slice

`tests/bdd/features/B-XXXX-*.feature` is the single source of truth for
behavior scenarios. The `@web` slice runs on Node via `playwright-bdd`,
which is a separate runner; rather than duplicate scenario text, the
location `apps/web/tests/bdd/features/` is a symlink that points at
`tests/bdd/features/`. The same files drive both runners; only the step
implementations differ.

## Why

Tagging a scenario `@api` is a claim that the api wire actually serves
this behavior. If the step body calls a handler in-process, the witness
is fraudulent — the api request handler, the auth middleware, the
session cookie path, the OpenAPI binding, none of it was exercised.
Routing every interface witness through a real wire harness keeps the
proof honest. Mechanical enforcement (AST walk + dependency edge)
removes the failure mode where someone "just for now" reaches into
`Handlers::` from a step.

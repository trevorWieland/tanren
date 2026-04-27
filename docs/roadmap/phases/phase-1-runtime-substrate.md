# Phase 1: Runtime Substrate

## Objective

Make Tanren execute dispatches reliably through multiple harnesses and
environment types while preserving one domain contract and one event truth.

## Work Items

- Harness contract: define capability discovery, execution lifecycle, streamed
  output, tool invocation, approval boundaries, and normalized failure classes.
- Harness adapters: implement Claude Code, Codex, and OpenCode adapters behind
  the shared contract.
- Environment lease contract: model provision, execute, teardown, cancel, and
  recovery semantics for isolated work.
- Environment adapters: implement local worktree, local container, and
  Docker-outside-of-Docker execution paths.
- Worker runtime: consume queued work, acquire leases, select harnesses,
  persist outcomes, retry typed transient failures, and resume safely after
  interruption.

## Acceptance Evidence

- Accepted behavior docs have positive and falsification witnesses under
  `tests/bdd/features`.
- `just ci` passes with the new runtime crates included.
- Adapter-specific payloads do not leak into the domain contract.
- Cancellation and crash recovery leave durable, queryable evidence.

# Protocol Documentation

This folder defines the coordinator <-> worker-manager file-based IPC contract.

## Read This First

Use this page as an index. The canonical protocol specification is
`PROTOCOL.md`.

## Contents

- `PROTOCOL.md` - full normative spec:
  - filesystem layout and atomic write rules
  - dispatch/result/workflow schemas
  - top-level and orchestrating state machines
  - concurrency and worktree rules
  - retry and timeout behavior
  - edge-case handling and worked examples
  - signal extraction reference

## When To Update

Update protocol docs in the same PR as any change to:

- dispatch/result field shape
- state names or transition behavior
- queueing, retry, timeout, or nudge semantics
- heartbeat or workflow recovery rules

Related high-level docs:

- `../docs/interfaces.md`
- `../docs/workflow/spec-lifecycle.md`

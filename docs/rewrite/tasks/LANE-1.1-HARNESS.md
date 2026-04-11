# Lane 1.1 — Harness Contract

> **Status:** Stub. Full brief to be written at the start of Phase 1.
> This file captures requirements carried forward from earlier lane
> audits so they are not lost.

## Scope

Defines the contract that harness adapters (`tanren-harness-claude`,
`tanren-harness-codex`, `tanren-harness-opencode`, `tanren-harness-bash`)
must satisfy when producing an `ExecuteResult` from a CLI invocation.

## Carried-Forward Requirements

### Output redaction (from Lane 0.2 audit)

`ExecuteResult::tail_output`, `ExecuteResult::stderr_tail`, and
`ExecuteResult::gate_output` are captured verbatim and serialized into
`StepCompleted` events, which are persisted to the event log
indefinitely. A secret that leaks into an event log is effectively
unrecoverable.

**Harness adapters MUST redact known secret patterns before producing
an `ExecuteResult`.** At minimum:

1. **API keys, bearer tokens, cookies, and session identifiers.**
   Match the common patterns (`sk-…`, `Bearer …`, `xoxb-…`,
   `AKIA…`, `ghp_…`, `AIza…`) before capturing stdout/stderr.
2. **Values of environment variables listed in
   `dispatch.required_secrets`.** The harness has access to the
   dispatch snapshot and the resolved secret values; any occurrence of
   those values in captured output must be replaced before the tail
   strings are populated.
3. **Contents of known credential files.** If the harness tails logs
   that may include file content (`~/.aws/credentials`, `~/.netrc`,
   `~/.config/gcloud/*`, `id_rsa`), those files must be filtered.
4. **Multi-line secrets.** Redaction must operate on the full captured
   output, not only line-by-line, so multi-line PEM keys cannot slip
   through.

The domain crate cannot enforce this contract — it has no harness
context — so this lane MUST implement redaction at the capture site
and add unit tests exercising each redaction pattern.

### Runtime type tagging (from Lane 0.2 audit)

`LeaseCapabilities.runtime_type` is currently `NonEmptyString`
(e.g. `"local"`, `"docker"`, `"dood"`, `"remote"`). A string tag keeps
third-party runtime adapters extensible without touching the domain
crate but trades compile-time safety.

**Action for this lane:** Once the built-in runtime set is stable,
re-evaluate whether to introduce a `RuntimeKind::{Local, Docker, DooD,
Remote, Custom(String)}` enum. The `Custom(String)` variant preserves
third-party extensibility while giving the built-ins compile-time
coverage.

## Dependencies

- Lane 0.2 (domain model) — requires `ExecuteResult`, `DispatchSnapshot`,
  `EnvironmentHandle`, `LeaseCapabilities`
- Lane 0.3 (store) — harness outputs end up in the event log via the
  store

## Open Questions

- Does redaction happen in the harness adapter or in a separate
  `output-redactor` crate that every harness depends on?
- Do we want a policy-driven allowlist/denylist of redaction patterns
  per org?

# Lane 1.2 — Runtime Substrate

> **Status:** Stub. Full brief to be written at the start of Phase 1.

## Scope

Implements concrete execution runtimes (`tanren-runtime-local`,
`tanren-runtime-docker`, `tanren-runtime-remote`) behind the
`ExecutionRuntime` trait defined in `tanren-runtime`.

## Carried-Forward Notes from Lane 0.2 Audit

### `LeaseCapabilities.runtime_type` typing

Currently a `NonEmptyString`. Decision to leave it stringly-typed was
deliberate: keeps third-party runtime adapters extensible without
touching the domain crate. Re-evaluate once the built-in runtime set is
stable and we have usage data from real dispatches.

If the decision is to type it, the recommended shape is:

```rust
pub enum RuntimeKind {
    Local,
    Docker,
    DooD,
    Remote,
    Custom(String),
}
```

The `Custom(String)` variant preserves the current extensibility path
while giving the built-ins compile-time coverage. Migration: change
`LeaseCapabilities.runtime_type` from `NonEmptyString` to `RuntimeKind`
and bump `SCHEMA_VERSION` per the domain module-level policy.

## Dependencies

- Lane 0.2 (domain model)
- Lane 1.1 (harness contract)

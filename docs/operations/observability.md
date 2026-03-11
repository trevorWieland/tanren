# Observability and Metering

Tanren emits structured events around dispatch handling, phase execution,
retries, and VM lifecycle.

## Event Types

Common emitted events:

- `DispatchReceived`
- `PhaseStarted`
- `PhaseCompleted`
- `PreflightCompleted`
- `PostflightCompleted`
- `RetryScheduled`
- `ErrorOccurred`
- `VMProvisioned`
- `VMReleased`
- `BootstrapCompleted`

## Storage Backends

- Default: `NullEventEmitter` (no persistence)
- Optional: `SqliteEventEmitter` via `WM_EVENTS_DB`

## Typical Queries

- duration by phase
- failure rate by error class
- VM utilization and estimated cost
- workflow completion throughput by project

For implementation details and schema context, see
`worker-manager/README.md` and `protocol/PROTOCOL.md`.

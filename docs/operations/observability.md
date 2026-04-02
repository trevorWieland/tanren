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

Events are stored via the unified `Store` class
(`tanren_core.store.repository`), configured via `db_url`.  The store
implements the `EventStore` protocol (append + query) defined in
`tanren_core/store/protocols.py` and supports both SQLite and Postgres
through SQLAlchemy 2.0 ORM.

## Typical Queries

- duration by phase
- failure rate by error class
- VM utilization and estimated cost
- workflow completion throughput by project

For implementation details and schema context, see
`../worker-README.md` and `../protocol/README.md`.

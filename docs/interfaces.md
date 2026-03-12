# Interaction Interfaces

Tanren currently exposes three primary interaction methods.

## CLI (`tanren`)

Primary for local operation and debugging.

- `tanren env` - validate and scaffold env requirements
- `tanren secret` - manage developer secret values
- `tanren vm` - inspect/recover/release VM allocations
- `tanren run` - provision/execute/teardown or full lifecycle

## Python Library Surface

Coordinators can import worker-manager classes and inject adapter
implementations directly. Protocol interfaces in
`worker-manager/src/worker_manager/adapters/protocols.py` are the stable
extension boundary.

## File-Based IPC

Coordinator and worker-manager communicate via filesystem queues:

- `dispatch/` -> coordinator writes dispatches
- `results/` -> worker writes outcomes
- `input/` -> worker writes nudges
- `in-progress/` -> worker heartbeats

Canonical schema and state transitions live in `protocol/PROTOCOL.md`.

## Planned Interface

An HTTP API is planned for dashboard and multi-coordinator use cases.

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
`packages/tanren-core/src/tanren_core/adapters/protocols.py` are the stable
extension boundary.

## File-Based IPC

Coordinator and worker-manager communicate via filesystem queues:

- `dispatch/` -> coordinator writes dispatches
- `results/` -> worker writes outcomes
- `input/` -> worker writes nudges
- `in-progress/` -> worker heartbeats

Canonical schema and state transitions live in `protocol/PROTOCOL.md`.

## HTTP API

Tanren exposes a FastAPI-based HTTP API for dashboard and multi-coordinator use cases.

- **Base URL**: `http://localhost:8000` (default)
- **Authentication**: `X-API-Key` header on protected endpoints

| Endpoint | Method | Auth | Description |
|----------|--------|------|-------------|
| `/health` | GET | No | Health check |
| `/dispatch` | POST | Yes | Submit a new dispatch |
| `/dispatch/{dispatch_id}` | GET | Yes | Get dispatch status |
| `/dispatches` | GET | Yes | List dispatches |
| `/run/provision` | POST | Yes | Provision an execution environment |
| `/run/execute` | POST | Yes | Execute a phase in a provisioned environment |
| `/run/teardown` | POST | Yes | Tear down an execution environment |
| `/run/full` | POST | Yes | Full lifecycle (provision + execute + teardown) |
| `/vm/list` | GET | Yes | List active VM assignments |
| `/vm/{vm_id}/release` | POST | Yes | Release a VM |
| `/vm/recover` | POST | Yes | Recover unreachable VMs |
| `/vm/dry-run` | POST | Yes | Dry-run VM provisioning |
| `/events` | GET | Yes | Query events |
| `/events/stream` | GET | Yes | SSE event stream |
| `/agents` | GET | Yes | List agent configurations |
| `/agents/{agent_id}` | GET | Yes | Get agent detail |

Configuration via `TANREN_API_*` environment variables (host, port, API key, CORS origins).

OpenAPI spec: `services/tanren-api/openapi.json`

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
| `/api/v1/health` | GET | No | Health check |
| `/api/v1/health/ready` | GET | No | Readiness probe |
| `/api/v1/config` | GET | Yes | Non-secret config projection |
| `/api/v1/dispatch` | POST | Yes | Submit a new dispatch |
| `/api/v1/dispatch/{dispatch_id}` | GET | Yes | Get dispatch status |
| `/api/v1/dispatch/{dispatch_id}` | DELETE | Yes | Cancel a dispatch |
| `/api/v1/run/provision` | POST | Yes | Provision execution environment |
| `/api/v1/run/{env_id}/execute` | POST | Yes | Execute phase in provisioned env |
| `/api/v1/run/{env_id}/teardown` | POST | Yes | Tear down execution environment |
| `/api/v1/run/full` | POST | Yes | Full lifecycle (provision+execute+teardown) |
| `/api/v1/run/{env_id}/status` | GET | Yes | Poll environment run status |
| `/api/v1/vm` | GET | Yes | List active VM assignments |
| `/api/v1/vm/provision` | POST | Yes | Provision a new VM |
| `/api/v1/vm/{vm_id}` | DELETE | Yes | Release a VM |
| `/api/v1/vm/dry-run` | POST | Yes | Dry-run VM provisioning |
| `/api/v1/events` | GET | Yes | Query structured events |

Configuration via `TANREN_API_*` environment variables (host, port, API key, CORS origins).

OpenAPI spec: `services/tanren-api/openapi.json`

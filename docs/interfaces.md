# Interaction Interfaces

Tanren exposes four primary interaction methods and a set of store protocols
that define the internal queue contract.

## CLI (`tanren`)

Primary for local operation and debugging.

- `tanren env` - validate and scaffold env requirements
- `tanren secret` - manage developer secret values
- `tanren vm` - inspect/recover/release VM allocations
- `tanren run` - provision/execute/teardown or full lifecycle

## Python Library Surface

Coordinators can import core classes and inject adapter implementations
directly. Protocol interfaces in
`packages/tanren-core/src/tanren_core/adapters/protocols.py` are the stable
extension boundary for execution environments and adapters.

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

## MCP Server

The MCP server exposes tanren operations as tools that LLM-based agents
(e.g. Claude Code) can invoke directly. It shares the same store backend
as the HTTP API.

- **Endpoint**: `/mcp/sse` on the same port as the API (default 8000)
- **Authentication**: `X-API-Key` header (same key as the HTTP API)
- **Tools**: 17 tools across dispatch, run, VM, events, and metrics domains
- **CLI auto-resolution**: `cli`/`auth` parameters are optional — resolved
  from `roles.yml` when omitted

See [mcp-setup.md](mcp-setup.md) for full setup instructions including
Claude Code `.mcp.json` configuration and required environment variables.

## Store Protocols

The internal queue contract is defined by three store protocols in
`packages/tanren-core/src/tanren_core/store/protocols.py`:

| Protocol | Responsibility |
|----------|----------------|
| `EventStore` | Append-only event log with transactional projection updates |
| `JobQueue` | Step-based job queue (enqueue, dequeue, ack, nack) |
| `StateStore` | Read-only queries against dispatch and step projections |

Backed by SQLite (default) or Postgres. See [protocol/README.md](../protocol/README.md)
for the full dispatch lifecycle and lane definitions.

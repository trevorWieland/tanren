# MCP Server Setup

The tanren MCP server exposes core operational capabilities (dispatch, VM,
run lifecycle, events, metrics, config) as tools that LLM-based agents
(e.g. Claude Code) can invoke directly.  Admin operations (user and key
management) are available only via the REST API.

## Endpoint

The MCP server runs as part of the API service at:

```
http://localhost:<port>/mcp/sse
```

Default port is `8000` (configurable via `TANREN_API_PORT`).

## Authentication

All MCP tools (except health/readiness) require an API key via the
`X-API-Key` header. Set `TANREN_API_API_KEY` in your API environment.

## Claude Code Configuration

Add to your project or user `.mcp.json`:

```json
{
  "mcpServers": {
    "tanren": {
      "type": "sse",
      "url": "http://localhost:8000/mcp/sse",
      "headers": {
        "X-API-Key": "your-api-key"
      }
    }
  }
}
```

## Required Environment Variables

The API process needs `WM_*` env vars for MCP dispatch resolution:

| Variable | Purpose |
|----------|---------|
| `WM_GITHUB_DIR` | Root directory containing project repos |
| `WM_REMOTE_CONFIG` | Path to `remote.yml` (for remote profiles) |
| `WM_ROLES_CONFIG_PATH` | Path to `roles.yml` (for CLI auto-resolution) |
| `WM_EVENTS_DB` | Database path (must match daemon) |
| `WM_DATA_DIR` | Data directory for worker state |

Without these, MCP tools that create dispatches will fail. The API logs
a warning at startup if `WM_*` vars are not set.

## Available Tools

### Dispatch (fire-and-forget)

| Tool | Description |
|------|-------------|
| `dispatch_create` | Submit a dispatch, auto-chains provision → execute → teardown |
| `dispatch_get_status` | Poll dispatch status by workflow ID |
| `dispatch_cancel` | Cancel a pending or running dispatch |

### Run (step-by-step control)

| Tool | Description |
|------|-------------|
| `run_provision` | Provision a VM/workspace |
| `run_execute` | Execute a phase on a provisioned environment |
| `run_teardown` | Release the VM/workspace |
| `run_full` | Provision + execute + teardown in one call |
| `run_status` | Poll environment status |

### VM Management

| Tool | Description |
|------|-------------|
| `vm_list` | List active VMs |
| `vm_release` | Release a stuck VM |
| `vm_dry_run` | Preview what provision would do (cost estimation) |

### Observability

| Tool | Description |
|------|-------------|
| `events_query` | Query structured events (filter by workflow, type) |
| `metrics_summary` | Execution success rates and duration stats |
| `metrics_costs` | Token usage costs (group by model/day/workflow) |
| `metrics_vms` | VM utilization and estimated costs |

## CLI Auto-Resolution

When `cli` and `auth` parameters are omitted from dispatch/execute tools,
they are auto-resolved from `roles.yml` based on the phase:

- `gate` → `bash` CLI with `api-key` auth
- `do-task` → implementation role from `roles.yml`
- `audit-task` → audit role from `roles.yml`
- Other phases → mapped via `roles.yml` role definitions

This matches the CLI behavior where `resolve_agent_tool()` handles
resolution automatically.

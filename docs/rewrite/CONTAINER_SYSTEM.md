# Tanren Clean-Room Rewrite: Container and Execution System

## Overview

Tanren execution must support multiple substrates with one orchestration model:

- local worktree execution
- local Docker containers
- DooD execution inside compose stacks
- remote VM-backed execution (Hetzner/GCP/DigitalOcean and future)

The container system is part of the broader execution environment layer, not a
standalone mode with separate orchestration semantics.

## Execution Lease Lifecycle

All execution substrates implement a common lease lifecycle:

`Requested -> Provisioning -> Ready -> Running -> Idle (optional reuse) -> Draining -> Released`

Error path:

`* -> Failed -> Releasing -> Released`

Cancellation path:

`Running/Ready -> Draining -> Released`

### State Semantics

| State | Description | Timeout Class |
|------|-------------|---------------|
| `Requested` | Placement/policy accepted, lease creation pending | queue timeout |
| `Provisioning` | Substrate setup in progress (container/VM/worktree) | provision timeout |
| `Ready` | Lease exists and can accept work | ready TTL |
| `Running` | A dispatch step is actively executing | execution timeout |
| `Idle` | Reusable lease waiting for follow-up work | idle TTL |
| `Draining` | Graceful shutdown and cleanup in progress | drain timeout |
| `Released` | Terminal success state, resources gone | terminal |
| `Failed` | Terminal failure prior to cleanup | terminal |

## Runtime Traits

The control plane depends on traits, not concrete runtimes.

### ExecutionRuntime

Core operations:

- `provision(spec) -> LeaseHandle`
- `run(handle, task) -> RunResult`
- `drain(handle) -> ()`
- `release(handle) -> ()`
- `health() -> RuntimeHealth`

### Runtime Implementations (Initial)

1. `LocalWorktreeRuntime`
2. `DockerRuntime` (local daemon socket)
3. `ComposeSiblingRuntime` (DooD-aware network and mount policy)
4. `RemoteVmRuntime` (provider adapter + remote execution transport)

## Container Spec Model

Container-backed runtimes share a spec model:

- image
- entrypoint/command
- environment variables (non-secret only; secret references resolved at runtime)
- mounts
- network policy
- resource limits (cpu/memory/gpu class)
- labels/metadata for ownership and cleanup
- runtime security profile (user, capabilities, read-only rootfs where feasible)

## Mount and Filesystem Security

Mount handling must be policy-driven:

- external allowlist for mount sources
- symlink resolution before validation
- blocked patterns (secret stores, host system paths)
- read-only defaults except explicit write targets
- per-tenant/group workspace boundaries

No mount path should be trusted because it appears in request payloads.

## Network Policy

Containerized executions must declare allowed network targets explicitly:

- allowed compose services (for DooD mode)
- allowed egress profile (none/internal/full)
- optional DNS-only behavior for restricted environments

Policy decision is part of lease provisioning and must be auditable.

## Warm Pool and Reuse Strategy

For high-frequency workloads, container runtimes may maintain a warm lease pool:

- configurable global or per-project/per-lane pool size
- background replenishment
- idle eviction
- hit/miss metrics and cost visibility

Reuse is optional and policy-gated. Sensitive workloads can require single-use leases.

## Orphan and Crash Recovery Guarantees

Recovery responsibilities:

- detect stale running leases after process crash
- force release orphaned containers/VMs/worktrees
- reconcile lease projection with substrate truth
- prevent duplicate claim of same lease handle

No resource should persist indefinitely after control-plane loss.

## Observability Requirements

Required metrics/events for container/execution system:

- lease provision latency by runtime type
- warm/cold start distribution
- running/idle pool counts
- timeout and failure class rates
- release success/failure rates
- estimated and actual runtime cost attribution

## Non-Goals

- Baking orchestration policy directly into a single runtime implementation
- Creating runtime-specific state machines that diverge from domain lifecycle
- Accepting unmanaged long-lived execution resources without ownership metadata

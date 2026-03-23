# Adapter Architecture

The tanren-core library uses protocol-based dependency injection to keep its core
orchestration logic decoupled from concrete infrastructure. Every external
concern -- git operations, process spawning, environment validation, event
storage -- is accessed through a `typing.Protocol` interface. The `Worker`
class accepts injected adapters; when omitted, it falls back to built-in
defaults that cover the common case (local subprocess execution against a
git repository with dotenv-based secrets).

This design follows the principle of **opinionated defaults, pluggable
internals**: the built-in adapters handle the 90% case out of the box, while
the protocol boundaries let you swap in alternatives (Docker, remote VM, Vault,
Postgres) without modifying core orchestration code.

All protocol classes are defined in `adapters/protocols.py` and decorated with
`@runtime_checkable`, so custom implementations can be validated at startup
with `isinstance()`.


## Adapter Categories

The table below lists every integration category in the tanren ecosystem.
The tanren-core library directly owns protocols for the first six categories. The
remaining categories (CI/CD, Secret Management, Token Usage, Coordinator
Interface) are handled by other components but listed here for completeness.

| Category | Built-in | Also Supports |
|---|---|---|
| Issue Source | GitHub Issues | Linear, Jira, custom |
| Source Control | GitHub | GitHub Enterprise, GitLab, Bitbucket |
| Execution Environment | Local subprocess | Docker, remote VM via SSH, cloud VM w/ lifecycle |
| CI/CD | GitHub Actions | GitLab CI, Jenkins, CircleCI |
| Secret Management | Flat file (`~/.config/tanren/secrets.env`) | Vault, AWS/GCP Secret Manager |
| Event/Metrics Storage | SQLite | Postgres, BigQuery, custom |
| Token Usage Collection | Log parsing (ccusage-style) | Metering proxy |
| Coordinator Interface | Web dashboard + CLI (built-in) | Messaging platforms (pluggable) |


## Current Protocol Interfaces

The tanren-core library defines eight protocols. Each one covers a single
responsibility and has exactly one built-in concrete implementation.

### WorktreeManager

Create, register, and clean up git worktrees for isolated agent workspaces.

```python
class WorktreeManager(Protocol):
    async def create(self, project: str, issue: int, branch: str, github_dir: str) -> Path: ...
    async def register(
        self,
        registry_path: Path,
        workflow_id: str,
        project: str,
        issue: int,
        branch: str,
        worktree_path: Path,
        github_dir: str,
    ) -> None: ...
    async def cleanup(self, workflow_id: str, registry_path: Path, github_dir: str) -> None: ...
```

**Lifecycle:** `create` is called during the SETUP phase to produce an isolated
working directory. `register` records it in a JSON registry so the coordinator
knows where each workflow lives. `cleanup` removes the worktree and its
registry entry during the CLEANUP phase.

**Default:** `GitWorktreeManager` -- delegates to `git worktree add/remove`.

### PreflightRunner

Run pre-flight checks before an agent process is spawned.

```python
class PreflightRunner(Protocol):
    async def run(
        self, worktree_path: Path, branch: str, spec_folder: Path, phase: str
    ) -> PreflightResult: ...
```

**Lifecycle:** Called before every work phase. Returns a `PreflightResult`
containing `passed`, `error`, `repairs` (auto-fixed issues), `file_hashes`
(snapshot for postflight diff), and `file_backups` (originals of protected
files).

**Default:** `GitPreflightRunner` -- verifies branch state, snapshots
protected files, clears stale status markers.

### PostflightRunner

Run post-flight integrity checks after an agent process exits.

```python
class PostflightRunner(Protocol):
    async def run(
        self,
        worktree_path: Path,
        branch: str,
        phase: str,
        preflight_hashes: dict[str, str],
        preflight_backups: dict[str, str],
        *,
        skip_push: bool = False,
    ) -> PostflightResult: ...
```

**Lifecycle:** Called after work phases that produce commits (DO_TASK,
AUDIT_TASK, RUN_DEMO, AUDIT_SPEC). Compares file hashes against preflight
snapshots, restores protected files if they were modified, and pushes the
branch. When `skip_push=True` (error/timeout outcomes), the push is skipped
but integrity checks still run.

**Default:** `GitPostflightRunner` -- delegates to `postflight.run_postflight()`.

### ProcessSpawner

Spawn CLI processes for dispatched work.

```python
class ProcessSpawner(Protocol):
    async def spawn(
        self,
        dispatch: Dispatch,
        worktree_path: Path,
        config: Config,
        *,
        task_env: dict[str, str] | None = None,
    ) -> ProcessResult: ...
```

**Lifecycle:** Called inside the retry loop. Receives the full `Dispatch`
(workflow ID, phase, CLI tool, branch, spec folder) and returns a
`ProcessResult` with `exit_code`, `stdout`, `duration_secs`, and `timed_out`.

**Default:** `SubprocessSpawner` -- wraps `asyncio.create_subprocess_exec`
with timeout handling.

### EnvValidator

Validate environment requirements before work phases.

```python
class EnvValidator(Protocol):
    async def load_and_validate(self, project_root: Path) -> tuple[EnvReport, dict[str, str]]: ...
```

**Lifecycle:** Called at the start of every work phase, before preflight. Loads
the project's `tanren.yml` env schema, resolves values from `.env` files and
the secrets store, and returns an `EnvReport` (pass/fail with diagnostics) plus
a `dict` of resolved key-value pairs to inject into the agent process.

**Default:** `DotenvEnvValidator` -- reads `tanren.yml` env requirements and
resolves from `.env` + `~/.config/tanren/secrets.env`.

### EnvProvisioner

Provision `.env` files in worktrees during setup.

```python
class EnvProvisioner(Protocol):
    def provision(self, worktree_path: Path, project_dir: Path) -> int: ...
```

**Lifecycle:** Called during SETUP after the worktree is created. Copies or
symlinks `.env` files from the main project directory into the worktree.
Returns the number of variables provisioned. This is a **sync** method; the
caller wraps it in `asyncio.to_thread()`.

**Default:** `DotenvEnvProvisioner` -- copies `.env` from project root into
the worktree.

### EventStore

Append structured events and query the event log for observability and
debugging. Defined in `store/protocols.py`.

```python
class EventStore(Protocol):
    async def append(self, event: Event) -> None: ...
    async def query_events(
        self, *, dispatch_id=None, event_type=None, since=None, until=None, limit=50, offset=0
    ) -> EventQueryResult: ...
    async def close(self) -> None: ...
```

**Lifecycle:** `append` is called at key points throughout dispatch handling
(DispatchReceived, PhaseStarted, PhaseCompleted, PreflightCompleted,
PostflightCompleted, RetryScheduled, ErrorOccurred). `query_events` supports
paginated, filterable reads over the event log. `close` is called during
graceful shutdown.

**Implementations:**
- `SqliteStore` -- writes events to a SQLite database with WAL mode and
  indexed columns for workflow_id, event_type, and timestamp.
- `PostgresStore` -- writes events to a Postgres database using an
  externally-owned `asyncpg` connection pool.

### ExecutionEnvironment

The highest-level adapter. Encapsulates the full lifecycle of where and how
agent work runs. See the deep dive below.

```python
class ExecutionEnvironment(Protocol):
    async def provision(self, dispatch: Dispatch, config: Config) -> EnvironmentHandle: ...
    async def execute(
        self, handle: EnvironmentHandle, dispatch: Dispatch, config: Config
    ) -> PhaseResult: ...
    async def get_access_info(self, handle: EnvironmentHandle) -> AccessInfo: ...
    async def teardown(self, handle: EnvironmentHandle) -> None: ...
```

**Defaults:**
- `LocalExecutionEnvironment` -- local subprocess execution (default when
  no `remote_config_path` is set).
- `SSHExecutionEnvironment` -- remote VM execution via SSH (used when
  `WM_REMOTE_CONFIG` points to a `remote.yml` file).


## ExecutionEnvironment Deep Dive

The `ExecutionEnvironment` protocol is the primary extension point for
running agent work in different contexts -- local subprocesses, Docker
containers, remote VMs, or cloud-managed instances.

### Lifecycle

```
provision() --> EnvironmentHandle
     |
     v
execute()   --> PhaseResult    (includes retry loop, heartbeat, postflight)
     |
     v
get_access_info() --> AccessInfo   (optional, for debugging)
     |
     v
teardown()  --> cleanup
```

1. **provision()** validates the environment (env vars, preflight checks) and
   returns an `EnvironmentHandle` that carries context through the remaining
   steps. Raises `ProvisionError` if validation or preflight fails.

2. **execute()** runs the agent process with heartbeat monitoring, transient
   error retry (up to 3 retries with backoff), and postflight integrity checks.
   Returns a `PhaseResult` with outcome, signal, duration, and metrics.

3. **get_access_info()** returns connection details for debugging a running
   environment -- SSH commands, VS Code remote URIs, working directory.

4. **teardown()** cleans up resources. No-op for local; would destroy
   containers or VMs for other environment types.

### Data Types

```python
class EnvironmentHandle(BaseModel):
    env_id: str  # UUID identifying this environment instance
    worktree_path: Path  # Where the code lives
    branch: str  # Git branch
    project: str  # Project name
    runtime: LocalEnvironmentRuntime | RemoteEnvironmentRuntime


class AccessInfo(BaseModel):
    ssh: str | None  # e.g., "ssh dev@10.0.1.42"
    vscode: str | None  # e.g., "code --remote ssh-remote+tanren-vm-3 /workspace"
    working_dir: str | None  # Local path or remote mount point
    status: str  # "running", "local", "stopped", etc.


class PhaseResult(BaseModel):
    outcome: Outcome  # SUCCESS, ERROR, TIMEOUT, etc.
    signal: str | None  # Extracted signal from agent output
    exit_code: int
    stdout: str | None
    duration_secs: int
    preflight_passed: bool
    postflight_result: PostflightResult | None
    env_report: EnvReport | None
    gate_output: str | None
    unchecked_tasks: int
    plan_hash: str
    retries: int
```

### LocalExecutionEnvironment

The built-in implementation composes the fine-grained adapters (EnvValidator,
PreflightRunner, PostflightRunner, ProcessSpawner, HeartbeatWriter) into the
four-method lifecycle:

```python
class LocalExecutionEnvironment:
    def __init__(self, *, env_validator, preflight, postflight, spawner, heartbeat, config): ...

    async def provision(self, dispatch, config) -> EnvironmentHandle:
        # 1. env_validator.load_and_validate()
        # 2. preflight.run()
        # 3. Return EnvironmentHandle with preflight state

    async def execute(self, handle, dispatch, config) -> PhaseResult:
        # 1. Start heartbeat
        # 2. Retry loop: spawner.spawn() -> extract signal -> map outcome
        # 3. Compute plan metrics
        # 4. postflight.run() for push phases
        # 5. Stop heartbeat, return PhaseResult

    async def get_access_info(self, handle) -> AccessInfo:
        # Returns AccessInfo(working_dir=worktree_path, status="local")

    async def teardown(self, handle) -> None:
        # No-op (heartbeat already stopped in execute)
```

### SSHExecutionEnvironment

The remote implementation composes sub-adapters for VM provisioning,
bootstrapping, workspace management, agent execution, and state persistence:

```python
class SSHExecutionEnvironment:
    def __init__(self, *, vm_provisioner, bootstrapper, workspace_mgr,
                 runner, state_store, secret_loader,
                 ssh_config_defaults, repo_urls): ...

    async def provision(self, dispatch, config) -> EnvironmentHandle:
        # 1. Read tanren.yml LOCALLY for environment profile
        # 2. Acquire VM from pool
        # 3. Create SSH connection
        # 4. Bootstrap VM (idempotent — docker, node, uv, claude)
        # 5. Setup workspace (git clone + branch checkout)
        # 6. Inject secrets (developer + project .env)
        # 7. Record assignment in SQLite
        # 8. Return EnvironmentHandle with VM metadata
        # On failure at any step: release VM (no orphans)

    async def execute(self, handle, dispatch, config) -> PhaseResult:
        # 1. Upload prompt, build CLI command with secret sourcing
        # 2. Execute via SSH with timeout
        # 3. Extract signal from remote filesystem
        # 4. Retry on transient errors (same logic as local)
        # 5. Push on push phases via SSH

    async def get_access_info(self, handle) -> AccessInfo:
        # Returns SSH command + VS Code Remote URI

    async def teardown(self, handle) -> None:
        # Guaranteed VM release with try/finally at every level:
        # 1. Run teardown commands from profile
        # 2. Cleanup workspace (remove secrets)
        # 3. Close SSH connection
        # 4. Release VM (MUST happen — no orphaned VMs)
        # 5. Record release in SQLite
```

**Sub-adapters:**

| Adapter | Protocol | Default | Purpose |
|---------|----------|---------|---------|
| `ManualVMProvisioner` | `VMProvisioner` | -- | Acquire/release from static VM list |
| `HetznerVMProvisioner` | `VMProvisioner` | -- | Create/delete/list Hetzner Cloud servers |
| `UbuntuBootstrapper` | `EnvironmentBootstrapper` | -- | Install dev tools (docker, node, uv, claude) |
| `GitWorkspaceManager` | `WorkspaceManager` | -- | Clone/pull, secret injection, cleanup |
| `RemoteAgentRunner` | -- | -- | Upload prompt, execute CLI, extract signal |
| `SqliteVMStateStore` | `VMStateStore` | -- | Persist VM assignments for recovery |
| `SecretLoader` | -- | -- | Load and bundle secrets for injection |

**Configuration via `remote.yml`:**

```yaml
execution_mode: remote
ssh:
  user: root
  key_path: ~/.ssh/tanren_vm
  port: 22
  connect_timeout: 10
git:
  auth: token
  token_env: GIT_TOKEN
provisioner:
  type: manual  # manual | hetzner
  settings:
    vms:
      - vm_id: vm-1
        host: 203.0.113.10
      - vm_id: vm-2
        host: 203.0.113.11
bootstrap:
  extra_script: ./scripts/vm-setup.sh  # optional
secrets:
  developer_secrets_path: ~/.config/tanren/secrets.env
repos:
  - project: my-project
    repo_url: https://github.com/org/my-project.git
    metadata: {}
```

Set `WM_REMOTE_CONFIG=/path/to/remote.yml` to enable remote execution.
For Hetzner support, install optional dependency: `uv sync --extra hetzner`.
Secrets are loaded explicitly from `remote.yml.secrets.developer_secrets_path`
during remote environment initialization (no global startup autoload).

### Dispatch-Aware vs. Generic Signatures

The tanren-core `ExecutionEnvironment` protocol takes `Dispatch` and
`Config` arguments, making it dispatch-aware. This is intentional: the
worker always has a dispatch context when it provisions and executes
environments.

The tanren architecture document defines a more generic version intended for
the future standalone orchestration engine:

```python
class ExecutionEnvironment(Protocol):
    async def provision(self, spec: EnvironmentSpec) -> EnvironmentHandle: ...
    async def execute(self, handle, cmd, env) -> ExecResult: ...
    async def observe(self, handle, query) -> ObservationResult: ...
    async def get_access_info(self, handle) -> AccessInfo: ...
    async def teardown(self, handle) -> None: ...
```

The generic version adds `observe()` for real-time monitoring and uses an
`EnvironmentSpec` (derived from `tanren.yml`) instead of `Dispatch`:

```yaml
environment:
  type: vm
  resources:
    cpu: 4
    memory: 16GB
    gpu: false
  compose: true
  setup:
    - docker compose up -d
    - ./scripts/seed-db.sh
```

When the standalone orchestration engine is built, it will use the generic
protocol. The tanren-core dispatch-aware version will remain as a
specialization that bridges dispatch semantics into the generic interface.


## Writing a Custom Adapter

To add a new execution environment (e.g., Docker), implement the
`ExecutionEnvironment` protocol. The example below shows the structure:

```python
import uuid
from pathlib import Path

from tanren_core.adapters.protocols import ExecutionEnvironment
from tanren_core.adapters.types import (
    AccessInfo,
    CustomEnvironmentRuntime,
    EnvironmentHandle,
    PhaseResult,
    ProvisionError,
)
from tanren_core.config import Config
from tanren_core.schemas import Dispatch, Outcome


class DockerExecutionEnvironment:
    """Run agent work inside Docker containers."""

    def __init__(self, *, image: str = "tanren-worker:latest", network: str = "tanren") -> None:
        self._image = image
        self._network = network

    async def provision(self, dispatch: Dispatch, config: Config) -> EnvironmentHandle:
        container_id = await self._create_container(dispatch, config)
        worktree_path = Path("/workspace") / dispatch.project

        return EnvironmentHandle(
            env_id=str(uuid.uuid4()),
            worktree_path=worktree_path,
            branch=dispatch.branch,
            project=dispatch.project,
            runtime=CustomEnvironmentRuntime(
                adapter="docker",
                metadata={"container_id": container_id},
            ),
        )

    async def execute(
        self, handle: EnvironmentHandle, dispatch: Dispatch, config: Config
    ) -> PhaseResult:
        container_id = handle.runtime.metadata["container_id"]
        exit_code, stdout = await self._docker_exec(container_id, dispatch)

        return PhaseResult(
            outcome=Outcome.SUCCESS if exit_code == 0 else Outcome.ERROR,
            signal=None,
            exit_code=exit_code,
            stdout=stdout,
            duration_secs=0,
            preflight_passed=True,
            postflight_result=None,
            env_report=None,
            gate_output=None,
        )

    async def get_access_info(self, handle: EnvironmentHandle) -> AccessInfo:
        container_id = handle.runtime.metadata["container_id"]
        return AccessInfo(
            ssh=f"docker exec -it {container_id} /bin/bash",
            working_dir=str(handle.worktree_path),
            status="running",
        )

    async def teardown(self, handle: EnvironmentHandle) -> None:
        container_id = handle.runtime.metadata["container_id"]
        await self._destroy_container(container_id)

    # -- private helpers (implement these) --

    async def _create_container(self, dispatch: Dispatch, config: Config) -> str:
        """Pull image, create and start container, clone repo, checkout branch."""
        raise NotImplementedError

    async def _docker_exec(self, container_id: str, dispatch: Dispatch) -> tuple[int, str]:
        """Run the agent command inside the container."""
        raise NotImplementedError

    async def _destroy_container(self, container_id: str) -> None:
        """Stop and remove the container."""
        raise NotImplementedError
```

The typed `runtime` field on `EnvironmentHandle` is the extension point for
carrying environment-specific state (for example local preflight context or
remote VM connection/workspace state) through the lifecycle.

For simpler adapters (e.g., a custom `EventStore` that forwards to an
external system), the pattern is the same: implement the protocol methods
and inject via the `Worker` constructor.


## Injecting Custom Adapters

All adapters are injected through the `Worker` constructor. Any
parameter left as `None` gets its built-in default:

```python
from tanren_core.worker import Worker
from tanren_core.config import Config

# Use all defaults
worker = Worker()

# Inject a custom execution environment
worker = Worker(
    execution_env=DockerExecutionEnvironment(image="my-image:v2"),
)

# Inject a custom event store alongside the default everything else
worker = Worker(
    event_store=PostgresStore(pool=my_pg_pool),
)

# Override fine-grained adapters (these feed into LocalExecutionEnvironment
# when no execution_env is provided)
worker = Worker(
    preflight=CustomPreflightRunner(),
    postflight=CustomPostflightRunner(),
    env_validator=VaultEnvValidator(vault_addr="https://vault.internal"),
)
```

The constructor signature:

```python
class Worker:
    def __init__(
        self,
        *,
        config: WorkerConfig,
        event_store: EventStore,
        job_queue: JobQueue,
        state_store: StateStore,
        execution_env: ExecutionEnvironment,
    ) -> None: ...
```

The `Worker` requires explicit store and execution environment injection.
Use the store factory (`tanren_core.store.factory`) to construct the
appropriate store backend (SQLite or Postgres) and pass the three store
protocols along with an `ExecutionEnvironment` implementation.

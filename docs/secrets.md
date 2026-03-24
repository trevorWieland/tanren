# Secret Management

Every secret in tanren falls into exactly one of four categories.
Understanding which category a secret belongs to determines where it's
configured, how it's delivered, and who can access it.

## Secret Taxonomy

### 1. Infrastructure Secrets

Authenticate the daemon to cloud providers and issue trackers. They never
leave the daemon process and never reach VMs.

| Secret | Purpose |
|--------|---------|
| `HCLOUD_TOKEN` | Hetzner VM provisioning |
| `GCP_SSH_PUBLIC_KEY` | GCP VM provisioning |
| `GIT_TOKEN` | Git clone/push on VMs (via `GIT_ASKPASS`, never in process args) |
| `GITHUB_TOKEN` | GitHub issue tracker API |
| `LINEAR_API_KEY` | Linear issue tracker API |
| SSH private key (file) | SSH into provisioned VMs |

**Where:** `daemon.env` or process environment. Set by the deployment admin.

### 2. Developer Secrets

Authenticate AI CLI tools on remote VMs. They belong to the user who
submits the dispatch.

| Secret | CLI | Injection method |
|--------|-----|-----------------|
| `CLAUDE_CODE_OAUTH_TOKEN` | claude | Env var in `.developer-secrets` |
| `CLAUDE_CREDENTIALS_JSON` | claude | Written as `~/.claude/.credentials.json` |
| `OPENCODE_ZAI_API_KEY` | opencode | Written as `auth.json` |
| `CODEX_AUTH_JSON` | codex | Written as `auth.json` |
| `MCP_CONTEXT7_KEY` etc. | any (MCP) | Referenced in MCP config headers |

**Where:** `daemon.env` (single-developer) or a cloud secret manager (team
deployments — future work).

**Reference-based injection:** The dispatch carries only secret *names* in
`required_secrets`. The daemon resolves values from its own `os.environ` at
provision time. **Secret values never appear in the dispatch payload or the
database.**

### 3. Project Secrets

Project-specific environment variables from the project's `.env` file.

| Example | Project |
|---------|---------|
| `RENTL_OPENROUTER_API_KEY` | rentl |
| `RENTL_QUALITY_API_KEY` | rentl |
| `PYPI_TOKEN` | rentl |

**Where:** `<project>/.env` (gitignored). Carried in `dispatch.project_env`
and written to `<workspace>/.env` on the VM.

> **Note:** Project env values *do* appear in dispatch payloads stored in
> the event store. Don't put high-sensitivity secrets here — use developer
> secrets instead.

### 4. Operational Config

API keys and settings for the tanren services themselves.

| Config | Purpose |
|--------|---------|
| `TANREN_API_API_KEY` | API authentication |
| `WM_EVENTS_DB` | Database URL |
| `WM_LOG_LEVEL` | Logging |

**Where:** `api.env` / `daemon.env`. No changes needed — these are not
injected onto VMs.

## Where Does My Env Var Go?

```
Does the tanren daemon itself use this value?
  YES → Is it for VM provisioning or issue tracking?
    YES → Infrastructure → daemon.env
    NO  → Operational → daemon.env or api.env
  NO  → Is it a CLI credential (claude/codex/opencode auth)?
    YES → Developer secret → daemon.env (single-dev) or secret manager (team)
    NO  → Is it project-specific (e.g. RENTL_* or PYPI_TOKEN)?
      YES → Project secret → project/.env (dispatch.project_env)
      NO  → Probably doesn't need to be in tanren at all
```

## Deployment Tiers

### Single-Developer (Local)

Everything in `~/.config/tanren/secrets.env` + project `.env`.
`daemon.env` has all developer secrets as env vars. Simple.

### Single-Developer (Docker)

Same secrets in `daemon.env`. Mounted via docker-compose `env_file`.
SSH key via entrypoint copy.

### Team (Shared Daemon) — Future Work

Developer secrets in a cloud secret manager (GCP Secret Manager, AWS
Secrets Manager, HashiCorp Vault). The daemon fetches per-user secrets
using a user identifier from the dispatch. Requires `SecretProvider`
integration.

## Secret Flow

```
CLI/API Client                              Daemon
─────────────                              ──────
reads secrets.env (validation only)         has all secrets in os.environ
reads required_clis from profile

dispatch.required_secrets = [               receives dispatch
  "CLAUDE_CODE_OAUTH_TOKEN",
  "OPENCODE_ZAI_API_KEY",                  resolves each name from os.environ:
  "MCP_CONTEXT7_KEY",                        CLAUDE_CODE_OAUTH_TOKEN → "sk-ant-..."
]                                            OPENCODE_ZAI_API_KEY → "1023bec..."
                                             MCP_CONTEXT7_KEY → "ctx7sk-..."
dispatch.project_env = {
  "RENTL_OPENROUTER_API_KEY": "sk-or-v1",  combines into SecretBundle:
  "RENTL_QUALITY_MODEL": "qwen...",           developer = resolved secrets
}                                             project = dispatch.project_env
                                              infrastructure = {GIT_TOKEN}

                                            injects onto VM:
                                              /workspace/.developer-secrets
                                              /workspace/project/.env
                                              ~/.claude/.credentials.json
                                              ~/.local/share/opencode/auth.json
```

**Key property: secret values never in dispatch payload or database.**

## Who Gets What

| Secret | In daemon.env | In required_secrets | In project_env | On VM |
|--------|--------------|---------------------|----------------|-------|
| `HCLOUD_TOKEN` | YES | NO | NO | NO |
| `GIT_TOKEN` | YES | NO | NO | Via GIT_ASKPASS |
| `CLAUDE_CODE_OAUTH_TOKEN` | YES | YES | NO | .developer-secrets |
| `OPENCODE_ZAI_API_KEY` | YES | YES | NO | auth.json |
| `RENTL_OPENROUTER_API_KEY` | NO | NO | YES | .env |
| `RENTL_QUALITY_MODEL` | NO | NO | YES | .env |

## Rotation Guide

| Category | How to rotate |
|----------|--------------|
| Infrastructure | Update `daemon.env`, restart daemon |
| Developer | Update `daemon.env`; daemon reads `os.environ` on each dispatch |
| Project | Update project `.env`; next dispatch picks it up automatically |
| Operational | Update `api.env`/`daemon.env`, restart services |

## Security Properties

- Infrastructure secrets never leave the daemon process
- Developer secret values never appear in dispatch payloads or the database
  (only names via `required_secrets`)
- Project env values appear in dispatch payloads (stored in event store
  with same access controls as daemon)
- All VM-injected secrets are cleaned up in teardown
- `GIT_TOKEN` uses `GIT_ASKPASS` — never in process args
- CLI credential files are chmod 600 on VMs
- Secrets directory is chmod 700

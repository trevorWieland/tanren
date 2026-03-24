# Tiered Test Plan

Manual validation procedure for tanren dispatch lifecycle.

## Prerequisites

- rentl repo at `~/github/rentl` with branch `tanren-test-validation`
- `tanren.yml` with `local-dev` (type: local) and `default` (type: remote) profiles
- Test spec at `tanren/specs/tanren-validation-test/`
- `~/.config/tanren/secrets.env` with all developer + infrastructure secrets
- `rentl/.env` with all project secrets
- `daemon.env` with infrastructure + developer secrets (Docker deployments)
- `make check` passes before starting manual tests

## Tier 1: Pipeline Mechanics

Core lifecycle — each test validates one piece of the pipeline.

| # | Test | Command | Expected | Local | Remote |
|---|------|---------|----------|-------|--------|
| 1a | Secret injection | gate: `printenv RENTL_OPENROUTER_API_KEY \| head -c 10` | success, exit 0 | `tanren run execute --phase gate --gate-cmd ...` | `POST /run/{id}/execute` |
| 1a-neg | Missing var | gate: `printenv FAKE_VAR_XYZ` | fail, exit 1 | same | same |
| 1b | Gate command | gate: `make check` | success, exit 0 | same | same |
| 1c | Auth CLI exec | do-task with claude | success, signal complete | `tanren run execute --phase do-task` | `POST /run/{id}/execute` |
| 1d | Push | verify on remote branch | pushed: true | `git fetch + log` | SSH into VM + check |
| 1e | Teardown | teardown dispatch | worktree/VM removed | `tanren run teardown` | `POST /run/{id}/teardown` |
| 1f | CLI auth | claude/codex/opencode hello | all respond | local only | n/a |

### Remote 1c: Claude Authentication

This is the primary validation that reference-based secret injection works:

1. `daemon.env` contains `CLAUDE_CODE_OAUTH_TOKEN=sk-ant-oat01-...`
2. CLI resolves `required_secrets = ("CLAUDE_CODE_OAUTH_TOKEN", "CLAUDE_CREDENTIALS_JSON")`
3. Daemon resolves from `os.environ`, injects onto VM as `.developer-secrets`
4. Claude Code starts, reads token, authenticates
5. Verify: no secret values in dispatch_json or event payloads in the database

## Tier 2: Concurrency

Using rentl + unicorn-armada (or a second project).

| # | Test | What it proves |
|---|------|---------------|
| 2a | Two rentl worktrees, different branches | Worktree isolation, registry conflict prevention |
| 2b | rentl + unicorn-armada in parallel (local) | Multi-project, env factory builds correct env per dispatch |
| 2c | Two remote dispatches: different compute configs | One daemon serves both, truly stateless |

## Tier 3: Error Recovery

| # | Test | What it proves |
|---|------|---------------|
| 3a | Execute fails → teardown runs (AUTO mode) | No orphaned VMs |
| 3b | Gate fails → FAILED outcome | Correct outcome mapping |
| 3c | Missing required secret → provision logs warning | Fail-fast on validation |

## Tier 4: Full Autonomous Runs

| # | Test | What it proves |
|---|------|---------------|
| 4a | `tanren run full` local | Full lifecycle in one command |
| 4b | API auto-chain remote | Provision → execute → teardown |
| 4c | Local + remote in parallel | Mixed environments simultaneously |

## Verification Checklist

After all tiers pass:

- [ ] `make check` passes
- [ ] Local Tier 1 1a-1e pass
- [ ] Remote Tier 1 1c: claude authenticates on VM
- [ ] `daemon.env` has all injectable secrets
- [ ] Database query shows no secret values in dispatch_json or event payloads:
  ```sql
  SELECT dispatch_json FROM dispatch_projections
  WHERE dispatch_json LIKE '%sk-ant-%';
  -- Expected: 0 rows
  ```

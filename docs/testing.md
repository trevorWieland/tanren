# Tiered Test Plan

Manual validation procedure for tanren dispatch lifecycle.

## Prerequisites

- tanren repo checked out, `make check` passes
- rentl repo at `~/github/rentl` with `tanren.yml` containing `environment` section
  (`local-dev`, `default`, `remote-small` profiles) on `main`
- unicorn-armada at `~/github/unicorn-armada` with `tanren.yml` containing
  `environment` section on `master`
- Test spec at `tanren/specs/tanren-validation-test/` in both projects
- Updated tanren commands in both projects' `.claude/commands/tanren/`
  (copy from `tanren/commands/`)
- `~/.config/tanren/secrets.env` (chmod 600) with:
  `HCLOUD_TOKEN`, `GIT_TOKEN`, `CLAUDE_CODE_OAUTH_TOKEN`,
  `OPENCODE_ZAI_API_KEY`, `MCP_CONTEXT7_KEY`
- `~/.config/tanren/tanren.env` with all `WM_*` variables
- `~/.config/tanren/remote.yml` with Hetzner provisioner config
- `~/.config/tanren/roles.yml` with CLI role mappings
- `rentl/.env` with project env vars (`RENTL_OPENROUTER_API_KEY`, etc.)

### Shell setup (required before every test command)

```bash
cd ~/github/tanren
set -a && source ~/.config/tanren/tanren.env && set +a
# For remote tests, also source secrets:
source ~/.config/tanren/secrets.env
```

## Tier 1: Pipeline Mechanics

### T1a: Secret injection (local)

```bash
uv run tanren run full --project rentl --branch tanren-test-validation \
  --spec-path tanren/specs/tanren-validation-test --phase gate \
  --environment-profile local-dev \
  --gate-cmd 'printenv RENTL_OPENROUTER_API_KEY | head -c 10' --timeout 60
```

Expected: `outcome: success`, `exit_code: 0`

### T1a-neg: Missing var (local)

```bash
uv run tanren run full --project rentl --branch tanren-test-validation \
  --spec-path tanren/specs/tanren-validation-test --phase gate \
  --environment-profile local-dev \
  --gate-cmd 'printenv FAKE_VAR_XYZ' --timeout 60
```

Expected: `outcome: fail`, `exit_code: 1`

### T1b: Gate command (local)

```bash
uv run tanren run full --project rentl --branch tanren-test-validation \
  --spec-path tanren/specs/tanren-validation-test --phase gate \
  --environment-profile local-dev \
  --gate-cmd 'make check' --timeout 300
```

Expected: `outcome: success`, `exit_code: 0`

### T1-remote: Provision + execute + teardown

```bash
# Provision
uv run tanren run provision --project rentl --branch tanren-test-validation \
  --environment-profile default
# → Note dispatch_id and host

# T1a-remote: secret injection
uv run tanren run execute --dispatch-id $DISPATCH_ID --project rentl \
  --spec-path tanren/specs/tanren-validation-test --phase gate \
  --gate-cmd 'printenv RENTL_OPENROUTER_API_KEY | head -c 10' --timeout 60
# Expected: outcome: success

# T1a-neg-remote: missing var
uv run tanren run execute --dispatch-id $DISPATCH_ID --project rentl \
  --spec-path tanren/specs/tanren-validation-test --phase gate \
  --gate-cmd 'printenv FAKE_VAR_XYZ' --timeout 60
# Expected: outcome: fail

# T1b-remote: make check
uv run tanren run execute --dispatch-id $DISPATCH_ID --project rentl \
  --spec-path tanren/specs/tanren-validation-test --phase gate \
  --gate-cmd 'make check' --timeout 300
# Expected: outcome: success

# T1c: Claude auth exec
uv run tanren run execute --dispatch-id $DISPATCH_ID --project rentl \
  --spec-path tanren/specs/tanren-validation-test --phase do-task --timeout 300
# Expected: outcome: success, signal: all-done
# Verify: .agent-status file written on VM, result.txt created

# T1d: Verify push
git fetch origin tanren-test-validation
git log origin/tanren-test-validation -1
# Expected: agent commit visible

# T1e: Teardown
uv run tanren run teardown --dispatch-id $DISPATCH_ID
# Expected: teardown: completed, VM unreachable
```

### T1-db-audit: No secrets in payload

```bash
uv run python3 -c "
import asyncio, os
from tanren_core.store.factory import create_sqlite_store
from pathlib import Path
from dotenv import dotenv_values

async def audit():
    db = str(Path(os.environ['WM_DATA_DIR']) / 'run.db')
    store = await create_sqlite_store(db)
    from tanren_core.store.views import DispatchListFilter
    dispatches = await store.query_dispatches(DispatchListFilter(limit=5))
    secrets = dotenv_values(os.path.expanduser('~/.config/tanren/secrets.env'))
    for d in dispatches:
        raw = d.dispatch.model_dump_json()
        for key, val in secrets.items():
            if val and val in raw:
                print(f'LEAKED: {key} value found in {d.dispatch_id}')
                return
    print('PASS: no secret values in any dispatch payload')
    await store.close()

asyncio.run(audit())
"
```

## Tier 2: Concurrency

### T2a: Two rentl worktrees, different branches (local)

Run two gates in parallel on different branches. Both must succeed.
Verify worktree registry is empty after both complete.

```bash
uv run tanren run full --project rentl --branch tanren-test-validation \
  --spec-path tanren/specs/tanren-validation-test --phase gate \
  --environment-profile local-dev --gate-cmd 'echo branch1' --timeout 60 &

uv run tanren run full --project rentl --branch tanren-test-validation-2 \
  --spec-path tanren/specs/tanren-validation-test --phase gate \
  --environment-profile local-dev --gate-cmd 'echo branch2' --timeout 60 &

wait
cat $WM_WORKTREE_REGISTRY_PATH  # Should be {"worktrees":{}}
```

### T2b: rentl + unicorn-armada in parallel (local)

Run gates on two different projects simultaneously. Verifies env factory
builds correct environment per project.

```bash
uv run tanren run full --project rentl --branch tanren-test-validation-2 \
  --spec-path tanren/specs/tanren-validation-test --phase gate \
  --environment-profile local-dev \
  --gate-cmd 'echo project=rentl && printenv RENTL_OPENROUTER_API_KEY | head -c 10' \
  --timeout 60 &

uv run tanren run full --project unicorn-armada --branch tanren-test-validation \
  --spec-path tanren/specs/tanren-validation-test --phase gate \
  --environment-profile local-dev --gate-cmd 'echo project=ua && make all' \
  --timeout 120 &

wait
```

**Important:** Both projects must have `environment` section in their
`tanren.yml` on the branch that's checked out in `WM_GITHUB_DIR/<project>/`.

### T2c: Two remote dispatches in parallel

Provision two VMs, run gates, teardown both.

```bash
uv run tanren run provision --project rentl --branch tanren-test-validation \
  --environment-profile remote-small &
uv run tanren run provision --project rentl --branch tanren-test-validation-2 \
  --environment-profile remote-small &
wait
# Note both dispatch_ids, then execute gates and teardown on each
```

## Tier 3: Error Recovery

### T3a: Execute fails → teardown runs (remote AUTO)

```bash
uv run tanren run full --project rentl --branch tanren-test-validation \
  --spec-path tanren/specs/tanren-validation-test --phase gate \
  --environment-profile remote-small \
  --gate-cmd 'echo "about to fail" && exit 1' --timeout 60
```

Expected: `outcome: fail`, `teardown: completed`, VM unreachable after.

### T3b: Gate fails → FAILED outcome

Already covered by T1a-neg (local + remote).

### T3c: Missing CLI auth → provision fails hard

Remove or rename `~/.config/tanren/secrets.env`, set only infrastructure
secrets (`HCLOUD_TOKEN`, `GIT_TOKEN`) in shell env, then:

```bash
uv run tanren run full --project rentl --branch tanren-test-validation \
  --spec-path tanren/specs/tanren-validation-test --phase do-task \
  --environment-profile remote-small --timeout 60
```

Expected: `failed terminally: No auth secret resolved for {cli}`,
teardown still runs. No wasted VM execution time.

**Note:** The CLI shown in the error comes from `roles.yml` role mapping,
not the `--phase` flag directly. `do-task` → `implementation` role →
whichever CLI is configured for that role.

## Tier 4: Full Autonomous Runs

| # | Test | What it proves |
|---|------|---------------|
| 4a | `tanren run full` local | Full lifecycle in one command |
| 4b | API auto-chain remote | Provision → execute → teardown |
| 4c | Local + remote in parallel | Mixed environments simultaneously |

## Troubleshooting

### Stale worktree registry

If you see `Branch X already in use by workflow Y`, check and clear:

```bash
cat $WM_WORKTREE_REGISTRY_PATH
echo '{"worktrees":{}}' > $WM_WORKTREE_REGISTRY_PATH
# Also clean up stale git worktrees:
cd ~/github/<project> && git worktree list
git worktree remove /path/to/stale-wt --force
```

### secrets.env permissions

```bash
chmod 600 ~/.config/tanren/secrets.env
```

### Hetzner resource unavailable

If `cpx41` is unavailable, use `remote-small` profile (`cpx21`) or check
available server types at the configured location.

# Bootstrap Guide

Use this flow to install tanren into an existing project and prepare the first
spec run.

## 1. Install Tanren Assets

```bash
cd /path/to/project
~/path/to/tanren/scripts/install.sh --profile python-uv
```

Installation adds:

- `.claude/commands/tanren/` and `.opencode/commands/tanren/`
- `tanren/standards/`
- `tanren/product/`
- `tanren/scripts/`
- `Makefile` gates (if missing)

## 2. One-Time Knowledge Bootstrap

Run once per project:

1. `plan-product`
2. `discover-standards`
3. `inject-standards`
4. `index-standards`

These steps establish the product and coding context used by all future agent
sessions.

## 3. Configure Execution

Define project/runtime config:

- `tanren.yml`: env requirements, environment profiles, gate commands
- `remote.yml` (optional): remote provider, SSH, workspace, secrets loading
- `roles.yml` (optional): role to CLI/model mapping

Verify with:

```bash
tanren env check
tanren vm dry-run --project my-project --environment-profile default
```

## 4. Execute First Lifecycle

```bash
tanren run full --project my-project --spec-path tanren/specs/s0001 --phase do-task
```

For full state semantics and protocol details, see `protocol/PROTOCOL.md`.

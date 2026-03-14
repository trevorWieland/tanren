# Contributing

Thanks for contributing to tanren.

## Development Setup

All commands run from the repository root.

```bash
uv sync
make check
```

For full validation before PR:

```bash
make format
make check
make ci
```

If your changes touch SSH or local environment flows, also run the relevant
integration target (`make integration-ssh` or `make integration-local`).

## Repository Areas

- `packages/tanren-core/`: core library
- `services/`: API, CLI, daemon services
- `services/tanren-api/`: HTTP API (FastAPI)
- `commands/`: workflow instructions used by agents
- `profiles/`: coding standards by stack
- `protocol/`: IPC wire contract
- `docs/`: canonical deep-dive documentation

## Commit Style

Use imperative commit subjects with Conventional Commit prefixes when possible:

- `feat(core): add ...`
- `feat(api): add ...`
- `fix: correct ...`
- `chore: update ...`

## Pull Request Requirements

Each PR should include:

- clear problem statement and implementation summary
- validation commands run and outcomes
- config/security impact notes (if applicable)
- linked issue/spec IDs

## Documentation Update Rule

If behavior or interfaces change, update docs in the same PR.

1. Update the canonical page in `docs/` (or `protocol/PROTOCOL.md` for wire contracts).
2. Update summaries/links in `README.md`, `docs/worker-manager-README.md`, or `AGENTS.md` only if needed.
3. Avoid diverging duplicate explanations across files.

For migration and source-of-truth context, see `docs/hld-migration-map.md` and
`docs/README.md`.

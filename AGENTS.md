# Repository Guidelines

This is a uv python monorepo, and thus you should ONLY ever use uv for python execution.
Never rely on running individual tests, targeted files for linting, etc. for validating your changes, use FULL suites.

## Project Structure & Module Organization
- Root folders: `commands/`, `profiles/`, `templates/`, `protocol/`, `scripts/`, `packages/`, and `services/`.
- Core library lives in `packages/tanren-core/src/tanren_core/`.
- Services live in `services/tanren-{api,daemon,cli}/`.
- Tests are under `tests/unit/` and `tests/integration/`.
- Keep new core modules in `packages/tanren-core/src/tanren_core/` and mirror test placement (for example, `tanren_core/ipc.py` -> `tests/unit/test_ipc.py`).

## Build, Test, and Development Commands
Unless explicitly stated otherwise, run all build/test/dev commands from the repo root.
- Clean slate setup:
  1. `uv sync`
  2. `make check`
- Change validation flow:
  1. `make format`
  2. `make check`
  3. `make ci`
- Additional suites when relevant: `make integration-ssh` for SSH-path changes, `make integration-local` for local environment workflow changes.
- From repo root: `scripts/install.sh --profile python-uv` installs tanren commands/standards into a project.

## Coding Style & Naming Conventions
- Python style is enforced by Ruff (`line-length = 100`, target `py314`) and Ty.
- Use 4-space indentation and explicit type hints for public functions.
- Naming rules: modules/functions/variables `snake_case`, classes `PascalCase`, CLI commands `kebab-case`, CLI flags `--snake-case`.
- Prefer small adapter-oriented modules; place protocol boundaries in `adapters/protocols.py` patterns.

## Testing Guidelines
- Framework: `pytest` with `pytest-asyncio`, `pytest-timeout`, and coverage reporting.
- Coverage gate is enforced at `--cov-fail-under=80`; new behavior should include happy-path, error-path, and edge-case tests.
- Name tests `test_*.py`; keep fast isolated checks in `tests/unit/` and environment-dependent flows in `tests/integration/`.
- Use markers intentionally (`ssh`, `local_env`) and avoid enabling them in default CI runs.

## Documentation Source of Truth
- Deep-dive docs live under `docs/`; root `README.md` should summarize and link.
- IPC wire contract changes are canonical in `protocol/PROTOCOL.md` (not duplicated elsewhere).
- Runtime implementation details are canonical in `docs/ADAPTERS.md`.
- If behavior, interfaces, lifecycle, or security model changes, update the relevant canonical doc in the same PR.
- Use `docs/hld-migration-map.md` as the topic coverage index; avoid creating disconnected duplicate explanations.

## Commit & Pull Request Guidelines
- Follow the observed commit style: Conventional Commit prefixes such as `feat(core): ...`, `feat(api): ...`, `fix: ...`, `chore: ...`.
- Keep subjects imperative and scoped to one change set.
- Acceptable PR gate: both `make check` and `make ci` must pass locally before requesting review.
- PRs should include: concise problem statement, implementation summary, executed command results, and any config/secret implications.
- Link related issue/spec IDs and include logs or terminal snippets when behavior changes are non-trivial.

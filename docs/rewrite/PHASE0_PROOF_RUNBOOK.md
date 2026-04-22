# Phase 0 Proof Runbook

## Purpose

Run and verify a reproducible Phase 0 proof pack that maps every BDD scenario
(`1.1` through `8.1`) to both positive and falsification evidence.

Canonical BDD source: `docs/rewrite/PHASE0_PROOF_BDD.md`.

---

## Prerequisites

- Repo is checked out at the desired commit.
- Rust toolchain and `cargo-nextest` are installed (same requirements as `just ci`).
- `tanren-cli` and `tanren-mcp` are installed and PATH-callable.
- Python runtime is available through `uv` (for proof helper scripts).
- `jq` is installed and PATH-callable.

Recommended bootstrap from repo root:

```bash
uv sync
scripts/runtime/install-runtime.sh
scripts/runtime/verify-installed-runtime.sh
```

---

## One-Command Proof Collection

From repo root:

```bash
scripts/proof/phase0/run.sh
```

Default output location:

- `artifacts/phase0-proof/<timestamp>/`

Optional flags:

```bash
scripts/proof/phase0/run.sh --output-root /tmp/phase0-proof --timestamp 20260420T120000Z
scripts/proof/phase0/run.sh --skip-verify
```

---

## Verification

Run explicit verification against an existing pack:

```bash
scripts/proof/phase0/verify.sh artifacts/phase0-proof/<timestamp>
```

`verify.sh` fails non-zero when any of these are missing or invalid:

- `summary.json` / `summary.md`
- `runtime/installed-runtime.json` with `runtime_ok=true`
- positive/falsification `PASS` markers for every scenario `1.1` through `8.1`
- auth/replay supplemental reports
- replay parity + rollback verdicts
- manual walkthrough summary

---

## Phase 0 Orchestration Entry Point

Phase 0 now ships a reusable, non-Python orchestration entrypoint:

```bash
scripts/orchestration/phase0.sh --spec-id <spec-uuid> --spec-folder <spec-folder>
```

Behavior contract:

- Resumes from store truth via `tanren-cli methodology spec status`.
- Autonomous loop runs Phase 0 task/spec gates and agentic phases through
  Codex harness invocation (`codex exec`) only.
- Harness override mode is intentionally disabled in acceptance flow
  (`--harness-cmd` and `TANREN_PHASE0_HARNESS_CMD` are rejected).
- Breaks out only at required human checkpoints:
  - missing spec -> prompt `shape-spec`
  - blocker halt -> prompt `resolve-blockers`
  - walk-ready -> prompt `walk-spec`
- Uses hook commands resolved from `tanren.yml` (task/spec/per-phase hooks)
  and records resolved config in run artifacts under:
  `<spec-folder>/orchestration/phase0/<timestamp>/`.
- Supports output modes via `--output-mode` (or `TANREN_PHASE0_OUTPUT_MODE`):
  - `silent` (default): single-line high-level progress (`task N/M - <verb>`).
  - `quiet`: task definition + deliverable summary + high-level action/result.
  - `verbose`: full command/harness logging.

Recommended operator flow:

1. Manually shape the spec (`shape-spec`).
2. Run `scripts/orchestration/phase0.sh ...`.
3. When prompted for walk readiness, manually run `walk-spec`.

---

## Artifact Layout

```text
artifacts/phase0-proof/<timestamp>/
  summary.json
  summary.md
  scenarios/
    <scenario-id>/
      positive/
      falsification/
  auth-replay/
    summary.json
  replay-pack/
    source-event-stream.jsonl
    target-task-list.json
    verdicts/
      equivalence.json
      rollback.json
  manual-walkthrough/
    phase-events.jsonl
    summary.json
    steps/
```

---

## Reading Results

1. Open `summary.md` for a human-readable matrix.
2. Open `summary.json` for machine-readable status.
3. For any failing scenario, inspect:
   - `scenarios/<id>/<kind>/command.txt`
   - `scenarios/<id>/<kind>/stdout.log`
   - `scenarios/<id>/<kind>/stderr.log`

Replay-specific interpretation:

- `replay-pack/verdicts/equivalence.json`: source vs replayed state parity.
- `replay-pack/verdicts/rollback.json`: invalid replay rejection and no-partial-apply guarantee.

---

## Token Troubleshooting

Token helper:

```bash
uv run python scripts/proof/phase0/mint_actor_token.py \
  --private-key-pem <path> \
  --issuer tanren-phase0-proof \
  --audience tanren-cli \
  --org-id 00000000-0000-0000-0000-0000000000a1 \
  --user-id 00000000-0000-0000-0000-0000000000b1 \
  --mode valid \
  --requested-ttl 600 \
  --max-ttl 900 \
  --token-only
```

The utility always prints computed token math to stderr:

- `iat`
- `exp`
- `exp_minus_iat`
- configured `max_ttl`

Invariant enforced:

- non-`ttl_over_max` modes fail when `exp_minus_iat > actor_token_max_ttl_secs`

Operational rule:

- derive `exp` from `iat` (`exp = iat + requested_ttl`), not from independent wall-clock math.

---

## CI + Ship Gate

After proof pack generation and verification:

```bash
just ci
lefthook run pre-commit
```

Then stage/commit/push with a clean `git status`.

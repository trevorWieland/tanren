# Tanren Behavior Features

Active behavior proof lives under `tests/bdd/features/`. The canonical
convention is documented in
[`docs/architecture/subsystems/behavior-proof.md`](../../docs/architecture/subsystems/behavior-proof.md)
under "BDD Tagging And File Convention". Read that section before adding
or editing a `.feature` file — it is the contract every R-* slice must
match.

## Quick reference

- One file per behavior:
  `tests/bdd/features/B-XXXX-<slug>.feature`.
- Feature-level tag: exactly `@B-XXXX` matching the filename.
- Each scenario carries exactly one of `@positive` / `@falsification`
  and 1–2 interface tags from `@web | @api | @mcp | @cli | @tui`.
- Two-interface scenarios require `# rationale: <one line>` immediately
  above the scenario's tags.
- `Scenario Outline` and `Examples:` are forbidden. `Background:` and
  `Rule:` are allowed.
- Closed tag allowlist — `@skip`, `@wip`, `@ignore`, and phase/wave
  tags are rejected.
- Coverage is strict-equality: every interface in the behavior's
  frontmatter `interfaces:` must have a `@positive` scenario, and a
  `@falsification` scenario when the R-* node lists falsification
  witnesses.

## Validators

```bash
# Tag and coverage validator (file → behavior → DAG):
just check-bdd-tags

# Inverse check (orphan feature files / DAG drift):
python3 scripts/roadmap_check.py
```

`just check` runs both as part of the standard PR gate. `just tests`
runs the cucumber-rs harness and the BDD runner binary; with zero
feature files shipped under F-0001/F-0002 it exits 0 with no scenarios.
Mutation testing is intentionally separated into `just mutation` and
nightly CI.

## Current CLI Install And Standards Runtime Behavior Slices

Repository install and runtime standards behavior is currently covered by:

- `tests/bdd/features/B-0068-bootstrap-tanren-assets.feature`
- `tests/bdd/features/B-0070-generate-selected-agent-integrations.feature`
- `tests/bdd/features/B-0071-load-runtime-standards-root.feature`

These slices assert the concrete `tanren-cli install` surface:

- required `--profile` (current value: `rust-cargo`);
- optional `--repo` defaulting to current directory;
- optional `--integrations` with `claude`, `codex`, `open-code` (default: all);
- generated command assets per selected integrations;
- standards files installed and preserved by policy;
- stale manifest-tracked generated assets removed;
- invalid profile/integration inputs fail before repository writes.

`B-0071` additionally asserts the concrete `tanren-cli standards inspect`
surface:

- success output starts with `status=ok command=standards.inspect`;
- output reports configured `standards_root` metadata from
  `.tanren/project-methodology.toml`;
- missing configured standards root fails with
  `error: standards_missing - configured standards root is missing: '<path>'`;
- malformed standards frontmatter fails with
  `error: standards_parse_failed - failed to parse standards frontmatter in '<path>': <reason>`.

Keep scope boundaries explicit in new BDD edits:

- install materialization and validation belong here;
- drift detection/remediation, upgrade, and uninstall are separate nodes and
  should not be folded into install-proof scenarios in this slice.

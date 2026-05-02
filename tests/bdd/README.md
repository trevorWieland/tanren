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

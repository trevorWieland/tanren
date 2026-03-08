# Make All Gate

`make all` is the local verification gate. `make ci` is the CI PR gate.

Gate tiers:
- `make check` — Task gate (format, lint, type, unit)
- `make ci` — CI PR gate (format, lint, type, unit, integration)
- `make all` — Spec gate (format, lint, type, unit, integration, quality)

Rules:
- Agents run `make all` locally before finalizing work (agents have API keys)
- CI runs `make ci` on PRs — quality tests require paid API keys unavailable in CI
- `make all` fails hard if quality tests can't run (no silent skipping)
- Fix failures and re-run until green
- Do not skip steps or substitute partial commands
- Human feedback on gate scope overrides spec non-negotiables

# Scenario Gates

`make check`, `make ci`, and `make all` must execute scenario suites, not mixed free-form tests.

Gate tiers:
- `make check` — unit + integration BDD scenario gates
- `make ci` — full CI BDD scenario gates (+ mutation gate when configured)
- `make all` — local full gate including quality scenarios

**Rules:**
- Gate commands run `.feature`-backed suites for all enabled tiers
- CI must fail if behavior scenarios are missing for changed behavior IDs
- CI must fail if mutation gate configured by the profile fails
- CI must fail if scenario-only coverage gate fails
- No bypassing scenario suites with ad-hoc direct test commands

**Why:** Gate outcomes should answer one question: did declared behaviors execute and pass?

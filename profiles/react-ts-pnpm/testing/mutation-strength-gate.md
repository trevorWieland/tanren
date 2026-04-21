# Mutation Strength Gate (StrykerJS)

Mutation testing is required to prove that behavior scenarios are robust.

```bash
# Example command shape
pnpm stryker run
```

**Rules:**
- CI must run Stryker for the profile-defined scope
- Surviving mutants require scenario or step-strength improvements, or explicit rationale
- Mutation score regressions fail CI
- Mutation review should reference affected behavior IDs

**Why:** Mutation testing detects weak tests that still pass despite meaningful logic faults.

# Mutation Strength Gate (mutmut)

Mutation testing is required to verify that behavior scenarios actually detect faults.

```bash
# Example command shape (project-specific wrapper may differ)
uv run mutmut run
uv run mutmut results
```

**Rules:**
- CI must run `mutmut` for the profile-defined scope
- Surviving mutants must be triaged and either fixed with new scenarios/steps or explicitly justified
- Mutation regressions fail CI
- Mutation evidence must reference impacted behavior IDs when possible

**Why:** Passing scenarios are only strong if they fail under realistic code mutations.

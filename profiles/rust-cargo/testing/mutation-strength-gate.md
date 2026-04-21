# Mutation Strength Gate (cargo-mutants)

`cargo-mutants` is required to validate scenario effectiveness.

```bash
# Example command shape
cargo mutants --workspace
```

**Rules:**
- CI must run `cargo-mutants` for the profile-defined scope
- Surviving mutants require either new/improved scenarios or explicit rationale
- Mutation regressions fail CI
- Mutation findings should reference affected behavior IDs

**Why:** Mutation testing catches weak scenarios that pass without actually protecting behavior.

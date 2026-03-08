# Validate Generated Artifacts Against Target Schemas

When code produces data consumed by another component, test it against the consuming schema — not just syntax.

## Rule

If a function generates config, data files, fixtures, or any artifact that another component will parse, the test must:

1. Generate the artifact
2. Validate it against the consuming component's schema (e.g., `RunConfig.model_validate()`, `SourceLine.model_validate()`)
3. Verify path references resolve correctly (file paths, input_path, output_dir)

## Examples

```python
# BAD: only checks syntax
config = generate_project(answers, target_dir)
assert (target_dir / "config.toml").exists()

# GOOD: validates against consuming schema
config = generate_project(answers, target_dir)
data = tomllib.loads((target_dir / "config.toml").read_text())
validated = RunConfig.model_validate(data)  # schema validation

# GOOD: verifies path alignment
assert (target_dir / validated.project.paths.input_path).exists()  # path resolves
```

## Common Failures

- Field names don't match schema (e.g., `original_text` vs `text`)
- ID patterns don't match regex validators (e.g., `"001"` vs `"line_001"`)
- File paths in config don't match generated file locations
- Schema validation passes but runtime execution fails (validate + execute)

## Why

Syntactic tests (file exists, TOML parses) miss semantic mismatches. The consuming component's schema is the contract — test against it directly.

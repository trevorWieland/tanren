# Validate Generated Artifacts Against Consuming Schemas

When a behavior scenario generates an artifact consumed elsewhere, validate against the real consumer schema.

```gherkin
@behavior(BEH-CONFIG-004) @tier(integration)
Scenario: Generated config is consumable by runtime
  Given a generated project config artifact
  When the runtime loads that config
  Then schema validation succeeds
  And all referenced paths resolve
```

**Rules:**
- Validate generated artifacts with the consuming schema type
- Assert path/reference alignment, not only parse validity
- Keep this validation in behavior scenarios tied to behavior IDs

**Why:** "File exists" checks are weak; scenario proof requires real consumer compatibility.

# Mock at the Execution Boundary

In BDD suites, mocks are allowed only where a scenario crosses an external execution boundary.

```gherkin
@behavior(BEH-PIPE-011) @tier(integration)
Scenario: Pipeline retries failed model call
  Given the model boundary returns a transient failure once
  When the pipeline executes
  Then the second attempt succeeds
```

```python
# Step binding patches the boundary that executes the side effect.
@given("the model boundary returns a transient failure once")
def model_boundary_retry(monkeypatch):
    calls = {"count": 0}

    async def boundary_call(*_args, **_kwargs):
        calls["count"] += 1
        if calls["count"] == 1:
            raise RuntimeError("transient")
        return {"ok": True}

    monkeypatch.setattr("myproj.adapters.model_client.ModelClient.generate", boundary_call)
    return calls
```

**Rules:**
- Patch at the last internal boundary before the external side effect
- Do not mock internal helpers to make scenarios pass
- Verify boundary mocks were exercised
- Quality tier scenarios use no model mocks

**Why:** Scenario truth depends on realistic execution flow, not internal stubbing.

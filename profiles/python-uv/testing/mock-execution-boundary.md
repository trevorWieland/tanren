# Mock at the Execution Boundary

Mock where execution actually happens, not where you think it happens. Verify the mock was invoked.

## Rule

Before writing a mock, trace the call path from your test entry point to the real side-effect. Patch at the last internal boundary before the external call.

## Per-Tier Boundaries

- **Unit tests** — Mock direct dependencies (e.g., function args, injected services). Can mock low-level internals.
- **Integration tests** — Mock at `ProfileAgent.run()` for agent execution. Never mock pydantic-ai internals or CLI factory functions (`_build_llm_runtime`). Return schema-valid output matching the agent's `_output_type`.
- **Quality tests** — No mocks. Real LLMs only (see `no-mocks-for-quality-tests`).

## Agent Mock Pattern

```python
async def mock_agent_run(self: ProfileAgent, payload: object) -> object:
    call_count["count"] += 1
    output_type = self._output_type
    if output_type == SceneSummary:
        return SceneSummary(scene_id="scene_001", summary="...", characters=[])
    # ... per-agent-type returns
```

## Verification

Always assert the mock was called:

```python
call_count = {"count": 0}
# ... run test ...
assert call_count["count"] > 0, "Mock was never invoked"
```

## Why

Patching the wrong boundary (e.g., a factory function) leaves the real execution path untouched. Tests pass on setup but fail on execution, creating confusing audit loops.

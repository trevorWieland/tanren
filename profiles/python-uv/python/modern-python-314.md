# Modern Python 3.14

Use Python 3.14 features and patterns. Avoid legacy constructs.

```python
# ✓ Good: Modern Python 3.14
# Type unions (Python 3.10+)
result: str | None

# Pattern matching (Python 3.10+)
match phase:
    case "translate":
        await translate()
    case "qa":
        await run_qa()
    case _:
        raise ValueError(f"Unknown phase: {phase}")

# Dictionary union operators (Python 3.9+)
config = base_config | custom_config

# f-string debugging (Python 3.8+)
print(f"{result=}")  # Prints: result='value'

# Async generators (Python 3.6+)
async def stream_translations():
    for scene in scenes:
        yield await translate_scene(scene)

# ✗ Bad: Legacy patterns
result: Optional[str]  # Old-style Union

if phase == "translate":
    await translate()
elif phase == "qa":
    await run_qa()
else:
    raise ValueError(f"Unknown phase: {phase}")  # No pattern matching

config = {**base_config, **custom_config}  # Old unpacking

print(f"result={result}")  # No f-string debugging

def stream_translations():  # Blocking generator
    for scene in scenes:
        yield translate_scene(scene)
```

**Modern Python 3.14 features to use:**
- Type unions: `str | None` instead of `Union[str, None]`
- Pattern matching: `match/case` instead of if/elif chains
- Dictionary union: `dict1 | dict2` instead of unpacking
- f-string debugging: `f"{var=}"` instead of `f"var={var}"`
- Walrus operator: `if (match := pattern.search(text)):` for assignment in expressions
- Async generators: `async def` and `async for`
- Context managers: `with` statements for resource management
- Dataclass improvements: `dataclass_transform` and field parameters

**Legacy patterns to avoid:**
- `Optional[T]`, `Union[T, U]` - use `T | None`, `T | U`
- `if/elif/else` chains for type/state - use `match/case`
- `{**d1, **d2}` - use `d1 | d2`
- `f"var={var}"` - use `f"{var=}"`
- `typing.List`, `typing.Dict` - use built-in `list`, `dict`
- `yield` without `async` - use async generators where appropriate

**Why:** Cleaner syntax, better expressiveness, fewer lines of code, performance improvements, optimized standard library features, and avoids learning outdated patterns.

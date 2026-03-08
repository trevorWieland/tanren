# Enum Field Runtime Types

`BaseSchema` uses `use_enum_values=True`. Enum fields are stored as their primitive
type (`str` for `StrEnum`, `int` for `IntEnum`) at runtime — not as enum instances.

Never call `.value` on fields read from Pydantic models.

```python
class MyConfig(BaseSchema):
    phase: PhaseName = Field(...)  # Annotation says PhaseName...

config = MyConfig(phase=PhaseName.TRANSLATE)
# config.phase is "translate" (str), NOT PhaseName.TRANSLATE

# ✗ Bad: crashes with AttributeError
path = phases_dir / f"{config.phase.value}.toml"

# ✓ Good: field is already a str
path = phases_dir / f"{config.phase}.toml"
```

- Use enum fields directly — they are already the primitive value
- Type function parameters as the primitive (`str`, `int`) when receiving
  values from Pydantic models, not as the enum type
- If you must reconstruct the enum, cast explicitly: `PhaseName(config.phase)`

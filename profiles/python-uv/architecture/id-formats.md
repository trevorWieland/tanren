# ID Formats

Use UUIDv7 for internal, non-human IDs. Use `{word}_{number}` for human-readable IDs.

Examples:
- UUIDv7: `01890a5c-91c8-7b2a-9f51-9b40d0cfb5b0`
- Human: `line_42`, `scene_7`, `run_3`

Rules:
- Internal IDs (runs, artifacts, notes, issues) must be UUIDv7
- Human-readable IDs (line_id, scene_id) must match `{word}_{number}`
- External engine IDs should be mapped to internal IDs and stored in metadata

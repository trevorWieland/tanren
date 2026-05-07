---
schema: tanren.design_surface_adapter.v0
surface_kind: command_line
status: draft
owner_command: define-design-system
updated_at: 2026-05-07
---

# CLI Adapter

The CLI adapter projects shared design-system records into command grammar,
stdout, stderr, exit codes, help text, and JSON output.

## Projection

- Vocabulary terms map to stable text labels and machine-readable codes.
- `content.code` maps to commands, flags, IDs, and JSON fields.
- Intent tokens map to words, symbols, stream selection, and exit-code
  categories rather than color alone.
- `--json` output uses registered machine-output patterns.

## Required Evidence

- Process execution proof with stdout, stderr, and exit-code assertions.
- Golden transcripts for user-facing examples.
- JSON envelope assertions for automation paths.

## Drift Signals

- Diagnostics printed to stdout when `--json` is active.
- New exit-code meanings not reflected in vocabulary or interaction models.
- Help examples that use terms not present in the product vocabulary.

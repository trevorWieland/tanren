# C01 Phase-Events Repair Runbook

One-off recovery for spec `00000000-0000-0000-0000-000000000c01`
(`tanren/specs/rust-testing-hard-cutover-phase0`).

## Purpose

- Snapshot the existing `phase-events.jsonl`.
- Rewrite every line into canonical JSON envelope form for the current
  schema (`schema_version=1.0.0`) without changing semantic payloads.
- Re-validate replay/status so orchestration can resume cleanly.

## Procedure

1. Run the one-off repair script:

```bash
uv run python scripts/orchestration/repair_c01_phase_events.py
```

2. Replay into a clean database:

```bash
tanren-cli --database-url sqlite:/tmp/tanren-c01-replay.db methodology --phase do-task replay tanren/specs/rust-testing-hard-cutover-phase0
```

3. Verify status in the replayed DB:

```bash
tanren-cli --database-url sqlite:/tmp/tanren-c01-replay.db methodology --phase do-task spec status --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000c01"}'
```

4. Re-run orchestration from the original workspace DB:

```bash
scripts/orchestration/phase0.sh --spec-id 00000000-0000-0000-0000-000000000c01 --spec-folder tanren/specs/rust-testing-hard-cutover-phase0 --config tanren.yml --database-url sqlite:tanren.db
```

## Notes

- Backups are written to:
  `tanren/specs/rust-testing-hard-cutover-phase0/orchestration/phase-events-repair-backups/`.
- This runbook is intentionally scoped to c01 and is not a generic
  migration procedure.

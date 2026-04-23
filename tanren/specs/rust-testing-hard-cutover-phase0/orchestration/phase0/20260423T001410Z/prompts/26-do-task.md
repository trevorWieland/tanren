Run Tanren phase `do-task` for spec `00000000-0000-0000-0000-000000000c01`.
Spec folder: `tanren/specs/rust-testing-hard-cutover-phase0`
Database URL: `sqlite:tanren.db?mode=rwc`
Target task_id: 019db58b-b2cb-7413-b146-fd88721c2784

Requirements:
- Use Tanren MCP tools for all structured state changes.
- If MCP is unavailable, use Tanren CLI with canonical globals:
  tanren-cli --database-url "sqlite:tanren.db?mode=rwc" methodology --phase "do-task" --spec-id "00000000-0000-0000-0000-000000000c01" --spec-folder "tanren/specs/rust-testing-hard-cutover-phase0" <noun> <verb> --params-file '<payload.json>'
- Complete this phase fully and emit `report_phase_outcome`.
- If blocked, emit a typed blocked outcome (or investigate escalation path).
- Never hand-edit orchestrator-owned artifacts.

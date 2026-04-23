---
kind: signposts
spec_id: 00000000-0000-0000-0000-000000000c01
entries:
- id: 019db8cc-3cf4-72a3-afa5-8a79a6fb294a
  status: resolved
  problem: Full just check failed in an unrelated existing test due an unnecessary qualification warning promoted to error.
  evidence: just check failed at bin/tanren-cli/tests/cli_methodology_spec_status.rs:263 with -D warnings unused-qualifications.
  tried:
  - Ran just check
  - Applied one-line fix from std::vec::Vec::len to Vec::len
  - Re-ran just check successfully
  resolution: Applied the minimal one-line fix to satisfy -D warnings and re-ran full just check to green.
  files_affected:
  - bin/tanren-cli/tests/cli_methodology_spec_status.rs
  - docs/rewrite/PHASE0_PROOF_RUNBOOK.md
  - docs/rewrite/PHASE0_PROOF_EVIDENCE_INDEX.md
  - scripts/proof/phase0/run.sh
  created_at: 2026-04-23T05:24:45.940773Z
  updated_at: 2026-04-23T05:24:51.778026Z
- id: 019db8d8-bf38-77c3-b535-9becf9266d81
  status: unresolved
  problem: 'T13 terminal closure gate blocked: just ci fails strict mutation gate'
  evidence: just ci exit 2 at check-phase0-mutation-gate; artifacts/phase0-mutation/enforced/20260423T053640Z/cargo-mutants.stdout.log shows 2 MISSED mutants in crates/tanren-bdd-phase0/src/main.rs and triage at artifacts/phase0-mutation/enforced/20260423T053640Z/triage.json records executed_nonzero.
  tried:
  - Ran just check (pass, exit 0)
  - Ran just ci (initial fail on formatting); applied formatter-equivalent layout in crates/tanren-bdd-phase0/src/main.rs and reran
  - Reran just ci; progressed to strict gates and failed at check-phase0-mutation-gate with exit 2
  files_affected:
  - docs/rewrite/PHASE0_ORCHESTRATION_META_READINESS_REPORT.md
  - crates/tanren-bdd-phase0/src/main.rs
  - artifacts/phase0-mutation/enforced/20260423T053640Z/triage.json
  - artifacts/phase0-mutation/enforced/20260423T053640Z/cargo-mutants.stdout.log
  created_at: 2026-04-23T05:38:25.720962Z
  updated_at: 2026-04-23T05:38:25.720962Z
- id: 019dba8a-90c0-7901-a457-69a875a89364
  status: architectural_constraint
  problem: Local strict mutation gate cannot complete in this sandbox due uv cache permission denial.
  evidence: artifacts/phase0-readiness/20260423T133008Z/just-ci.log captures check-phase0-mutation-gate failing with os error 1 while opening /Users/trevor/.cache/uv/sdists-v9/.git.
  tried:
  - Ran just ci and captured full gate output to artifacts/phase0-readiness/20260423T133008Z/just-ci.log.
  - Re-ran just ci with pipefail-enabled wrapper to confirm terminal exit code 2.
  files_affected:
  - docs/rewrite/PHASE0_ORCHESTRATION_META_READINESS_REPORT.md
  - artifacts/phase0-readiness/20260423T133008Z/just-ci.log
  created_at: 2026-04-23T13:32:16.448671Z
  updated_at: 2026-04-23T13:32:16.448671Z
- id: 019dbadf-1161-7c62-bd55-46b97b4950da
  status: resolved
  problem: MCP mutation-runtime guard test was non-hermetic under inherited TANREN_SPEC_FOLDER, causing false CI failure in mutation_without_runtime_env_returns_env_scoped_validation_error.
  evidence: just ci failed in tanren-mcp::tool_surface_mutation_runtime at line 243 with isError false; shell env had TANREN_SPEC_FOLDER=tanren/specs/rust-testing-hard-cutover-phase0.
  tried:
  - Validated failing assertion and runtime env in shell
  - Updated spawn_without_runtime_spec_folder helper to env_remove TANREN_SPEC_FOLDER and TANREN_SPEC_ID
  resolution: Cleared inherited runtime env in the no-runtime helper (env_remove TANREN_SPEC_FOLDER/TANREN_SPEC_ID); subsequent just ci passed fully with strict mutation gate survivors=0.
  files_affected:
  - bin/tanren-mcp/tests/tool_surface_mutation_runtime.rs
  created_at: 2026-04-23T15:04:34.401379Z
  updated_at: 2026-04-23T15:04:40.367137Z
---
# Signposts

## Entries
- 019db8cc-3cf4-72a3-afa5-8a79a6fb294a [resolved] Full just check failed in an unrelated existing test due an unnecessary qualification warning promoted to error.
- 019db8d8-bf38-77c3-b535-9becf9266d81 [unresolved] T13 terminal closure gate blocked: just ci fails strict mutation gate
- 019dba8a-90c0-7901-a457-69a875a89364 [architectural_constraint] Local strict mutation gate cannot complete in this sandbox due uv cache permission denial.
- 019dbadf-1161-7c62-bd55-46b97b4950da [resolved] MCP mutation-runtime guard test was non-hermetic under inherited TANREN_SPEC_FOLDER, causing false CI failure in mutation_without_runtime_env_returns_env_scoped_validation_error.

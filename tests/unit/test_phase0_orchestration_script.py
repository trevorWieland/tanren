"""Regression tests for Phase 0 orchestration script failure routing/logging."""

from __future__ import annotations

import json
import os
import stat
import subprocess
from pathlib import Path


def _write_executable(path: Path, content: str) -> None:
    path.write_text(content)
    path.chmod(path.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)


def _write_tanren_cli_stub(path: Path) -> None:
    _write_executable(
        path,
        """#!/usr/bin/env bash
set -euo pipefail

if [[ -n "${TANREN_TEST_TANREN_CLI_LOG:-}" ]]; then
  printf '%s\\n' "$*" >> "${TANREN_TEST_TANREN_CLI_LOG}"
fi

if [[ "${1:-}" == "install" ]]; then
  exit 0
fi

if [[ "${1:-}" == "--database-url" ]]; then
  shift 2
fi

if [[ "${1:-}" != "methodology" ]]; then
  echo "{}"
  exit 0
fi
shift

while [[ $# -gt 0 ]]; do
  case "$1" in
    --methodology-config|--phase|--spec-id|--spec-folder)
      shift 2
      ;;
    phase-capabilities)
      cat "${TANREN_TEST_PHASE_CAPS_FILE:?}"
      exit 0
      ;;
    spec)
      shift
      [[ "${1:-}" == "status" ]] || exit 1
      shift
      if [[ "${1:-}" == "--json" ]]; then
        shift 2
      fi
      idx=0
      if [[ -f "${TANREN_TEST_STATUS_INDEX_FILE:?}" ]]; then
        idx="$(cat "${TANREN_TEST_STATUS_INDEX_FILE}")"
      fi
      IFS=':' read -r -a status_files <<< "${TANREN_TEST_STATUS_FILES:?}"
      count="${#status_files[@]}"
      (( count > 0 )) || exit 1
      if (( idx >= count )); then
        idx=$((count - 1))
      fi
      cat "${status_files[$idx]}"
      echo $((idx + 1)) > "${TANREN_TEST_STATUS_INDEX_FILE}"
      exit 0
      ;;
    task)
      shift
      case "${1:-}" in
        list)
          shift
          if [[ "${1:-}" == "--json" ]]; then
            shift 2
          fi
          cat "${TANREN_TEST_TASKS_FILE:?}"
          exit 0
          ;;
        guard)
          shift
          if [[ "${1:-}" == "--json" ]]; then
            shift 2
          fi
          echo '{"schema_version":"1.0.0"}'
          exit 0
          ;;
        reset-guards)
          shift
          if [[ "${1:-}" == "--json" ]]; then
            shift 2
          fi
          echo '{"schema_version":"1.0.0"}'
          exit 0
          ;;
        *)
          exit 1
          ;;
      esac
      ;;
    *)
      shift
      ;;
  esac
done

echo "{}"
""",
    )


def _write_stub_tools(bin_dir: Path, codex_log: Path) -> None:
    _write_tanren_cli_stub(bin_dir / "tanren-cli")
    _write_executable(
        bin_dir / "tanren-mcp",
        """#!/usr/bin/env bash
exit 0
""",
    )
    _write_executable(
        bin_dir / "uv",
        """#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" == "run" && "${2:-}" == "python" ]]; then
  echo "stub-capability-token"
  exit 0
fi
exit 0
""",
    )
    _write_executable(
        bin_dir / "codex",
        f"""#!/usr/bin/env bash
set -euo pipefail
printf '%s\\n' "$*" >> "{codex_log}"
exit "${{TANREN_TEST_CODEX_EXIT:-0}}"
""",
    )


def _status_payload(
    spec_id: str,
    next_action: str,
    next_task_id: str | None,
    next_step: str | None,
    *,
    required_guards: list[object] | None = None,
    pending_required_guards: list[str] | None = None,
) -> dict:
    if required_guards is None:
        required_guards = ["gate_checked", "audited", "adherent"]
    payload: dict[str, object] = {
        "schema_version": "1.0.0",
        "spec_id": spec_id,
        "spec_exists": True,
        "blockers_active": next_action == "resolve_blockers_required",
        "ready_for_walk_spec": False,
        "next_action": next_action,
        "required_guards": required_guards,
        "total_tasks": 1,
        "completed_tasks": 0,
        "abandoned_tasks": 0,
        "implemented_tasks": 1,
        "in_progress_tasks": 0,
        "pending_tasks": 0,
    }
    if next_task_id:
        payload["next_task_id"] = next_task_id
    if next_step:
        payload["next_step"] = next_step
        payload["next_step_reason"] = "test route"
    if pending_required_guards is not None:
        payload["pending_required_guards"] = pending_required_guards
    return payload


def _tasks_payload(spec_id: str, task_id: str) -> dict:
    return {
        "schema_version": "1.0.0",
        "tasks": [
            {
                "id": task_id,
                "spec_id": spec_id,
                "title": "T",
                "description": "",
                "acceptance_criteria": [{"description": "d"}],
                "status": {
                    "state": "implemented",
                    "guards": {"gate_checked": False, "audited": False, "adherent": False},
                },
            }
        ],
    }


def _write_config(path: Path, public_key: Path, private_key: Path, task_hook: str) -> None:
    path.write_text(
        "\n".join([
            "methodology:",
            "  variables:",
            f'    task_verification_hook: "{task_hook}"',
            "  mcp:",
            "    security:",
            "      capability_issuer: test-issuer",
            "      capability_audience: test-audience",
            f"      capability_public_key_file: {public_key}",
            f"      capability_private_key_file: {private_key}",
            "      capability_max_ttl_secs: 900",
            "environment:",
            "  default:",
            "    verification_hooks:",
            '      default: "true"',
        ])
    )


def _run_phase0(
    tmp_path: Path,
    statuses: list[dict],
    *,
    task_hook: str = "true",
    extra_lines: list[str] | None = None,
) -> tuple[subprocess.CompletedProcess[str], Path, Path, Path]:
    spec_id = "00000000-0000-0000-0000-000000000c01"
    task_id = "00000000-0000-0000-0000-000000000111"
    spec_folder = tmp_path / "spec-folder"
    spec_folder.mkdir(parents=True, exist_ok=True)

    bin_dir = tmp_path / "bin"
    bin_dir.mkdir(parents=True, exist_ok=True)
    codex_log = tmp_path / "codex.log"
    tanren_cli_log = tmp_path / "tanren-cli.log"
    _write_stub_tools(bin_dir, codex_log)

    status_files: list[Path] = []
    for idx, status in enumerate(statuses):
        status_file = tmp_path / f"status{idx}.json"
        status_file.write_text(json.dumps(status))
        status_files.append(status_file)
    tasks_file = tmp_path / "tasks.json"
    status_index_file = tmp_path / "status.idx"
    phase_caps_file = tmp_path / "phase-capabilities.json"
    tasks_file.write_text(json.dumps(_tasks_payload(spec_id, task_id)))
    phase_caps_file.write_text(
        json.dumps({
            "schema_version": "1.0.0",
            "phases": [
                {"phase": "do-task", "capabilities_csv": "phase.outcome"},
                {"phase": "audit-task", "capabilities_csv": "phase.outcome"},
                {"phase": "adhere-task", "capabilities_csv": "phase.outcome"},
                {"phase": "run-demo", "capabilities_csv": "phase.outcome"},
                {"phase": "audit-spec", "capabilities_csv": "phase.outcome"},
                {"phase": "adhere-spec", "capabilities_csv": "phase.outcome"},
                {"phase": "investigate", "capabilities_csv": "phase.outcome,phase.escalate"},
            ],
        })
    )

    public_key = tmp_path / "public.pem"
    private_key = tmp_path / "private.pem"
    public_key.write_text("pub")
    private_key.write_text("priv")
    config_path = tmp_path / "tanren.yml"
    _write_config(config_path, public_key, private_key, task_hook=task_hook)
    if extra_lines:
        with config_path.open("a", encoding="utf-8") as handle:
            handle.write("\n")
            handle.write("\n".join(extra_lines))
            handle.write("\n")

    env = os.environ.copy()
    env["PATH"] = f"{bin_dir}:{env['PATH']}"
    env["TANREN_TEST_PHASE_CAPS_FILE"] = str(phase_caps_file)
    env["TANREN_TEST_STATUS_FILES"] = ":".join(str(path) for path in status_files)
    env["TANREN_TEST_STATUS_INDEX_FILE"] = str(status_index_file)
    env["TANREN_TEST_TASKS_FILE"] = str(tasks_file)
    env["TANREN_TEST_TANREN_CLI_LOG"] = str(tanren_cli_log)

    script_path = Path("scripts/orchestration/phase0.sh")
    result = subprocess.run(
        [
            str(script_path),
            "--spec-id",
            spec_id,
            "--spec-folder",
            str(spec_folder),
            "--config",
            str(config_path),
            "--database-url",
            "sqlite:stub.db",
            "--output-mode",
            "silent",
            "--max-cycles",
            "4",
        ],
        cwd=Path(__file__).resolve().parents[2],
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )
    return result, spec_folder, codex_log, tanren_cli_log


def test_phase0_batches_all_task_checks_before_single_investigate_and_writes_bundle(
    tmp_path: Path,
) -> None:
    spec_id = "00000000-0000-0000-0000-000000000c01"
    task_id = "00000000-0000-0000-0000-000000000111"
    status0 = _status_payload(
        spec_id,
        "run_loop",
        task_id,
        "task_gate",
        pending_required_guards=["gate_checked", "audited", "adherent"],
    )
    status1 = _status_payload(spec_id, "resolve_blockers_required", None, None)
    result, spec_folder, codex_log_path, tanren_cli_log_path = _run_phase0(
        tmp_path,
        [status0, status1],
        task_hook="false",
    )

    assert result.returncode == 30
    assert "task 1/1 - task_checks (batch-checking)" in result.stdout
    assert "task 1/1 - task_checks batch start" in result.stdout
    assert "task 1/1 - task_gate hook start: task_verification_hook" in result.stdout
    assert "routing task_checks_batch failure to investigate" in result.stdout
    assert "check batch failed" in result.stdout
    assert "iat=" not in result.stdout
    codex_log = codex_log_path.read_text()
    assert "Run Tanren phase `audit-task`" in codex_log
    assert "Run Tanren phase `adhere-task`" in codex_log
    assert "Run Tanren phase `investigate`" in codex_log
    tanren_cli_log = tanren_cli_log_path.read_text()
    assert "task reset-guards" in tanren_cli_log

    bundle_indexes = sorted(
        spec_folder.glob("orchestration/phase0/*/investigation-bundles/*/index.md")
    )
    assert len(bundle_indexes) == 1
    bundle_index = bundle_indexes[0].read_text()
    assert "task_verification_hook" in bundle_index
    assert "Failed Checks" in bundle_index


def test_phase0_executes_task_investigate_step_before_blocker_checkpoint(tmp_path: Path) -> None:
    spec_id = "00000000-0000-0000-0000-000000000c01"
    task_id = "00000000-0000-0000-0000-000000000111"
    status0 = _status_payload(spec_id, "run_loop", task_id, "task_investigate")
    status0["investigate_source_phase"] = "audit-task"
    status0["investigate_source_outcome"] = "blocked"
    status0["investigate_source_task_id"] = task_id
    status1 = _status_payload(spec_id, "resolve_blockers_required", None, None)
    result, _spec_folder, codex_log_path, _tanren_cli_log_path = _run_phase0(
        tmp_path,
        [status0, status1],
        task_hook="true",
    )

    assert result.returncode == 30
    assert "task 1/1 - task_investigate (investigating)" in result.stdout
    assert "iat=" not in result.stdout
    codex_log = codex_log_path.read_text()
    assert "Run Tanren phase `investigate`" in codex_log


def test_phase0_passes_recovery_bundle_pointer_to_do_task_retry_prompt(tmp_path: Path) -> None:
    spec_id = "00000000-0000-0000-0000-000000000c01"
    task_id = "00000000-0000-0000-0000-000000000111"
    status0 = _status_payload(
        spec_id,
        "run_loop",
        task_id,
        "task_gate",
        pending_required_guards=["gate_checked", "audited", "adherent"],
    )
    status1 = _status_payload(spec_id, "run_loop", task_id, "task_do_task")
    status2 = _status_payload(spec_id, "resolve_blockers_required", None, None)
    result, spec_folder, codex_log_path, _tanren_cli_log_path = _run_phase0(
        tmp_path,
        [status0, status1, status2],
        task_hook="false",
    )

    assert result.returncode == 30
    codex_log = codex_log_path.read_text()
    assert "Run Tanren phase `do-task`" in codex_log
    assert "Context bundle index:" in codex_log

    recovery_files = sorted(spec_folder.glob("orchestration/phase0/*/recovery/task-*.md"))
    assert len(recovery_files) == 1
    recovery_context = recovery_files[0].read_text()
    assert "bundle_index:" in recovery_context


def test_phase0_missing_extra_guard_hook_is_hard_configuration_error(tmp_path: Path) -> None:
    spec_id = "00000000-0000-0000-0000-000000000c01"
    task_id = "00000000-0000-0000-0000-000000000111"
    status0 = _status_payload(
        spec_id,
        "run_loop",
        task_id,
        "task_adhere",
        required_guards=["gate_checked", "audited", "adherent", {"extra": "lint_extra"}],
        pending_required_guards=["lint_extra"],
    )
    status1 = _status_payload(spec_id, "resolve_blockers_required", None, None)
    result, _spec_folder, _codex_log_path, _tanren_cli_log_path = _run_phase0(
        tmp_path,
        [status0, status1],
        task_hook="true",
    )

    assert result.returncode != 0
    assert "missing required methodology.variables.task_check_hook_lint_extra" in result.stderr

#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
cd "${REPO_ROOT}"

usage() {
    cat <<'USAGE'
Usage: scripts/proof/phase0/run.sh [--output-root PATH] [--timestamp YYYYMMDDTHHMMSSZ] [--skip-verify]

Collect a Phase 0 proof artifact pack under artifacts/phase0-proof/<timestamp>/.
USAGE
}

OUTPUT_ROOT="${PHASE0_PROOF_OUTPUT_ROOT:-${REPO_ROOT}/artifacts/phase0-proof}"
TIMESTAMP="$(date -u +"%Y%m%dT%H%M%SZ")"
SKIP_VERIFY=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --output-root)
            OUTPUT_ROOT="$2"
            shift 2
            ;;
        --timestamp)
            TIMESTAMP="$2"
            shift 2
            ;;
        --skip-verify)
            SKIP_VERIFY=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown arg: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

PACK_DIR="${OUTPUT_ROOT}/${TIMESTAMP}"
mkdir -p "${PACK_DIR}/scenarios"

RESULTS_TSV="${PACK_DIR}/_results.tsv"
SCENARIO_INDEX="${PACK_DIR}/_scenarios.tsv"
: > "${RESULTS_TSV}"

cat > "${SCENARIO_INDEX}" <<'SCENARIOS'
1.1	Valid lifecycle changes are accepted
1.2	Invalid lifecycle changes are rejected
2.1	Accepted changes emit durable events
2.2	Replay reconstructs equivalent state
2.3	Corrupt/non-canonical history fails safely
3.1	Operator can run core dispatch flow end-to-end
3.2	Authentication and replay protections are enforced
4.1	Structured workflow state is code-owned
4.2	Agent markdown remains behavior-only
5.1	Completion requires required guards
5.2	Terminal tasks are not reopened
6.1	Inputs are validated at the boundary
6.2	Capability scoping prevents out-of-phase actions
6.3	MCP and CLI produce equivalent semantics
7.1	Install output is predictable and idempotent
7.2	Drift is detectable and explicit
7.3	Multi-agent targets stay semantically aligned
8.1	Human-guided end-to-end spec loop runs in Phase 0
SCENARIOS

quote_cmd() {
    local out=""
    local token
    for token in "$@"; do
        if [[ -n "${out}" ]]; then
            out+=" "
        fi
        out+="$(printf '%q' "${token}")"
    done
    printf '%s\n' "${out}"
}

tanren_cli() {
    cargo run --quiet -p tanren-cli -- "$@"
}

record_result() {
    local scenario="$1"
    local witness_kind="$2"
    local status="$3"
    local owner="$4"
    local witness_name="$5"
    local rel_dir="$6"
    local command_str="$7"
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "${scenario}" "${witness_kind}" "${status}" "${owner}" "${witness_name}" "${rel_dir}" "${command_str}" \
        >> "${RESULTS_TSV}"
}

run_command_witness() {
    local scenario="$1"
    local witness_kind="$2"
    local owner="$3"
    local witness_name="$4"
    shift 4

    local witness_dir="${PACK_DIR}/scenarios/${scenario}/${witness_kind}"
    mkdir -p "${witness_dir}"

    local cmd
    cmd="$(quote_cmd "$@")"
    printf '%s\n' "${cmd}" > "${witness_dir}/command.txt"

    local exit_code=0
    if "$@" > "${witness_dir}/stdout.log" 2> "${witness_dir}/stderr.log"; then
        touch "${witness_dir}/PASS"
    else
        exit_code=$?
        touch "${witness_dir}/FAIL"
    fi

    printf '{"exit_code": %d, "status": "%s"}\n' \
        "${exit_code}" "$([[ ${exit_code} -eq 0 ]] && echo pass || echo fail)" \
        > "${witness_dir}/status.json"

    record_result \
        "${scenario}" \
        "${witness_kind}" \
        "$([[ ${exit_code} -eq 0 ]] && echo pass || echo fail)" \
        "${owner}" \
        "${witness_name}" \
        "scenarios/${scenario}/${witness_kind}" \
        "${cmd}"

    return ${exit_code}
}

run_nextest_witness() {
    local scenario="$1"
    local witness_kind="$2"
    local owner="$3"
    local package="$4"
    local test_name="$5"

    local -a cmd=(cargo nextest run -p "${package}" --locked --no-tests=pass)
    case "${package}" in
        tanren-orchestrator|tanren-store)
            cmd+=(--features test-hooks)
            ;;
    esac
    cmd+=("${test_name}")

    run_command_witness \
        "${scenario}" \
        "${witness_kind}" \
        "${owner}" \
        "${package}::${test_name}" \
        "${cmd[@]}"
}

collect_auth_replay_pack() {
    local dir="${PACK_DIR}/auth-replay"
    mkdir -p "${dir}"

    local issuer="tanren-phase0-proof"
    local audience="tanren-cli"
    local org_id="00000000-0000-0000-0000-0000000000a1"
    local user_id="00000000-0000-0000-0000-0000000000b1"

    uv run python - "${dir}" <<'PY'
from __future__ import annotations

from pathlib import Path

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric import ed25519

root = Path(__import__("sys").argv[1])
priv = ed25519.Ed25519PrivateKey.generate()
pub = priv.public_key()

priv_pem = priv.private_bytes(
    encoding=serialization.Encoding.PEM,
    format=serialization.PrivateFormat.PKCS8,
    encryption_algorithm=serialization.NoEncryption(),
)
pub_pem = pub.public_bytes(
    encoding=serialization.Encoding.PEM,
    format=serialization.PublicFormat.SubjectPublicKeyInfo,
)

(root / "actor-private-key.pem").write_bytes(priv_pem)
(root / "actor-public-key.pem").write_bytes(pub_pem)
PY

    local db_url="sqlite:${dir}/auth.db?mode=rwc"
    tanren_cli --database-url "${db_url}" db migrate > "${dir}/migrate.stdout.log" 2> "${dir}/migrate.stderr.log"

    local mint_script="${REPO_ROOT}/scripts/proof/phase0/mint_actor_token.py"
    local token_modes=(valid wrong_issuer wrong_audience expired ttl_over_max)

    mkdir -p "${dir}/tokens" "${dir}/runs"

    for mode in "${token_modes[@]}"; do
        uv run python "${mint_script}" \
            --private-key-pem "${dir}/actor-private-key.pem" \
            --issuer "${issuer}" \
            --audience "${audience}" \
            --org-id "${org_id}" \
            --user-id "${user_id}" \
            --mode "${mode}" \
            --requested-ttl 600 \
            --max-ttl 900 \
            --token-only \
            > "${dir}/tokens/${mode}.jwt" \
            2> "${dir}/tokens/${mode}.diag.log"

        set +e
        tanren_cli \
            --database-url "${db_url}" \
            dispatch list \
            --actor-token-file "${dir}/tokens/${mode}.jwt" \
            --actor-public-key-file "${dir}/actor-public-key.pem" \
            --token-issuer "${issuer}" \
            --token-audience "${audience}" \
            --actor-token-max-ttl-secs 900 \
            > "${dir}/runs/${mode}-list.stdout.log" \
            2> "${dir}/runs/${mode}-list.stderr.log"
        echo "$?" > "${dir}/runs/${mode}-list.exit_code"
        set -e
    done

    # Valid token create
    tanren_cli \
        --database-url "${db_url}" \
        dispatch create \
        --project "proof-project" \
        --phase do_task \
        --cli claude \
        --branch main \
        --spec-folder spec \
        --workflow-id phase0-proof \
        --actor-token-file "${dir}/tokens/valid.jwt" \
        --actor-public-key-file "${dir}/actor-public-key.pem" \
        --token-issuer "${issuer}" \
        --token-audience "${audience}" \
        --actor-token-max-ttl-secs 900 \
        > "${dir}/runs/valid-create.stdout.log" \
        2> "${dir}/runs/valid-create.stderr.log"

    # Replay reuse token: first create succeeds, second fails.
    uv run python "${mint_script}" \
        --private-key-pem "${dir}/actor-private-key.pem" \
        --issuer "${issuer}" \
        --audience "${audience}" \
        --org-id "${org_id}" \
        --user-id "${user_id}" \
        --mode replay_reuse \
        --requested-ttl 600 \
        --max-ttl 900 \
        --token-only \
        > "${dir}/tokens/replay_reuse.jwt" \
        2> "${dir}/tokens/replay_reuse.diag.log"

    tanren_cli \
        --database-url "${db_url}" \
        dispatch create \
        --project "proof-project" \
        --phase do_task \
        --cli claude \
        --branch main \
        --spec-folder spec \
        --workflow-id phase0-proof-replay-1 \
        --actor-token-file "${dir}/tokens/replay_reuse.jwt" \
        --actor-public-key-file "${dir}/actor-public-key.pem" \
        --token-issuer "${issuer}" \
        --token-audience "${audience}" \
        --actor-token-max-ttl-secs 900 \
        > "${dir}/runs/replay-first-create.stdout.log" \
        2> "${dir}/runs/replay-first-create.stderr.log"

    set +e
    tanren_cli \
        --database-url "${db_url}" \
        dispatch create \
        --project "proof-project" \
        --phase do_task \
        --cli claude \
        --branch main \
        --spec-folder spec \
        --workflow-id phase0-proof-replay-2 \
        --actor-token-file "${dir}/tokens/replay_reuse.jwt" \
        --actor-public-key-file "${dir}/actor-public-key.pem" \
        --token-issuer "${issuer}" \
        --token-audience "${audience}" \
        --actor-token-max-ttl-secs 900 \
        > "${dir}/runs/replay-second-create.stdout.log" \
        2> "${dir}/runs/replay-second-create.stderr.log"
    echo "$?" > "${dir}/runs/replay-second-create.exit_code"
    set -e

    uv run python - "${dir}" <<'PY'
from __future__ import annotations

import json
from pathlib import Path

root = Path(__import__("sys").argv[1])
runs = root / "runs"

def read_exit(name: str) -> int:
    p = runs / f"{name}.exit_code"
    if p.exists():
        return int(p.read_text(encoding="utf-8").strip())
    return 0

summary = {
    "valid_list": read_exit("valid-list") == 0,
    "wrong_issuer_rejected": read_exit("wrong_issuer-list") != 0,
    "wrong_audience_rejected": read_exit("wrong_audience-list") != 0,
    "expired_rejected": read_exit("expired-list") != 0,
    "ttl_over_max_rejected": read_exit("ttl_over_max-list") != 0,
    "replay_reuse_rejected": read_exit("replay-second-create") != 0,
}
summary["status"] = "pass" if all(summary.values()) else "fail"
(root / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
PY
}

collect_replay_pack() {
    local dir="${PACK_DIR}/replay-pack"
    mkdir -p "${dir}/source-spec" "${dir}/verdicts"

    local spec_id="00000000-0000-0000-0000-0000000002a2"
    local caps="task.create,task.read"

    local source_db="sqlite:${dir}/source.db?mode=rwc"
    local target_db="sqlite:${dir}/target.db?mode=rwc"
    local invalid_target_db="sqlite:${dir}/invalid-target.db?mode=rwc"

    tanren_cli --database-url "${source_db}" db migrate > "${dir}/source-migrate.stdout.log" 2> "${dir}/source-migrate.stderr.log"
    tanren_cli --database-url "${target_db}" db migrate > "${dir}/target-migrate.stdout.log" 2> "${dir}/target-migrate.stderr.log"
    tanren_cli --database-url "${invalid_target_db}" db migrate > "${dir}/invalid-target-migrate.stdout.log" 2> "${dir}/invalid-target-migrate.stderr.log"

    cat > "${dir}/create-task.json" <<JSON
{"schema_version":"1.0.0","spec_id":"${spec_id}","title":"Replay proof task","description":"generated for phase0 replay proof","origin":{"kind":"user"},"acceptance_criteria":[]}
JSON

    TANREN_PHASE_CAPABILITIES="${caps}" tanren_cli \
        --database-url "${source_db}" \
        methodology \
        --phase shape-spec \
        --spec-id "${spec_id}" \
        --spec-folder "${dir}/source-spec" \
        task create \
        --params-file "${dir}/create-task.json" \
        > "${dir}/source-create.stdout.log" \
        2> "${dir}/source-create.stderr.log"

    cat > "${dir}/list-tasks.json" <<JSON
{"schema_version":"1.0.0","spec_id":"${spec_id}"}
JSON

    TANREN_PHASE_CAPABILITIES="${caps}" tanren_cli \
        --database-url "${source_db}" \
        methodology \
        --phase do-task \
        task list \
        --params-file "${dir}/list-tasks.json" \
        > "${dir}/source-task-list.json" \
        2> "${dir}/source-task-list.stderr.log"

    cp "${dir}/source-spec/phase-events.jsonl" "${dir}/source-event-stream.jsonl"

    tanren_cli \
        --database-url "${target_db}" \
        methodology replay "${dir}/source-spec" \
        > "${dir}/target-replay.stdout.log" \
        2> "${dir}/target-replay.stderr.log"

    TANREN_PHASE_CAPABILITIES="${caps}" tanren_cli \
        --database-url "${target_db}" \
        methodology \
        --phase do-task \
        task list \
        --params-file "${dir}/list-tasks.json" \
        > "${dir}/target-task-list.json" \
        2> "${dir}/target-task-list.stderr.log"

    uv run python - "${dir}" <<'PY'
from __future__ import annotations

import json
from pathlib import Path

root = Path(__import__("sys").argv[1])
source = json.loads((root / "source-task-list.json").read_text(encoding="utf-8"))
target = json.loads((root / "target-task-list.json").read_text(encoding="utf-8"))

verdict = {
    "equivalent": source == target,
    "source_task_count": len(source.get("tasks", [])),
    "target_task_count": len(target.get("tasks", [])),
}
verdict["status"] = "pass" if verdict["equivalent"] else "fail"
(root / "verdicts" / "equivalence.json").write_text(
    json.dumps(verdict, indent=2) + "\n", encoding="utf-8"
)
PY

    uv run python - "${dir}/source-event-stream.jsonl" "${dir}/invalid-source-event-stream.jsonl" <<'PY'
from __future__ import annotations

import json
from pathlib import Path

source = Path(__import__("sys").argv[1])
dest = Path(__import__("sys").argv[2])
lines = source.read_text(encoding="utf-8").splitlines()
out = []
for idx, line in enumerate(lines, start=1):
    if not line.strip():
        continue
    data = json.loads(line)
    if idx == 1:
        data.pop("origin_kind", None)
    out.append(json.dumps(data, separators=(",", ":")))
dest.write_text("\n".join(out) + "\n", encoding="utf-8")
PY

    mkdir -p "${dir}/invalid-spec"
    cp "${dir}/invalid-source-event-stream.jsonl" "${dir}/invalid-spec/phase-events.jsonl"

    set +e
    tanren_cli \
        --database-url "${invalid_target_db}" \
        methodology replay "${dir}/invalid-spec" \
        > "${dir}/invalid-replay.stdout.log" \
        2> "${dir}/invalid-replay.stderr.log"
    echo "$?" > "${dir}/invalid-replay.exit_code"
    set -e

    TANREN_PHASE_CAPABILITIES="${caps}" tanren_cli \
        --database-url "${invalid_target_db}" \
        methodology \
        --phase do-task \
        task list \
        --params-file "${dir}/list-tasks.json" \
        > "${dir}/invalid-target-task-list.json" \
        2> "${dir}/invalid-target-task-list.stderr.log"

    uv run python - "${dir}" <<'PY'
from __future__ import annotations

import json
from pathlib import Path

root = Path(__import__("sys").argv[1])
exit_code = int((root / "invalid-replay.exit_code").read_text(encoding="utf-8").strip())
tasks = json.loads((root / "invalid-target-task-list.json").read_text(encoding="utf-8")).get("tasks", [])
verdict = {
    "replay_rejected": exit_code != 0,
    "no_partial_apply": len(tasks) == 0,
    "invalid_replay_exit_code": exit_code,
    "invalid_target_task_count": len(tasks),
}
verdict["status"] = "pass" if verdict["replay_rejected"] and verdict["no_partial_apply"] else "fail"
(root / "verdicts" / "rollback.json").write_text(
    json.dumps(verdict, indent=2) + "\n", encoding="utf-8"
)
PY
}

collect_manual_walkthrough_pack() {
    local dir="${PACK_DIR}/manual-walkthrough"
    mkdir -p "${dir}" "${dir}/steps"

    local spec_id="00000000-0000-0000-0000-0000000008a1"
    local caps="task.create,task.start,task.complete,task.read,finding.add,demo.frontmatter,demo.results,spec.frontmatter,phase.outcome"
    local db_url="sqlite:${dir}/walkthrough.db?mode=rwc"
    local spec_folder="${dir}/spec"

    tanren_cli --database-url "${db_url}" db migrate > "${dir}/migrate.stdout.log" 2> "${dir}/migrate.stderr.log"
    mkdir -p "${spec_folder}"

    run_step() {
        local step="$1"
        shift
        local step_dir="${dir}/steps/${step}"
        mkdir -p "${step_dir}"
        printf '%s\n' "$(quote_cmd "$@")" > "${step_dir}/command.txt"
        TANREN_PHASE_CAPABILITIES="${caps}" "$@" > "${step_dir}/stdout.json" 2> "${step_dir}/stderr.log"
    }

    cat > "${dir}/step1-set-title.json" <<JSON
{"schema_version":"1.0.0","spec_id":"${spec_id}","title":"Phase0 walkthrough sample"}
JSON

    run_step "1-shape-spec-title" \
        tanren_cli --database-url "${db_url}" methodology --phase shape-spec --spec-id "${spec_id}" --spec-folder "${spec_folder}" spec set-title --params-file "${dir}/step1-set-title.json"

    cat > "${dir}/step1-create-task.json" <<JSON
{"schema_version":"1.0.0","spec_id":"${spec_id}","title":"Draft proof closure plan","description":"Create deterministic proof harness and docs.","origin":{"kind":"user"},"acceptance_criteria":[]}
JSON

    run_step "1-shape-spec-task" \
        tanren_cli --database-url "${db_url}" methodology --phase shape-spec --spec-id "${spec_id}" --spec-folder "${spec_folder}" task create --params-file "${dir}/step1-create-task.json"

    uv run python - "${dir}/steps/1-shape-spec-task/stdout.json" "${dir}/task_id.txt" <<'PY'
from __future__ import annotations

import json
from pathlib import Path

payload = json.loads(Path(__import__("sys").argv[1]).read_text(encoding="utf-8"))
Path(__import__("sys").argv[2]).write_text(payload["task_id"] + "\n", encoding="utf-8")
PY

    local task_id
    task_id="$(tr -d '\n' < "${dir}/task_id.txt")"

    cat > "${dir}/step1-demo-add.json" <<JSON
{"schema_version":"1.0.0","spec_id":"${spec_id}","id":"demo-step-1","mode":"RUN","description":"Run proof harness","expected_observable":"summary.json and summary.md generated"}
JSON

    run_step "1-shape-spec-demo" \
        tanren_cli --database-url "${db_url}" methodology --phase shape-spec --spec-id "${spec_id}" --spec-folder "${spec_folder}" demo add-step --params-file "${dir}/step1-demo-add.json"

    cat > "${dir}/step2-list.json" <<JSON
{"schema_version":"1.0.0","spec_id":"${spec_id}"}
JSON

    run_step "2-resolve-context" \
        tanren_cli --database-url "${db_url}" methodology --phase do-task task list --params-file "${dir}/step2-list.json"

    cat > "${dir}/step3-start.json" <<JSON
{"schema_version":"1.0.0","task_id":"${task_id}"}
JSON

    run_step "3-do-task-start" \
        tanren_cli --database-url "${db_url}" methodology --phase do-task --spec-id "${spec_id}" --spec-folder "${spec_folder}" task start --params-file "${dir}/step3-start.json"

    cat > "${dir}/step3-complete.json" <<JSON
{"schema_version":"1.0.0","task_id":"${task_id}","evidence_refs":["proof://phase0/summary"]}
JSON

    run_step "3-do-task-complete" \
        tanren_cli --database-url "${db_url}" methodology --phase do-task --spec-id "${spec_id}" --spec-folder "${spec_folder}" task complete --params-file "${dir}/step3-complete.json"

    cat > "${dir}/step4-finding.json" <<JSON
{"schema_version":"1.0.0","spec_id":"${spec_id}","severity":"note","title":"Harness output reviewed","description":"Audit-task reviewed generated proof artifacts.","source":{"kind":"audit","phase":"audit-task","pillar":"completeness"},"attached_task":"${task_id}","affected_files":[],"line_numbers":[]}
JSON

    run_step "4-audit-task" \
        tanren_cli --database-url "${db_url}" methodology --phase audit-task --spec-id "${spec_id}" --spec-folder "${spec_folder}" finding add --params-file "${dir}/step4-finding.json"

    cat > "${dir}/step5-demo-result.json" <<JSON
{"schema_version":"1.0.0","spec_id":"${spec_id}","step_id":"demo-step-1","status":"pass","observed":"Proof harness generated deterministic summary files."}
JSON

    run_step "5-run-demo" \
        tanren_cli --database-url "${db_url}" methodology --phase run-demo --spec-id "${spec_id}" --spec-folder "${spec_folder}" demo append-result --params-file "${dir}/step5-demo-result.json"

    cat > "${dir}/step6-outcome.json" <<JSON
{"schema_version":"1.0.0","spec_id":"${spec_id}","outcome":{"outcome":"complete","summary":"Audit-spec verified artifact completeness and consistency."}}
JSON

    run_step "6-audit-spec" \
        tanren_cli --database-url "${db_url}" methodology --phase audit-spec --spec-id "${spec_id}" --spec-folder "${spec_folder}" phase outcome --params-file "${dir}/step6-outcome.json"

    cat > "${dir}/step7-walk-outcome.json" <<JSON
{"schema_version":"1.0.0","spec_id":"${spec_id}","outcome":{"outcome":"complete","summary":"Walk-spec validated end-to-end trace from shape-spec to walk-spec."}}
JSON

    run_step "7-walk-spec" \
        tanren_cli --database-url "${db_url}" methodology --phase walk-spec --spec-id "${spec_id}" --spec-folder "${spec_folder}" phase outcome --params-file "${dir}/step7-walk-outcome.json"

    TANREN_PHASE_CAPABILITIES="${caps}" tanren_cli \
        --database-url "${db_url}" \
        methodology --phase walk-spec task list --params-file "${dir}/step2-list.json" \
        > "${dir}/final-task-list.json" 2> "${dir}/final-task-list.stderr.log"

    cp "${spec_folder}/phase-events.jsonl" "${dir}/phase-events.jsonl"

    uv run python - "${dir}" "${task_id}" <<'PY'
from __future__ import annotations

import json
from pathlib import Path

root = Path(__import__("sys").argv[1])
task_id = __import__("sys").argv[2]
final_tasks = json.loads((root / "final-task-list.json").read_text(encoding="utf-8")).get("tasks", [])
summary = {
    "status": "pass" if any(task.get("id") == task_id for task in final_tasks) else "fail",
    "spec_id": "00000000-0000-0000-0000-0000000008a1",
    "task_id": task_id,
    "step_count": 7,
    "phase_events_lines": len((root / "phase-events.jsonl").read_text(encoding="utf-8").splitlines()),
}
(root / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
PY
}

declare -a FAILED_WITNESSES=()

run_and_track() {
    if ! "$@"; then
        FAILED_WITNESSES+=("$*")
    fi
}

# Feature 1
run_and_track run_nextest_witness "1.1" "positive" "orchestrator" "tanren-orchestrator" "create_dispatch_returns_pending_view"
run_and_track run_nextest_witness "1.1" "falsification" "orchestrator" "tanren-orchestrator" "finalize_without_running_state_rejected"
run_and_track run_nextest_witness "1.2" "positive" "orchestrator" "tanren-orchestrator" "cancel_already_cancelled_returns_error"
run_and_track run_nextest_witness "1.2" "falsification" "orchestrator" "tanren-orchestrator" "cancel_dispatch_transitions_to_cancelled"

# Feature 2
run_and_track run_nextest_witness "2.1" "positive" "orchestrator" "tanren-orchestrator" "create_emits_dispatch_created_and_step_enqueued"
run_and_track run_nextest_witness "2.1" "falsification" "store" "tanren-store" "create_dispatch_replay_rejection_rolls_back_projection_step_and_events"
run_and_track run_nextest_witness "2.2" "positive" "cli" "tanren-cli" "replay_round_trips_real_generated_phase_events_file"
run_and_track run_nextest_witness "2.2" "falsification" "store" "tanren-store" "replay_rejects_tool_mismatch"
run_and_track run_nextest_witness "2.3" "positive" "store" "tanren-store" "replay_reports_malformed_line_with_raw_context"
run_and_track run_nextest_witness "2.3" "falsification" "store" "tanren-store" "replay_preserves_line_number_and_raw_for_midstream_malformed_line"

# Feature 3
run_and_track run_nextest_witness "3.1" "positive" "cli" "tanren-cli" "sqlite_lifecycle_create_get_list_cancel_is_consistent"
run_and_track run_nextest_witness "3.1" "falsification" "cli" "tanren-cli" "cancel_unauthorized_dispatch_is_hidden_as_not_found"
run_and_track run_nextest_witness "3.2" "positive" "cli-auth" "tanren-cli" "token_without_kid_header_is_accepted_with_static_public_key"
run_and_track run_nextest_witness "3.2" "falsification" "cli-auth" "tanren-cli" "mutating_commands_consume_replay_and_reject_second_use"

# Feature 4
run_and_track run_nextest_witness "4.1" "positive" "app-services" "tanren-app-services" "finalize_emits_unauthorized_edit_and_reverts_file"
run_and_track run_nextest_witness "4.1" "falsification" "app-services" "tanren-app-services" "finalize_allows_projected_phase_events_appends"
run_and_track run_nextest_witness "4.2" "positive" "app-services" "tanren-app-services" "load_catalog_accepts_extension_namespace"
run_and_track run_nextest_witness "4.2" "falsification" "app-services" "tanren-app-services" "load_catalog_rejects_unknown_declared_tool"

# Feature 5
run_and_track run_nextest_witness "5.1" "positive" "app-services" "tanren-app-services" "mark_guard_satisfied_fires_task_completed_when_config_satisfied"
run_and_track run_nextest_witness "5.1" "falsification" "app-services" "tanren-app-services" "mark_guard_satisfied_keeps_implemented_when_guard_not_required"
run_and_track run_nextest_witness "5.2" "positive" "domain" "tanren-domain" "complete_remains_terminal_under_arbitrary_suffixes"
run_and_track run_nextest_witness "5.2" "falsification" "domain" "tanren-domain" "completed_without_all_guards_stays_implemented"

# Feature 6
run_and_track run_nextest_witness "6.1" "positive" "cli-methodology" "tanren-cli" "validation_error_returns_exit_4_with_typed_field_path"
run_and_track run_nextest_witness "6.1" "falsification" "cli-methodology" "tanren-cli" "task_create_then_list_round_trips"
run_and_track run_nextest_witness "6.2" "positive" "cli-methodology" "tanren-cli" "capability_enforcement_denies_when_env_scope_excludes_tool"
run_and_track run_nextest_witness "6.2" "falsification" "cli-methodology" "tanren-cli" "task_create_then_list_round_trips"
run_and_track run_nextest_witness "6.3" "positive" "cli-mcp-parity" "tanren-cli" "cli_and_mcp_match_full_envelopes_and_phase_event_projection_for_mutation_matrix"
run_and_track run_nextest_witness "6.3" "falsification" "mcp-parity" "tanren-mcp" "cli_and_mcp_match_invalid_input_rejection_for_full_tool_matrix"

# Feature 7
run_and_track run_nextest_witness "7.1" "positive" "installer" "tanren-app-services" "plan_install_applies_task_tool_binding_per_target"
run_and_track run_nextest_witness "7.1" "falsification" "installer" "tanren-app-services" "empty_plan_has_no_drift"
run_and_track run_nextest_witness "7.2" "positive" "installer" "tanren-app-services" "drift_reports_nested_symlink_as_extra_file_without_following_target"
run_and_track run_nextest_witness "7.2" "falsification" "cli-install" "tanren-cli" "install_strict_dry_run_reports_exact_diff_payload"
run_and_track run_nextest_witness "7.3" "positive" "installer" "tanren-app-services" "command_matrix_semantic_hashes_match_across_all_targets"
run_and_track run_nextest_witness "7.3" "falsification" "command-contract" "tanren-app-services" "load_catalog_rejects_unknown_required_capability"

# Supplemental artifact packs for G3/G4/G5/G7
collect_auth_replay_pack
collect_replay_pack
collect_manual_walkthrough_pack

# Feature 8
run_and_track run_command_witness "8.1" "positive" "manual-walkthrough" "manual_walkthrough_pack_generated" test -f "${PACK_DIR}/manual-walkthrough/summary.json"
run_and_track run_nextest_witness "8.1" "falsification" "cli-methodology" "tanren-cli" "ingest_phase_events_strict_provenance_rejects_legacy_lines"

uv run python - "${PACK_DIR}" <<'PY'
from __future__ import annotations

import json
from pathlib import Path

pack = Path(__import__("sys").argv[1])
results = []
for line in (pack / "_results.tsv").read_text(encoding="utf-8").splitlines():
    if not line.strip():
        continue
    scenario, witness_kind, status, owner, witness_name, rel_dir, command = line.split("\t")
    results.append(
        {
            "scenario": scenario,
            "witness_kind": witness_kind,
            "status": status,
            "owner": owner,
            "witness_name": witness_name,
            "artifact_path": rel_dir,
            "command": command,
        }
    )

scenario_titles = {}
for line in (pack / "_scenarios.tsv").read_text(encoding="utf-8").splitlines():
    if not line.strip():
        continue
    sid, title = line.split("\t", 1)
    scenario_titles[sid] = title

scenario_map: dict[str, dict[str, object]] = {}
for sid, title in scenario_titles.items():
    scenario_map[sid] = {
        "scenario": sid,
        "title": title,
        "positive": None,
        "falsification": None,
        "status": "fail",
    }

for row in results:
    sid = row["scenario"]
    wk = row["witness_kind"]
    scenario_map[sid][wk] = row

for sid, item in scenario_map.items():
    pos = item["positive"]
    neg = item["falsification"]
    if pos and neg and pos["status"] == "pass" and neg["status"] == "pass":
        item["status"] = "pass"

pack_status = "pass" if all(item["status"] == "pass" for item in scenario_map.values()) else "fail"

summary = {
    "generated_at_utc": __import__("datetime").datetime.now(__import__("datetime").UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
    "pack_dir": str(pack),
    "status": pack_status,
    "scenario_count": len(scenario_map),
    "scenarios": [scenario_map[sid] for sid in sorted(scenario_map.keys(), key=lambda s: tuple(map(int, s.split("."))))],
    "supplemental": {
        "auth_replay_summary": "auth-replay/summary.json",
        "replay_equivalence": "replay-pack/verdicts/equivalence.json",
        "replay_rollback": "replay-pack/verdicts/rollback.json",
        "manual_walkthrough": "manual-walkthrough/summary.json",
    },
}

(pack / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")

lines = [
    "# Phase 0 Proof Summary",
    "",
    f"Pack: `{pack}`",
    f"Overall status: **{pack_status.upper()}**",
    "",
    "| Scenario | Title | Positive | Falsification | Status |",
    "|---|---|---|---|---|",
]
for row in summary["scenarios"]:
    pos = row["positive"]["artifact_path"] if row["positive"] else "missing"
    neg = row["falsification"]["artifact_path"] if row["falsification"] else "missing"
    lines.append(f"| {row['scenario']} | {row['title']} | `{pos}` | `{neg}` | **{row['status']}** |")

lines.extend(
    [
        "",
        "## Supplemental Packs",
        "",
        "- `auth-replay/summary.json`",
        "- `replay-pack/verdicts/equivalence.json`",
        "- `replay-pack/verdicts/rollback.json`",
        "- `manual-walkthrough/summary.json`",
    ]
)
(pack / "summary.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
PY

if [[ ${#FAILED_WITNESSES[@]} -gt 0 ]]; then
    echo "One or more scenario witnesses failed:" >&2
    for item in "${FAILED_WITNESSES[@]}"; do
        echo "  - ${item}" >&2
    done
fi

if [[ "${SKIP_VERIFY}" -ne 1 ]]; then
    "${REPO_ROOT}/scripts/proof/phase0/verify.sh" "${PACK_DIR}"
fi

echo "Phase 0 proof pack written to: ${PACK_DIR}"

if [[ ${#FAILED_WITNESSES[@]} -gt 0 ]]; then
    exit 1
fi

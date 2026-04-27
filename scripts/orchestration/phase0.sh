#!/usr/bin/env bash
# Phase 0 orchestration driver (CLI entrypoint, Codex-first harness).
#
# Flow policy:
# - interactive checkpoints: shape-spec, resolve-blockers, walk-spec
# - autonomous loop is state-machine-driven one step per cycle
# - resume source of truth: tanren-cli methodology spec status

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

usage() {
    cat <<'USAGE_EOF'
Usage: scripts/orchestration/phase0.sh --spec-id <uuid> [options]

Options:
  --spec-id <uuid>                  Required spec id.
  --spec-folder <path>              Spec folder path (default: <spec_root>/<spec-id> from tanren.yml).
  --database-url <url>              Tanren DB URL (default: sqlite:tanren.db; normalized to ?mode=rwc).
  --config <path>                   tanren.yml path (default: tanren.yml).
  --harness-model <model>           Optional harness model override.
  --output-mode <mode>              Output verbosity: silent|quiet|verbose (default: silent).
  --max-cycles <n>                  Max autonomous cycles before fail (default: 64).
  --dry-run                         Simulate actions without mutating state.
  -h, --help                        Show help.
USAGE_EOF
}

finish_silent_line() {
    if [[ "${OUTPUT_MODE:-silent}" == "silent" && "${SILENT_LINE_ACTIVE:-0}" == "1" ]]; then
        printf '\n'
        SILENT_LINE_ACTIVE=0
    fi
}

print_line() {
    finish_silent_line
    printf '[phase0] %s\n' "$*"
}

die() {
    print_line "ERROR: $*" >&2
    if [[ -n "${LAST_COMMAND_LOG:-}" ]]; then
        print_line "last command log: ${LAST_COMMAND_LOG}" >&2
    fi
    if [[ -n "${RUN_DIR:-}" ]]; then
        print_line "run artifacts: ${RUN_DIR}" >&2
    elif [[ -n "${BOOTSTRAP_LOG_ROOT:-}" ]]; then
        print_line "bootstrap logs: ${BOOTSTRAP_LOG_ROOT}" >&2
    fi
    exit 1
}

need_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

trim_scalar() {
    local value="${1:-}"
    value="${value%%#*}"
    value="${value#"${value%%[![:space:]]*}"}"
    value="${value%"${value##*[![:space:]]}"}"
    if [[ "${#value}" -ge 2 && "${value:0:1}" == '"' && "${value: -1}" == '"' ]]; then
        value="${value:1:${#value}-2}"
    fi
    if [[ "${#value}" -ge 2 && "${value:0:1}" == "'" && "${value: -1}" == "'" ]]; then
        value="${value:1:${#value}-2}"
    fi
    printf '%s' "$value"
}

yaml_methodology_var() {
    local config_path="$1"
    local key="$2"
    awk -v key="$key" '
        /^methodology:[[:space:]]*$/ { in_methodology=1; next }
        in_methodology && $0 !~ /^  / && $0 !~ /^[[:space:]]*$/ { in_methodology=0; in_variables=0 }
        in_methodology && /^  variables:[[:space:]]*$/ { in_variables=1; next }
        in_variables && $0 !~ /^    / && $0 !~ /^[[:space:]]*$/ { in_variables=0 }
        in_variables {
            pattern = "^    " key ":[[:space:]]*"
            if ($0 ~ pattern) {
                value = $0
                sub(pattern, "", value)
                print value
                exit
            }
        }
    ' "$config_path"
}

yaml_default_hook() {
    local config_path="$1"
    local phase_key="$2"
    awk -v phase_key="$phase_key" '
        /^environment:[[:space:]]*$/ { in_environment=1; next }
        in_environment && $0 !~ /^  / && $0 !~ /^[[:space:]]*$/ { in_environment=0; in_default=0; in_hooks=0 }
        in_environment && /^  default:[[:space:]]*$/ { in_default=1; next }
        in_default && $0 !~ /^    / && $0 !~ /^[[:space:]]*$/ { in_default=0; in_hooks=0 }
        in_default && /^    verification_hooks:[[:space:]]*$/ { in_hooks=1; next }
        in_hooks && $0 !~ /^      / && $0 !~ /^[[:space:]]*$/ { in_hooks=0 }
        in_hooks {
            pattern = "^      " phase_key ":[[:space:]]*"
            if ($0 ~ pattern) {
                value = $0
                sub(pattern, "", value)
                print value
                exit
            }
        }
    ' "$config_path"
}

yaml_mcp_security() {
    local config_path="$1"
    local key="$2"
    awk -v key="$key" '
        /^methodology:[[:space:]]*$/ { in_methodology=1; next }
        in_methodology && $0 !~ /^  / && $0 !~ /^[[:space:]]*$/ { in_methodology=0; in_mcp=0; in_security=0 }
        in_methodology && /^  mcp:[[:space:]]*$/ { in_mcp=1; next }
        in_mcp && $0 !~ /^    / && $0 !~ /^[[:space:]]*$/ { in_mcp=0; in_security=0 }
        in_mcp && /^    security:[[:space:]]*$/ { in_security=1; next }
        in_security && $0 !~ /^      / && $0 !~ /^[[:space:]]*$/ { in_security=0 }
        in_security {
            pattern = "^      " key ":[[:space:]]*"
            if ($0 ~ pattern) {
                value = $0
                sub(pattern, "", value)
                print value
                exit
            }
        }
    ' "$config_path"
}

normalize_database_url() {
    local url="$1"
    if [[ "$url" == sqlite:* && "$url" != *"?"* && "$url" != sqlite::memory:* ]]; then
        printf '%s?mode=rwc' "$url"
        return
    fi
    printf '%s' "$url"
}

resolve_hook() {
    local var_key="$1"
    local phase_key="$2"
    local fallback="$3"
    local from_var
    from_var="$(trim_scalar "$(yaml_methodology_var "$CONFIG_PATH" "$var_key")")"
    if [[ -n "$from_var" ]]; then
        printf '%s' "$from_var"
        return
    fi
    local from_phase
    from_phase="$(trim_scalar "$(yaml_default_hook "$CONFIG_PATH" "$phase_key")")"
    if [[ -n "$from_phase" ]]; then
        printf '%s' "$from_phase"
        return
    fi
    local from_default
    from_default="$(trim_scalar "$(yaml_default_hook "$CONFIG_PATH" "default")")"
    if [[ -n "$from_default" ]]; then
        printf '%s' "$from_default"
        return
    fi
    printf '%s' "$fallback"
}

print_progress() {
    local message="$1"
    case "$OUTPUT_MODE" in
        silent)
            if [[ -t 1 ]]; then
                # Clear the current terminal line before repainting to avoid
                # stale suffixes when message length shrinks across steps.
                printf '\r\033[2K[phase0] %s' "$message"
                SILENT_LINE_ACTIVE=1
            else
                print_line "$message"
            fi
            ;;
        quiet)
            print_line "$message"
            ;;
        verbose)
            print_line "$message"
            ;;
        *)
            die "unsupported output mode: ${OUTPUT_MODE}"
            ;;
    esac
}

log_quiet() {
    if [[ "$OUTPUT_MODE" == "quiet" || "$OUTPUT_MODE" == "verbose" ]]; then
        print_line "$*"
    fi
}

log_verbose() {
    if [[ "$OUTPUT_MODE" == "verbose" ]]; then
        print_line "$*"
    fi
}

sanitize_label() {
    printf '%s' "$1" | tr -cs '[:alnum:]_.-' '_'
}

run_shell_command() {
    local label="$1"
    local command="$2"
    local allow_fail="${3:-0}"

    if [[ "$DRY_RUN" == "1" ]]; then
        case "$OUTPUT_MODE" in
            verbose)
                print_line "[dry-run] ${label}: ${command}"
                ;;
            quiet)
                print_line "[dry-run] ${label}"
                ;;
            silent)
                ;;
        esac
        return 0
    fi

    COMMAND_INDEX=$((COMMAND_INDEX + 1))
    local safe_label
    safe_label="$(sanitize_label "$label")"
    local log_root
    if [[ -n "${RUN_DIR:-}" ]]; then
        log_root="${RUN_DIR}/commands"
    else
        BOOTSTRAP_LOG_ROOT="${BOOTSTRAP_LOG_ROOT:-${TMPDIR:-/tmp}/tanren-phase0-bootstrap-${$}}"
        log_root="${BOOTSTRAP_LOG_ROOT}/commands"
    fi
    local log_file="${log_root}/$(printf '%03d' "$COMMAND_INDEX")-${safe_label}.log"
    mkdir -p "$(dirname "$log_file")"
    LAST_COMMAND_LOG="$log_file"

    local status=0
    if [[ "$OUTPUT_MODE" == "verbose" ]]; then
        print_line "${label}: ${command}"
        set +e
        bash -lc "$command" 2>&1 | tee "$log_file"
        status=${PIPESTATUS[0]}
        set -e
    else
        set +e
        bash -lc "$command" >"$log_file" 2>&1
        status=$?
        set -e
    fi

    if [[ $status -ne 0 ]]; then
        if [[ "$OUTPUT_MODE" != "verbose" ]]; then
            print_line "command failed: ${label} (exit ${status})"
            while IFS= read -r line; do
                print_line "  ${line}"
            done < <(tail -n 20 "$log_file")
        fi
        if [[ "$allow_fail" == "1" ]]; then
            return "$status"
        fi
        die "${label} failed (exit ${status}); see ${log_file}"
    fi

    if [[ "$OUTPUT_MODE" == "quiet" ]]; then
        print_line "${label}: ok"
    fi
}

run_hook() {
    local hook_name="$1"
    local hook_cmd="$2"
    local allow_fail="${3:-0}"
    [[ -n "$hook_cmd" ]] || die "${hook_name} resolved to empty command"
    run_shell_command "$hook_name" "$hook_cmd" "$allow_fail"
}

spec_status_json() {
    local payload
    payload="$(printf '{"schema_version":"1.0.0","spec_id":"%s"}' "$SPEC_ID")"
    tanren-cli --database-url "$DATABASE_URL" methodology \
        --methodology-config "$CONFIG_PATH" \
        --phase "$STATUS_PHASE" \
        spec status \
        --json "$payload"
}

list_tasks_json() {
    local payload
    payload="$(printf '{"schema_version":"1.0.0","spec_id":"%s"}' "$SPEC_ID")"
    tanren-cli --database-url "$DATABASE_URL" methodology \
        --methodology-config "$CONFIG_PATH" \
        --phase "$STATUS_PHASE" \
        task list \
        --json "$payload"
}

check_kind_json() {
    local check_kind="$1"
    jq -cn --arg check_kind "$check_kind" '
        if $check_kind == "gate" then {kind:"gate"}
        elif $check_kind == "audit" then {kind:"audit"}
        elif $check_kind == "adherence" then {kind:"adherence"}
        elif $check_kind == "demo" then {kind:"demo"}
        elif $check_kind == "spec_gate" then {kind:"spec_gate"}
        else {kind:"custom", name:$check_kind} end
    '
}

source_check_json() {
    local source_phase="$1"
    local check_kind="$2"
    local task_id="${3:-}"
    local kind_json
    kind_json="$(check_kind_json "$check_kind")"
    if [[ -n "$task_id" ]]; then
        jq -cn \
            --arg source_phase "$source_phase" \
            --arg task_id "$task_id" \
            --argjson kind "$kind_json" \
            '{phase:$source_phase, kind:$kind, scope:{scope:"task", task_id:$task_id}}'
    else
        jq -cn \
            --arg source_phase "$source_phase" \
            --argjson kind "$kind_json" \
            '{phase:$source_phase, kind:$kind, scope:{scope:"spec"}}'
    fi
}

open_finding_ids_json() {
    local scope="$1"
    local check_kind="$2"
    local task_id="${3:-}"
    local kind_json payload
    if [[ -n "$task_id" ]]; then
        if [[ -n "$check_kind" ]]; then
            kind_json="$(check_kind_json "$check_kind")"
            payload="$(jq -cn \
                --arg sid "$SPEC_ID" \
                --arg task_id "$task_id" \
                --argjson kind "$kind_json" \
                '{
                schema_version: "1.0.0",
                spec_id: $sid,
                status: "open",
                severity: "fix_now",
                scope: "task",
                task_id: $task_id,
                check_kind: $kind
            }')"
        else
            payload="$(jq -cn \
                --arg sid "$SPEC_ID" \
                --arg task_id "$task_id" \
                '{
                schema_version: "1.0.0",
                spec_id: $sid,
                status: "open",
                severity: "fix_now",
                scope: "task",
                task_id: $task_id
            }')"
        fi
    else
        if [[ -n "$check_kind" ]]; then
            kind_json="$(check_kind_json "$check_kind")"
            payload="$(jq -cn \
                --arg sid "$SPEC_ID" \
                --argjson kind "$kind_json" \
                '{
                schema_version: "1.0.0",
                spec_id: $sid,
                status: "open",
                severity: "fix_now",
                scope: "spec",
                check_kind: $kind
            }')"
        else
            payload="$(jq -cn \
                --arg sid "$SPEC_ID" \
                '{
                schema_version: "1.0.0",
                spec_id: $sid,
                status: "open",
                severity: "fix_now",
                scope: "spec"
            }')"
        fi
    fi
    tanren-cli --database-url "$DATABASE_URL" methodology \
        --methodology-config "$CONFIG_PATH" \
        --phase investigate \
        finding list \
        --json "$payload" |
        jq '[.findings[].finding.id]'
}

prior_attempts_json() {
    local fingerprint="$1"
    local source_check="$2"
    local payload
    payload="$(jq -cn \
        --arg sid "$SPEC_ID" \
        --arg fingerprint "$fingerprint" \
        --argjson source_check "$source_check" \
        '{
            schema_version: "1.0.0",
            spec_id: $sid,
            fingerprint: $fingerprint,
            source_check: $source_check
        }')"
    tanren-cli --database-url "$DATABASE_URL" methodology \
        --methodology-config "$CONFIG_PATH" \
        --phase investigate \
        investigation list-attempts \
        --json "$payload" |
        jq '.attempts'
}

load_phase_capability_map() {
    tanren-cli --database-url "$DATABASE_URL" methodology \
        --methodology-config "$CONFIG_PATH" \
        phase-capabilities
}

phase_capabilities_csv() {
    local phase="$1"
    local csv
    csv="$(jq -r --arg phase "$phase" '.phases[] | select(.phase == $phase) | .capabilities_csv' <<<"$PHASE_CAPABILITY_MAP_JSON")"
    [[ -n "$csv" ]] || die "phase ${phase} is not present in canonical phase-capability map"
    printf '%s' "$csv"
}

mint_capability_envelope() {
    local phase="$1"
    local session_id="$2"
    local capabilities_csv="$3"
    local diagnostics_flag=""
    if [[ "${OUTPUT_MODE:-silent}" == "verbose" ]]; then
        diagnostics_flag="--diagnostics-stderr"
    fi
    cargo run --quiet -p tanren-xtask -- mint-mcp-capability-envelope \
        --private-key-pem "${MCP_CAPABILITY_PRIVATE_KEY_FILE}" \
        --issuer "${MCP_CAPABILITY_ISSUER}" \
        --audience "${MCP_CAPABILITY_AUDIENCE}" \
        --phase "${phase}" \
        --spec-id "${SPEC_ID}" \
        --agent-session-id "${session_id}" \
        --capabilities "${capabilities_csv}" \
        --requested-ttl "${MCP_CAPABILITY_MAX_TTL_SECS}" \
        --max-ttl "${MCP_CAPABILITY_MAX_TTL_SECS}" \
        ${diagnostics_flag:+$diagnostics_flag} \
        --token-only
}

run_harness_phase() {
    local phase="$1"
    local task_id="${2:-}"
    local allow_fail="${3:-0}"
    local context_file="${4:-}"
    local prompt_file="${RUN_DIR}/prompts/${CYCLE}-${phase}.md"
    mkdir -p "$(dirname "$prompt_file")"

    local task_line=""
    if [[ -n "$task_id" ]]; then
        task_line="Target task_id: ${task_id}"
    fi
    local context_line=""
    local context_instructions=""
    if [[ -n "$context_file" ]]; then
        context_line="Context bundle index: ${context_file}"
        context_instructions=$'- Read the bundle index and every referenced artifact in full before diagnosing root cause.\n- Use the bundle evidence as the authoritative failure context for this run.'
    fi

    cat >"$prompt_file" <<EOF2
Run Tanren phase \`${phase}\` for spec \`${SPEC_ID}\`.
Spec folder: \`${SPEC_FOLDER}\`
Database URL: \`${DATABASE_URL}\`
${task_line}
${context_line}

Requirements:
- Use Tanren MCP tools for all structured state changes.
- If MCP is unavailable, use Tanren CLI with canonical globals:
  tanren-cli --database-url "${DATABASE_URL}" methodology --phase "${phase}" --spec-id "${SPEC_ID}" --spec-folder "${SPEC_FOLDER}" <noun> <verb> --params-file '<payload.json>'
- Complete this phase fully and emit \`report_phase_outcome\`.
- If blocked, emit a typed blocked outcome (or investigate escalation path).
- Never hand-edit orchestrator-owned artifacts.
${context_instructions}
EOF2

    if [[ "$DRY_RUN" == "1" ]]; then
        if [[ "$OUTPUT_MODE" == "verbose" ]]; then
            print_line "[dry-run] harness phase ${phase} (prompt: ${prompt_file})"
        elif [[ "$OUTPUT_MODE" == "quiet" ]]; then
            print_line "[dry-run] harness phase ${phase}"
        fi
        return 0
    fi

    local capabilities_csv
    capabilities_csv="$(phase_capabilities_csv "$phase")"
    local session_id="${RUN_STAMP}-${CYCLE}-${phase}"
    local envelope
    envelope="$(mint_capability_envelope "$phase" "$session_id" "$capabilities_csv")"

    local cmd="TANREN_CONFIG=$(printf '%q' "$CONFIG_PATH") "
    cmd+="TANREN_SPEC_FOLDER=$(printf '%q' "$SPEC_FOLDER") "
    cmd+="TANREN_CLI=$(printf '%q' "$TANREN_CLI") "
    cmd+="TANREN_DATABASE_URL=$(printf '%q' "$DATABASE_URL") "
    cmd+="TANREN_MCP_CAPABILITY_ENVELOPE=$(printf '%q' "$envelope") "
    cmd+="TANREN_MCP_CAPABILITY_ISSUER=$(printf '%q' "$MCP_CAPABILITY_ISSUER") "
    cmd+="TANREN_MCP_CAPABILITY_AUDIENCE=$(printf '%q' "$MCP_CAPABILITY_AUDIENCE") "
    cmd+="TANREN_MCP_CAPABILITY_PUBLIC_KEY_FILE=$(printf '%q' "$MCP_CAPABILITY_PUBLIC_KEY_FILE") "
    cmd+="TANREN_MCP_CAPABILITY_MAX_TTL_SECS=$(printf '%q' "$MCP_CAPABILITY_MAX_TTL_SECS") "
    cmd+="$HARNESS_CMD"
    if [[ -n "$HARNESS_MODEL" ]]; then
        cmd+=" --model $(printf '%q' "$HARNESS_MODEL")"
    fi
    cmd+=" $(printf '%q' "$(cat "$prompt_file")")"
    run_shell_command "harness:${phase}" "$cmd" "$allow_fail"
}

report_phase_outcome_cli() {
    local phase="$1"
    local outcome_json="$2"
    local task_id="${3:-}"
    local allow_fail="${4:-0}"
    local idempotency_key="${5:-}"
    local payload
    if [[ -n "$task_id" ]]; then
        payload="$(jq -cn --arg sid "$SPEC_ID" --arg tid "$task_id" --argjson outcome "$outcome_json" --arg idempotency_key "$idempotency_key" '
            ({
                schema_version: "1.0.0",
                spec_id: $sid,
                task_id: $tid,
                outcome: $outcome
            } + if $idempotency_key == "" then {} else {idempotency_key: $idempotency_key} end)
        ')"
    else
        payload="$(jq -cn --arg sid "$SPEC_ID" --argjson outcome "$outcome_json" --arg idempotency_key "$idempotency_key" '
            ({
                schema_version: "1.0.0",
                spec_id: $sid,
                outcome: $outcome
            } + if $idempotency_key == "" then {} else {idempotency_key: $idempotency_key} end)
        ')"
    fi
    local cmd
    cmd="TANREN_PHASE_CAPABILITIES=phase.outcome "
    cmd+="tanren-cli --database-url $(printf '%q' "$DATABASE_URL") methodology"
    cmd+=" --methodology-config $(printf '%q' "$CONFIG_PATH")"
    cmd+=" --phase $(printf '%q' "$phase")"
    cmd+=" --spec-id $(printf '%q' "$SPEC_ID")"
    cmd+=" --spec-folder $(printf '%q' "$SPEC_FOLDER")"
    cmd+=" phase outcome --json $(printf '%q' "$payload")"
    run_shell_command "phase_outcome_${phase}" "$cmd" "$allow_fail"
}

mark_task_guard() {
    local task_id="$1"
    local guard="$2"
    local allow_fail="${3:-0}"
    local payload
    payload="$(jq -cn --arg task_id "$task_id" --arg guard "$guard" '
        {
            schema_version: "1.0.0",
            task_id: $task_id,
            guard: (
                if ($guard == "gate_checked" or $guard == "audited" or $guard == "adherent")
                then $guard
                else { extra: $guard }
                end
            ),
            idempotency_key: $idempotency_key
        }
    ' --arg idempotency_key "phase0-${RUN_STAMP}-${CYCLE}-mark-${task_id}-${guard}")"
    local cmd
    cmd="tanren-cli --database-url $(printf '%q' "$DATABASE_URL") methodology"
    cmd+=" --methodology-config $(printf '%q' "$CONFIG_PATH")"
    cmd+=" --phase do-task"
    cmd+=" --spec-id $(printf '%q' "$SPEC_ID")"
    cmd+=" --spec-folder $(printf '%q' "$SPEC_FOLDER")"
    cmd+=" task guard --json $(printf '%q' "$payload")"
    run_shell_command "task_guard_${guard}" "$cmd" "$allow_fail"
}

reset_task_guards() {
    local task_id="$1"
    local reason="$2"
    local payload
    payload="$(jq -cn --arg task_id "$task_id" --arg reason "$reason" '
        {
            schema_version: "1.0.0",
            task_id: $task_id,
            reason: $reason
        }
    ')"
    local cmd
    cmd="tanren-cli --database-url $(printf '%q' "$DATABASE_URL") methodology"
    cmd+=" --methodology-config $(printf '%q' "$CONFIG_PATH")"
    cmd+=" --phase do-task"
    cmd+=" --spec-id $(printf '%q' "$SPEC_ID")"
    cmd+=" --spec-folder $(printf '%q' "$SPEC_FOLDER")"
    cmd+=" task reset-guards --json $(printf '%q' "$payload")"
    run_shell_command "task_reset_guards" "$cmd"
}

task_recovery_context_file() {
    local task_id="$1"
    printf '%s/recovery/task-%s.md' "$RUN_DIR" "$(sanitize_label "$task_id")"
}

spec_recovery_context_file() {
    printf '%s/recovery/spec.md' "$RUN_DIR"
}

resolve_extra_guard_hook() {
    local guard="$1"
    local var_key="task_check_hook_${guard}"
    local hook
    hook="$(trim_scalar "$(yaml_methodology_var "$CONFIG_PATH" "$var_key")")"
    [[ -n "$hook" ]] || die "missing required methodology.variables.${var_key} for required extra guard '${guard}'"
    printf '%s' "$hook"
}

extract_required_guards() {
    local status_json="$1"
    jq -r '
        .required_guards[]? |
        if type == "string" then .
        elif type == "object" then (.extra // empty)
        else empty
        end
    ' <<<"$status_json"
}

append_check_result() {
    local results_file="$1"
    local guard="$2"
    local check_id="$3"
    local status="$4"
    local label="$5"
    local log_file="$6"
    printf '%s\t%s\t%s\t%s\t%s\n' "$guard" "$check_id" "$status" "$label" "$log_file" >>"$results_file"
}

create_investigation_bundle() {
    local scope="$1"
    local source_phase="$2"
    local task_id="${3:-}"
    local results_file="$4"
    local bundle_stamp
    bundle_stamp="$(date -u +%Y%m%dT%H%M%SZ)"
    local suffix="spec"
    if [[ -n "$task_id" ]]; then
        suffix="$(sanitize_label "$task_id")"
    fi
    local bundle_dir="${RUN_DIR}/investigation-bundles/${bundle_stamp}-${scope}-${suffix}"
    local logs_dir="${bundle_dir}/logs"
    local index_file="${bundle_dir}/index.md"
    local envelope_file="${bundle_dir}/check-failure-envelope.json"
    mkdir -p "$logs_dir"
    cp "$results_file" "${bundle_dir}/checks.tsv"
    local check_kind="gate"
    local finding_check_kind="gate"
    case "$source_phase" in
        task_checks|spec_checks) finding_check_kind="" ;;
        audit-task|audit_spec|audit-spec) check_kind="audit"; finding_check_kind="audit" ;;
        adhere-task|adhere_spec|adhere-spec) check_kind="adherence"; finding_check_kind="adherence" ;;
        run-demo|run_demo) check_kind="demo"; finding_check_kind="demo" ;;
        spec-gate|spec_gate) check_kind="spec_gate"; finding_check_kind="spec_gate" ;;
    esac
    local envelope_scope fingerprint source_check finding_ids prior_attempts
    if [[ -n "$task_id" ]]; then
        envelope_scope="$(jq -cn --arg task_id "$task_id" '{scope:"task", task_id:$task_id}')"
        fingerprint="${source_phase}:${task_id}"
        source_check="$(source_check_json "$source_phase" "$check_kind" "$task_id")"
        finding_ids="$(open_finding_ids_json "$scope" "$finding_check_kind" "$task_id")"
    else
        envelope_scope="$(jq -cn '{scope:"spec"}')"
        fingerprint="${source_phase}:spec"
        source_check="$(source_check_json "$source_phase" "$check_kind")"
        finding_ids="$(open_finding_ids_json "$scope" "$finding_check_kind")"
    fi
    prior_attempts="$(prior_attempts_json "$fingerprint" "$source_check")"
    if [[ -n "$task_id" ]]; then
        jq -cn \
            --arg sid "$SPEC_ID" \
            --argjson scope "$envelope_scope" \
            --arg fingerprint "$fingerprint" \
            --argjson loop_index "$CYCLE" \
            --arg index "$index_file" \
            --arg checks "${bundle_dir}/checks.tsv" \
            --argjson source_check "$source_check" \
            --argjson source_finding_ids "$finding_ids" \
            --argjson prior_attempts "$prior_attempts" \
            '{
              schema_version: "1.0.0",
              spec_id: $sid,
              scope: $scope,
              source_check: $source_check,
              source_finding_ids: $source_finding_ids,
              evidence_refs: [$index, $checks],
              prior_attempts: $prior_attempts,
              fingerprint: $fingerprint,
              loop_index: $loop_index
            }' >"$envelope_file"
    else
        jq -cn \
            --arg sid "$SPEC_ID" \
            --argjson scope "$envelope_scope" \
            --arg fingerprint "$fingerprint" \
            --argjson loop_index "$CYCLE" \
            --arg index "$index_file" \
            --arg checks "${bundle_dir}/checks.tsv" \
            --argjson source_check "$source_check" \
            --argjson source_finding_ids "$finding_ids" \
            --argjson prior_attempts "$prior_attempts" \
            '{
              schema_version: "1.0.0",
              spec_id: $sid,
              scope: $scope,
              source_check: $source_check,
              source_finding_ids: $source_finding_ids,
              evidence_refs: [$index, $checks],
              prior_attempts: $prior_attempts,
              fingerprint: $fingerprint,
              loop_index: $loop_index
            }' >"$envelope_file"
    fi

    {
        printf '# Investigation Bundle\n\n'
        printf -- '- cycle: %s\n' "$CYCLE"
        printf -- '- spec_id: %s\n' "$SPEC_ID"
        printf -- '- scope: %s\n' "$scope"
        if [[ -n "$task_id" ]]; then
            printf -- '- task_id: %s\n' "$task_id"
        fi
        printf -- '- source_phase: %s\n\n' "$source_phase"
        printf -- '- check_failure_envelope: %s\n\n' "$envelope_file"
        printf '## Check Results\n\n'
        printf '| Guard | Check | Status | Label | Full Log |\n'
        printf '| --- | --- | --- | --- | --- |\n'

        local row_index=0
        local failed_count=0
        while IFS=$'\t' read -r guard check_id status label log_file; do
            [[ -n "$guard" ]] || continue
            row_index=$((row_index + 1))
            local log_name
            log_name="$(printf '%02d-%s-%s.log' "$row_index" "$(sanitize_label "$guard")" "$(sanitize_label "$check_id")")"
            local copied_log="${logs_dir}/${log_name}"
            if [[ -n "$log_file" && -f "$log_file" ]]; then
                cp "$log_file" "$copied_log"
            else
                printf 'missing log for %s (%s)\n' "$guard" "$check_id" >"$copied_log"
            fi
            printf '| `%s` | `%s` | `%s` | %s | `%s` |\n' \
                "$guard" "$check_id" "$status" "$label" "$copied_log"
            if [[ "$status" != "pass" ]]; then
                failed_count=$((failed_count + 1))
            fi
        done <"$results_file"

        printf '\n## Failed Checks\n\n'
        if ((failed_count == 0)); then
            printf -- '- none\n'
        else
            while IFS=$'\t' read -r guard check_id status label log_file; do
                [[ -n "$guard" ]] || continue
                if [[ "$status" != "pass" ]]; then
                    printf -- '- `%s` / `%s` => `%s` (%s)\n' "$guard" "$check_id" "$status" "$label"
                fi
            done <"$results_file"
        fi
    } >"$index_file"

    printf '%s' "$bundle_dir"
}

run_task_check_batch() {
    local task_id="$1"
    local task_index="$2"
    local task_total="$3"
    local status_json="$4"

    local guards_to_run_raw
    guards_to_run_raw="$(jq -r '
        .pending_task_checks[]? |
        if type == "string" then .
        elif type == "object" then (.extra // empty)
        else empty
        end
    ' <<<"$status_json" | sed '/^$/d')"
    [[ -n "$guards_to_run_raw" ]] || die "task ${task_id} has no pending guards to execute in batch"

    local batch_dir="${RUN_DIR}/check-batches"
    mkdir -p "$batch_dir"
    local results_file="${batch_dir}/${CYCLE}-$(sanitize_label "$task_id").tsv"
    : >"$results_file"

    local failed_checks=0

    while IFS= read -r guard; do
        [[ -n "$guard" ]] || continue
        case "$guard" in
            gate_checked)
                if [[ "$OUTPUT_MODE" == "silent" ]]; then
                    print_line "task ${task_index}/${task_total} - task_gate hook start: task_verification_hook"
                fi
                local gate_hook_ok=0
                if run_hook "task_verification_hook" "$TASK_HOOK" "1"; then
                    append_check_result "$results_file" "$guard" "task_verification_hook" "pass" "task verification hook" "$LAST_COMMAND_LOG"
                    gate_hook_ok=1
                else
                    append_check_result "$results_file" "$guard" "task_verification_hook" "fail" "task verification hook failed" "$LAST_COMMAND_LOG"
                    failed_checks=$((failed_checks + 1))
                fi

                if ((gate_hook_ok == 1)); then
                    if mark_task_guard "$task_id" "gate_checked" "1"; then
                        append_check_result "$results_file" "$guard" "mark_task_guard_gate_checked" "pass" "mark gate_checked guard" "$LAST_COMMAND_LOG"
                    else
                        append_check_result "$results_file" "$guard" "mark_task_guard_gate_checked" "fail" "mark gate_checked guard failed" "$LAST_COMMAND_LOG"
                        failed_checks=$((failed_checks + 1))
                    fi
                fi
                ;;
            audited)
                if [[ "$OUTPUT_MODE" == "silent" ]]; then
                    print_line "task ${task_index}/${task_total} - task_audit start"
                fi
                if run_harness_phase "audit-task" "$task_id" "1"; then
                    append_check_result "$results_file" "$guard" "audit-task" "pass" "audit-task phase" "$LAST_COMMAND_LOG"
                else
                    append_check_result "$results_file" "$guard" "audit-task" "fail" "audit-task phase failed" "$LAST_COMMAND_LOG"
                    failed_checks=$((failed_checks + 1))
                fi
                if run_hook "audit_task_hook" "$AUDIT_TASK_HOOK" "1"; then
                    append_check_result "$results_file" "$guard" "audit_task_hook" "pass" "audit_task_hook" "$LAST_COMMAND_LOG"
                else
                    append_check_result "$results_file" "$guard" "audit_task_hook" "fail" "audit_task_hook failed" "$LAST_COMMAND_LOG"
                    failed_checks=$((failed_checks + 1))
                fi
                ;;
            adherent)
                if [[ "$OUTPUT_MODE" == "silent" ]]; then
                    print_line "task ${task_index}/${task_total} - task_adhere start"
                fi
                if run_harness_phase "adhere-task" "$task_id" "1"; then
                    append_check_result "$results_file" "$guard" "adhere-task" "pass" "adhere-task phase" "$LAST_COMMAND_LOG"
                else
                    append_check_result "$results_file" "$guard" "adhere-task" "fail" "adhere-task phase failed" "$LAST_COMMAND_LOG"
                    failed_checks=$((failed_checks + 1))
                fi
                if run_hook "adhere_task_hook" "$ADHERE_TASK_HOOK" "1"; then
                    append_check_result "$results_file" "$guard" "adhere_task_hook" "pass" "adhere_task_hook" "$LAST_COMMAND_LOG"
                else
                    append_check_result "$results_file" "$guard" "adhere_task_hook" "fail" "adhere_task_hook failed" "$LAST_COMMAND_LOG"
                    failed_checks=$((failed_checks + 1))
                fi
                ;;
            *)
                local extra_hook
                extra_hook="$(resolve_extra_guard_hook "$guard")"
                local extra_hook_ok=0
                if run_hook "task_check_hook_${guard}" "$extra_hook" "1"; then
                    append_check_result "$results_file" "$guard" "task_check_hook_${guard}" "pass" "extra guard hook" "$LAST_COMMAND_LOG"
                    extra_hook_ok=1
                else
                    append_check_result "$results_file" "$guard" "task_check_hook_${guard}" "fail" "extra guard hook failed" "$LAST_COMMAND_LOG"
                    failed_checks=$((failed_checks + 1))
                fi
                if ((extra_hook_ok == 1)); then
                    if mark_task_guard "$task_id" "$guard" "1"; then
                        append_check_result "$results_file" "$guard" "mark_task_guard_${guard}" "pass" "mark extra guard" "$LAST_COMMAND_LOG"
                    else
                        append_check_result "$results_file" "$guard" "mark_task_guard_${guard}" "fail" "mark extra guard failed" "$LAST_COMMAND_LOG"
                        failed_checks=$((failed_checks + 1))
                    fi
                fi
                ;;
        esac
    done <<<"$guards_to_run_raw"

    if ((failed_checks > 0)); then
        local bundle_dir
        bundle_dir="$(create_investigation_bundle "task" "task_checks" "$task_id" "$results_file")"
        local bundle_index="${bundle_dir}/index.md"
        local bundle_envelope="${bundle_dir}/check-failure-envelope.json"
        print_line "task ${task_index}/${task_total} - check batch failed (${failed_checks}); bundle: ${bundle_index}"
        reset_task_guards "$task_id" "task check batch failed in cycle ${CYCLE}; see ${bundle_index}"
        local recovery_context_file
        recovery_context_file="$(task_recovery_context_file "$task_id")"
        mkdir -p "$(dirname "$recovery_context_file")"
        cat >"$recovery_context_file" <<EOF2
Latest same-task repair context for task ${task_id}
- cycle: ${CYCLE}
- source_task_id: ${task_id}
- failed_checks: ${failed_checks}
- bundle_index: ${bundle_index}
- check_failure_envelope: ${bundle_envelope}
- repair_target: edit this same task; do not create a remediation task
EOF2
        run_investigate_for_failure "task" "task_checks_batch" "$task_id" "$bundle_index"
        return 1
    fi

    local recovery_context_file
    recovery_context_file="$(task_recovery_context_file "$task_id")"
    rm -f "$recovery_context_file"
    if [[ "$OUTPUT_MODE" == "silent" ]]; then
        print_line "task ${task_index}/${task_total} - task checks complete"
    fi
    return 0
}

run_spec_check_batch() {
    local status_json="$1"
    local batch_dir="${RUN_DIR}/check-batches"
    mkdir -p "$batch_dir"
    local results_file="${batch_dir}/${CYCLE}-spec.tsv"
    : >"$results_file"

    local checks_to_run
    checks_to_run="$(jq -r '.pending_spec_checks[]?' <<<"$status_json" | sed '/^$/d')"
    [[ -n "$checks_to_run" ]] || die "spec check batch has no pending checks to execute"

    local failed_checks=0
    local check

    if [[ "$OUTPUT_MODE" == "silent" ]]; then
        print_line "spec - spec_checks batch start"
    fi

    while IFS= read -r check; do
        [[ -n "$check" ]] || continue
        case "$check" in
            spec_gate)
                if [[ "$OUTPUT_MODE" == "silent" ]]; then
                    print_line "spec - spec_gate hook start: spec_verification_hook"
                fi
                if run_hook "spec_verification_hook" "$SPEC_HOOK" "1"; then
                    append_check_result "$results_file" "$check" "spec_verification_hook" "pass" "spec verification hook" "$LAST_COMMAND_LOG"
                    local gate_complete
                    gate_complete="$(jq -cn '{"outcome":"complete","summary":"spec gate passed"}')"
                    if ! report_phase_outcome_cli "spec-gate" "$gate_complete" "" "1" "phase0-${RUN_STAMP}-${CYCLE}-spec-gate-complete"; then
                        append_check_result "$results_file" "$check" "phase_outcome_spec_gate" "fail" "record spec-gate complete outcome failed" "$LAST_COMMAND_LOG"
                        failed_checks=$((failed_checks + 1))
                    else
                        append_check_result "$results_file" "$check" "phase_outcome_spec_gate" "pass" "record spec-gate complete outcome" "$LAST_COMMAND_LOG"
                    fi
                else
                    append_check_result "$results_file" "$check" "spec_verification_hook" "fail" "spec verification hook failed" "$LAST_COMMAND_LOG"
                    failed_checks=$((failed_checks + 1))
                    local gate_error
                    gate_error="$(jq -cn '{"outcome":"error","reason":{"kind":"other","detail":"spec gate hook failed"},"summary":"spec gate failed"}')"
                    if ! report_phase_outcome_cli "spec-gate" "$gate_error" "" "1" "phase0-${RUN_STAMP}-${CYCLE}-spec-gate-error"; then
                        append_check_result "$results_file" "$check" "phase_outcome_spec_gate" "fail" "record spec-gate error outcome failed" "$LAST_COMMAND_LOG"
                        failed_checks=$((failed_checks + 1))
                    else
                        append_check_result "$results_file" "$check" "phase_outcome_spec_gate" "pass" "record spec-gate error outcome" "$LAST_COMMAND_LOG"
                    fi
                fi
                ;;
            run_demo)
                if [[ "$OUTPUT_MODE" == "silent" ]]; then
                    print_line "spec - run_demo start"
                fi
                if run_harness_phase "run-demo" "" "1"; then
                    append_check_result "$results_file" "$check" "run-demo" "pass" "run-demo phase" "$LAST_COMMAND_LOG"
                else
                    append_check_result "$results_file" "$check" "run-demo" "fail" "run-demo phase failed" "$LAST_COMMAND_LOG"
                    failed_checks=$((failed_checks + 1))
                    local run_demo_error
                    run_demo_error="$(jq -cn '{"outcome":"error","reason":{"kind":"other","detail":"run-demo check failed"},"summary":"run-demo check failed"}')"
                    if ! report_phase_outcome_cli "run-demo" "$run_demo_error" "" "1"; then
                        append_check_result "$results_file" "$check" "phase_outcome_run_demo" "fail" "record run-demo error outcome failed" "$LAST_COMMAND_LOG"
                        failed_checks=$((failed_checks + 1))
                    else
                        append_check_result "$results_file" "$check" "phase_outcome_run_demo" "pass" "record run-demo error outcome" "$LAST_COMMAND_LOG"
                    fi
                fi
                ;;
            audit_spec)
                if [[ "$OUTPUT_MODE" == "silent" ]]; then
                    print_line "spec - audit_spec start"
                fi
                if run_harness_phase "audit-spec" "" "1"; then
                    append_check_result "$results_file" "$check" "audit-spec" "pass" "audit-spec phase" "$LAST_COMMAND_LOG"
                else
                    append_check_result "$results_file" "$check" "audit-spec" "fail" "audit-spec phase failed" "$LAST_COMMAND_LOG"
                    failed_checks=$((failed_checks + 1))
                    local audit_spec_error
                    audit_spec_error="$(jq -cn '{"outcome":"error","reason":{"kind":"other","detail":"audit-spec check failed"},"summary":"audit-spec check failed"}')"
                    if ! report_phase_outcome_cli "audit-spec" "$audit_spec_error" "" "1"; then
                        append_check_result "$results_file" "$check" "phase_outcome_audit_spec" "fail" "record audit-spec error outcome failed" "$LAST_COMMAND_LOG"
                        failed_checks=$((failed_checks + 1))
                    else
                        append_check_result "$results_file" "$check" "phase_outcome_audit_spec" "pass" "record audit-spec error outcome" "$LAST_COMMAND_LOG"
                    fi
                fi
                ;;
            adhere_spec)
                if [[ "$OUTPUT_MODE" == "silent" ]]; then
                    print_line "spec - adhere_spec start"
                fi
                if run_harness_phase "adhere-spec" "" "1"; then
                    append_check_result "$results_file" "$check" "adhere-spec" "pass" "adhere-spec phase" "$LAST_COMMAND_LOG"
                else
                    append_check_result "$results_file" "$check" "adhere-spec" "fail" "adhere-spec phase failed" "$LAST_COMMAND_LOG"
                    failed_checks=$((failed_checks + 1))
                    local adhere_spec_error
                    adhere_spec_error="$(jq -cn '{"outcome":"error","reason":{"kind":"other","detail":"adhere-spec check failed"},"summary":"adhere-spec check failed"}')"
                    if ! report_phase_outcome_cli "adhere-spec" "$adhere_spec_error" "" "1"; then
                        append_check_result "$results_file" "$check" "phase_outcome_adhere_spec" "fail" "record adhere-spec error outcome failed" "$LAST_COMMAND_LOG"
                        failed_checks=$((failed_checks + 1))
                    else
                        append_check_result "$results_file" "$check" "phase_outcome_adhere_spec" "pass" "record adhere-spec error outcome" "$LAST_COMMAND_LOG"
                    fi
                fi
                ;;
            *)
                die "unknown spec check: ${check}"
                ;;
        esac
    done <<<"$checks_to_run"

    if ((failed_checks > 0)); then
        local bundle_dir
        bundle_dir="$(create_investigation_bundle "spec" "spec_checks" "" "$results_file")"
        local bundle_index="${bundle_dir}/index.md"
        print_line "spec - spec_checks batch failed (${failed_checks}); bundle: ${bundle_index}"
        local recovery_context_file
        recovery_context_file="$(spec_recovery_context_file)"
        mkdir -p "$(dirname "$recovery_context_file")"
        cat >"$recovery_context_file" <<EOF2
Latest remediation context for spec ${SPEC_ID}
- cycle: ${CYCLE}
- failed_checks: ${failed_checks}
- bundle_index: ${bundle_index}
EOF2
        run_investigate_for_failure "spec" "spec_checks_batch" "" "$bundle_index"
        return 1
    fi

    local recovery_context_file
    recovery_context_file="$(spec_recovery_context_file)"
    rm -f "$recovery_context_file"
    if [[ "$OUTPUT_MODE" == "silent" ]]; then
        print_line "spec - spec checks complete"
    fi
    return 0
}

run_investigate_for_failure() {
    local scope="$1"
    local source_phase="$2"
    local task_id="${3:-}"
    local context_file="${4:-}"

    if [[ "$scope" == "task" ]]; then
        if [[ -n "$context_file" ]]; then
            print_line "routing ${source_phase} failure to investigate (task_id=${task_id}, bundle=${context_file})"
            run_harness_phase "investigate" "$task_id" "0" "$context_file"
        else
            print_line "routing ${source_phase} failure to investigate (task_id=${task_id})"
            run_harness_phase "investigate" "$task_id"
        fi
    else
        if [[ -n "$context_file" ]]; then
            print_line "routing ${source_phase} failure to investigate (spec scope, bundle=${context_file})"
            run_harness_phase "investigate" "" "0" "$context_file"
        else
            print_line "routing ${source_phase} failure to investigate (spec scope)"
            run_harness_phase "investigate"
        fi
    fi
}

prompt_checkpoint() {
    local headline="$1"
    local detail="$2"
    finish_silent_line
    printf '\n[phase0] %s\n%s\n\n' "$headline" "$detail"
}

phase_step_verb() {
    case "$1" in
        task_do) printf 'implementing' ;;
        task_check_batch) printf 'batch-checking' ;;
        task_investigate) printf 'investigating' ;;
        spec_check_batch) printf 'batch-checking' ;;
        spec_investigate) printf 'investigating' ;;
        walk_spec_required) printf 'walk-ready' ;;
        resolve_blockers_required) printf 'blocked' ;;
        complete) printf 'complete' ;;
        *) printf 'working' ;;
    esac
}

quiet_task_summary() {
    local cycle="$1"
    local task_idx="$2"
    local total="$3"
    local step="$4"
    local task_id="$5"
    local task_title="$6"
    local task_description="$7"
    local deliverable="$8"
    local reason="$9"

    print_line "cycle ${cycle}: task ${task_idx}/${total} - ${step}"
    print_line "  id: ${task_id}"
    print_line "  title: ${task_title}"
    if [[ -n "$task_description" ]]; then
        print_line "  definition: ${task_description}"
    fi
    if [[ -n "$deliverable" ]]; then
        print_line "  deliverable: ${deliverable}"
    fi
    if [[ -n "$reason" ]]; then
        print_line "  routing: ${reason}"
    fi
}

SPEC_ID=""
SPEC_FOLDER=""
DATABASE_URL="sqlite:tanren.db"
CONFIG_PATH="tanren.yml"
HARNESS_CMD="codex exec"
HARNESS_MODEL="${TANREN_PHASE0_HARNESS_MODEL:-}"
OUTPUT_MODE="${TANREN_PHASE0_OUTPUT_MODE:-silent}"
STATUS_PHASE="do-task"
MAX_CYCLES=64
DRY_RUN=0
SILENT_LINE_ACTIVE=0
COMMAND_INDEX=0
LAST_COMMAND_LOG=""
RUN_DIR=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --spec-id)
            SPEC_ID="${2:-}"
            shift 2
            ;;
        --spec-folder)
            SPEC_FOLDER="${2:-}"
            shift 2
            ;;
        --database-url)
            DATABASE_URL="${2:-}"
            shift 2
            ;;
        --config)
            CONFIG_PATH="${2:-}"
            shift 2
            ;;
        --harness-cmd)
            die "--harness-cmd is no longer supported in Phase 0 acceptance mode; harness is hard-locked to 'codex exec'"
            ;;
        --harness-model)
            HARNESS_MODEL="${2:-}"
            shift 2
            ;;
        --output-mode)
            OUTPUT_MODE="${2:-}"
            shift 2
            ;;
        --max-cycles)
            MAX_CYCLES="${2:-}"
            shift 2
            ;;
        --dry-run)
            DRY_RUN=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            die "unknown argument: $1"
            ;;
    esac
done

[[ -n "$SPEC_ID" ]] || {
    usage
    die "--spec-id is required"
}
[[ -f "$CONFIG_PATH" ]] || die "config not found: $CONFIG_PATH"
DATABASE_URL="$(normalize_database_url "$DATABASE_URL")"

case "$OUTPUT_MODE" in
    silent|quiet|verbose)
        ;;
    *)
        die "invalid --output-mode '${OUTPUT_MODE}' (expected silent|quiet|verbose)"
        ;;
esac

if [[ -n "${TANREN_PHASE0_HARNESS_CMD:-}" ]]; then
    die "TANREN_PHASE0_HARNESS_CMD override is no longer supported in Phase 0 acceptance mode; remove it and use the hard-locked 'codex exec' harness"
fi

need_cmd tanren-cli
need_cmd tanren-mcp
need_cmd jq
need_cmd cargo
need_cmd codex
TANREN_CLI="$(command -v tanren-cli)"

if [[ "$DRY_RUN" != "1" ]]; then
    run_shell_command "config-parse-check" "tanren-cli install --config $(printf '%q' "$CONFIG_PATH") --dry-run >/dev/null"
fi

PHASE_CAPABILITY_MAP_JSON="$(load_phase_capability_map)"

if [[ -z "$SPEC_FOLDER" ]]; then
    spec_root="$(trim_scalar "$(yaml_methodology_var "$CONFIG_PATH" "spec_root")")"
    [[ -n "$spec_root" ]] || spec_root="tanren/specs"
    SPEC_FOLDER="${spec_root}/${SPEC_ID}"
fi

TASK_HOOK="$(resolve_hook "task_verification_hook" "do-task" "just check")"
SPEC_HOOK="$(resolve_hook "spec_verification_hook" "run-demo" "just ci")"
AUDIT_TASK_HOOK="$(resolve_hook "audit_task_hook" "audit-task" "$TASK_HOOK")"
ADHERE_TASK_HOOK="$(resolve_hook "adhere_task_hook" "adhere-task" "$TASK_HOOK")"

MCP_CAPABILITY_ISSUER="${TANREN_MCP_CAPABILITY_ISSUER:-$(trim_scalar "$(yaml_mcp_security "$CONFIG_PATH" "capability_issuer")")}"
MCP_CAPABILITY_AUDIENCE="${TANREN_MCP_CAPABILITY_AUDIENCE:-$(trim_scalar "$(yaml_mcp_security "$CONFIG_PATH" "capability_audience")")}"
MCP_CAPABILITY_PUBLIC_KEY_FILE="${TANREN_MCP_CAPABILITY_PUBLIC_KEY_FILE:-$(trim_scalar "$(yaml_mcp_security "$CONFIG_PATH" "capability_public_key_file")")}"
MCP_CAPABILITY_PRIVATE_KEY_FILE="${TANREN_MCP_CAPABILITY_PRIVATE_KEY_FILE:-$(trim_scalar "$(yaml_mcp_security "$CONFIG_PATH" "capability_private_key_file")")}"
MCP_CAPABILITY_MAX_TTL_SECS="${TANREN_MCP_CAPABILITY_MAX_TTL_SECS:-$(trim_scalar "$(yaml_mcp_security "$CONFIG_PATH" "capability_max_ttl_secs")")}"
[[ -n "$MCP_CAPABILITY_MAX_TTL_SECS" ]] || MCP_CAPABILITY_MAX_TTL_SECS="900"

[[ -n "$MCP_CAPABILITY_ISSUER" ]] || die "missing methodology.mcp.security.capability_issuer"
[[ -n "$MCP_CAPABILITY_AUDIENCE" ]] || die "missing methodology.mcp.security.capability_audience"
[[ -n "$MCP_CAPABILITY_PUBLIC_KEY_FILE" ]] || die "missing methodology.mcp.security.capability_public_key_file"
[[ -n "$MCP_CAPABILITY_PRIVATE_KEY_FILE" ]] || die "missing methodology.mcp.security.capability_private_key_file"
if [[ "$DRY_RUN" != "1" ]]; then
    [[ -f "$MCP_CAPABILITY_PUBLIC_KEY_FILE" ]] || die "missing capability public key file: $MCP_CAPABILITY_PUBLIC_KEY_FILE"
    [[ -f "$MCP_CAPABILITY_PRIVATE_KEY_FILE" ]] || die "missing capability private key file: $MCP_CAPABILITY_PRIVATE_KEY_FILE"
fi

RUN_STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
RUN_DIR="${SPEC_FOLDER}/orchestration/phase0/${RUN_STAMP}"
mkdir -p "$RUN_DIR"

cat >"${RUN_DIR}/resolved-config.env" <<EOF3
SPEC_ID=${SPEC_ID}
SPEC_FOLDER=${SPEC_FOLDER}
DATABASE_URL=${DATABASE_URL}
CONFIG_PATH=${CONFIG_PATH}
HARNESS_CMD=${HARNESS_CMD}
HARNESS_MODEL=${HARNESS_MODEL}
OUTPUT_MODE=${OUTPUT_MODE}
TASK_HOOK=${TASK_HOOK}
SPEC_HOOK=${SPEC_HOOK}
AUDIT_TASK_HOOK=${AUDIT_TASK_HOOK}
ADHERE_TASK_HOOK=${ADHERE_TASK_HOOK}
MCP_CAPABILITY_ISSUER=${MCP_CAPABILITY_ISSUER}
MCP_CAPABILITY_AUDIENCE=${MCP_CAPABILITY_AUDIENCE}
MCP_CAPABILITY_PUBLIC_KEY_FILE=${MCP_CAPABILITY_PUBLIC_KEY_FILE}
MCP_CAPABILITY_PRIVATE_KEY_FILE=${MCP_CAPABILITY_PRIVATE_KEY_FILE}
MCP_CAPABILITY_MAX_TTL_SECS=${MCP_CAPABILITY_MAX_TTL_SECS}
EOF3

log_quiet "spec_id=${SPEC_ID}"
log_quiet "spec_folder=${SPEC_FOLDER}"
log_quiet "output_mode=${OUTPUT_MODE}"
log_quiet "run_dir=${RUN_DIR}"
log_verbose "harness=${HARNESS_CMD}${HARNESS_MODEL:+ (model=${HARNESS_MODEL})}"
log_verbose "task_hook=${TASK_HOOK}"
log_verbose "spec_hook=${SPEC_HOOK}"

last_signature=""
stagnant=0

for ((CYCLE = 1; CYCLE <= MAX_CYCLES; CYCLE++)); do
    log_verbose "cycle ${CYCLE}: querying spec status"
    status_json="$(spec_status_json)"
    printf '%s\n' "$status_json" >"${RUN_DIR}/last-status.json"
    printf '%s\n' "$status_json" >"${RUN_DIR}/status-cycle-${CYCLE}.json"

    transition="$(jq -r '.next_transition' <<<"$status_json")"
    signature="$(jq -c '{next_transition,next_task_id,pending_task_checks,pending_spec_checks,investigate_source_phase,investigate_source_outcome,investigate_source_task_id,total_tasks,pending_tasks,in_progress_tasks,implemented_tasks,completed_tasks,abandoned_tasks,blockers_active}' <<<"$status_json")"
    if [[ "$signature" == "$last_signature" ]]; then
        stagnant=$((stagnant + 1))
    else
        stagnant=0
    fi
    last_signature="$signature"
    if ((stagnant >= 3)); then
        die "orchestration made no state progress across 3 cycles; inspect ${RUN_DIR}/status-cycle-*.json and resolve manually"
    fi

    case "$transition" in
        shape_spec_required)
            prompt_checkpoint \
                "Spec Not Found (manual checkpoint: shape-spec)" \
                "Spec ${SPEC_ID} has no methodology state yet. Use your harness to run shape-spec, then re-run this orchestrator.

Suggested harness command:
  ${HARNESS_CMD} '/shape-spec for spec ${SPEC_ID} in ${SPEC_FOLDER}'

CLI command for typed mutations:
  ${TANREN_CLI} --database-url ${DATABASE_URL} methodology --methodology-config ${CONFIG_PATH} --phase shape-spec --spec-id ${SPEC_ID} --spec-folder ${SPEC_FOLDER} <noun> <verb> --params-file \"<payload.json>\""
            exit 20
            ;;
        resolve_blockers_required)
            last_blocker_phase="$(jq -r '.last_blocker_phase // empty' <<<"$status_json")"
            if [[ -n "$last_blocker_phase" && "$last_blocker_phase" != "investigate" ]]; then
                die "spec status returned resolve_blockers_required from non-investigate phase (${last_blocker_phase}); installed tanren appears stale. Reinstall tanren, then re-run this orchestrator."
            fi
            prompt_checkpoint \
                "Blocker Halt (manual checkpoint: resolve-blockers)" \
                "Spec ${SPEC_ID} is blocked. Run resolve-blockers with your harness, then re-run this orchestrator.

Suggested harness command:
  ${HARNESS_CMD} '/resolve-blockers for spec ${SPEC_ID} in ${SPEC_FOLDER}'"
            exit 30
            ;;
        walk_spec_required)
            prompt_checkpoint \
                "Walk-Spec Ready (manual checkpoint: walk-spec)" \
                "Autonomous phases and configured checks converged. Run walk-spec manually to validate readiness.

Suggested harness command:
  ${HARNESS_CMD} '/walk-spec for spec ${SPEC_ID} in ${SPEC_FOLDER}'

After walk-spec completes, rerun this script to confirm final status."
            exit 40
            ;;
        complete)
            print_line "spec ${SPEC_ID} already completed walk-spec; nothing else to run"
            exit 0
            ;;
        task_do|task_check_batch|task_investigate)
            next_task_id="$(jq -r '.next_task_id // empty' <<<"$status_json")"
            [[ -n "$next_task_id" ]] || die "transition ${transition} requires next_task_id"
            transition_reason="$(jq -r '.transition_reason // empty' <<<"$status_json")"
            tasks_json="$(list_tasks_json)"
            task_total="$(jq -r '.total_tasks' <<<"$status_json")"
            task_index="$(jq -r --arg tid "$next_task_id" '.tasks | to_entries[] | select(.value.id == $tid) | (.key + 1)' <<<"$tasks_json")"
            task_title="$(jq -r --arg tid "$next_task_id" '.tasks[] | select(.id == $tid) | .title' <<<"$tasks_json")"
            task_description="$(jq -r --arg tid "$next_task_id" '.tasks[] | select(.id == $tid) | .description // empty' <<<"$tasks_json")"
            task_deliverable="$(jq -r --arg tid "$next_task_id" '.tasks[] | select(.id == $tid) | ((.acceptance_criteria[0].measurable // .acceptance_criteria[0].description) // empty)' <<<"$tasks_json")"
            phase_verb="$(phase_step_verb "$transition")"

            if [[ "$OUTPUT_MODE" == "silent" ]]; then
                if [[ "$transition" == "task_check_batch" ]]; then
                    print_line "task ${task_index}/${task_total} - task_checks (batch-checking)"
                else
                    print_progress "task ${task_index}/${task_total} - ${transition} (${phase_verb})"
                fi
            elif [[ "$OUTPUT_MODE" == "quiet" ]]; then
                quiet_task_summary "$CYCLE" "$task_index" "$task_total" "$phase_verb" "$next_task_id" "$task_title" "$task_description" "$task_deliverable" "$transition_reason"
            else
                print_line "cycle ${CYCLE}: task ${task_index}/${task_total} transition=${transition} task_id=${next_task_id}"
                if [[ -n "$transition_reason" ]]; then
                    print_line "routing: ${transition_reason}"
                fi
            fi

            step_failed=0
            case "$transition" in
                task_do)
                    do_task_context_file="$(task_recovery_context_file "$next_task_id")"
                    if [[ -f "$do_task_context_file" ]]; then
                        print_line "task ${task_index}/${task_total} - task_do using recovery context ${do_task_context_file}"
                        if ! run_harness_phase "do-task" "$next_task_id" "1" "$do_task_context_file"; then
                            run_investigate_for_failure "task" "do-task" "$next_task_id" "$do_task_context_file"
                            step_failed=1
                        fi
                    elif ! run_harness_phase "do-task" "$next_task_id" "1"; then
                        run_investigate_for_failure "task" "do-task" "$next_task_id"
                        step_failed=1
                    fi
                    ;;
                task_check_batch)
                    if [[ "$OUTPUT_MODE" == "silent" ]]; then
                        print_line "task ${task_index}/${task_total} - task_checks batch start"
                    fi
                    if ! run_task_check_batch "$next_task_id" "$task_index" "$task_total" "$status_json"; then
                        step_failed=1
                    fi
                    ;;
                task_investigate)
                    task_investigate_context_file="$(task_recovery_context_file "$next_task_id")"
                    if [[ -f "$task_investigate_context_file" ]]; then
                        run_harness_phase "investigate" "$next_task_id" "0" "$task_investigate_context_file"
                    else
                        run_harness_phase "investigate" "$next_task_id"
                    fi
                    ;;
                *)
                    die "unsupported task transition: ${transition}"
                    ;;
            esac
            if ((step_failed == 1)); then
                continue
            fi
            if [[ "$DRY_RUN" == "1" ]]; then
                finish_silent_line
                print_line "dry-run completed one simulated autonomous cycle; exiting without persistence"
                exit 0
            fi
            ;;
        spec_check_batch)
            transition_reason="$(jq -r '.transition_reason // empty' <<<"$status_json")"
            phase_verb="$(phase_step_verb "$transition")"
            if [[ "$OUTPUT_MODE" == "silent" ]]; then
                print_line "spec - spec_checks (batch-checking)"
            else
                print_progress "spec - ${transition} (${phase_verb})"
            fi
            if [[ "$OUTPUT_MODE" == "verbose" && -n "$transition_reason" ]]; then
                print_line "routing: ${transition_reason}"
            fi
            if ! run_spec_check_batch "$status_json"; then
                continue
            fi
            if [[ "$DRY_RUN" == "1" ]]; then
                finish_silent_line
                print_line "dry-run completed one simulated autonomous cycle; exiting without persistence"
                exit 0
            fi
            ;;
        spec_investigate)
            phase_verb="$(phase_step_verb "$transition")"
            print_progress "spec - ${transition} (${phase_verb})"
            spec_investigate_context_file="$(spec_recovery_context_file)"
            if [[ -f "$spec_investigate_context_file" ]]; then
                run_harness_phase "investigate" "" "0" "$spec_investigate_context_file"
            else
                run_harness_phase "investigate"
            fi
            if [[ "$DRY_RUN" == "1" ]]; then
                finish_silent_line
                print_line "dry-run completed one simulated autonomous cycle; exiting without persistence"
                exit 0
            fi
            ;;
        *)
            die "unknown next_transition from spec status: ${transition}"
            ;;
    esac
done

die "max cycles (${MAX_CYCLES}) reached without terminal checkpoint"

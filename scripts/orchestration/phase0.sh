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
                printf '\r[phase0] %s' "$message"
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
    local log_file="${RUN_DIR}/commands/$(printf '%03d' "$COMMAND_INDEX")-${safe_label}.log"
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
        die "${label} failed (exit ${status}); see ${log_file}"
    fi

    if [[ "$OUTPUT_MODE" == "quiet" ]]; then
        print_line "${label}: ok"
    fi
}

run_hook() {
    local hook_name="$1"
    local hook_cmd="$2"
    [[ -n "$hook_cmd" ]] || die "${hook_name} resolved to empty command"
    run_shell_command "$hook_name" "$hook_cmd"
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
    uv run python "${REPO_ROOT}/scripts/proof/phase0/mint_mcp_capability_envelope.py" \
        --private-key-pem "${MCP_CAPABILITY_PRIVATE_KEY_FILE}" \
        --issuer "${MCP_CAPABILITY_ISSUER}" \
        --audience "${MCP_CAPABILITY_AUDIENCE}" \
        --phase "${phase}" \
        --spec-id "${SPEC_ID}" \
        --agent-session-id "${session_id}" \
        --capabilities "${capabilities_csv}" \
        --requested-ttl "${MCP_CAPABILITY_MAX_TTL_SECS}" \
        --max-ttl "${MCP_CAPABILITY_MAX_TTL_SECS}" \
        --token-only
}

run_harness_phase() {
    local phase="$1"
    local task_id="${2:-}"
    local prompt_file="${RUN_DIR}/prompts/${CYCLE}-${phase}.md"
    mkdir -p "$(dirname "$prompt_file")"

    local task_line=""
    if [[ -n "$task_id" ]]; then
        task_line="Target task_id: ${task_id}"
    fi

    cat >"$prompt_file" <<EOF2
Run Tanren phase \`${phase}\` for spec \`${SPEC_ID}\`.
Spec folder: \`${SPEC_FOLDER}\`
Database URL: \`${DATABASE_URL}\`
${task_line}

Requirements:
- Use Tanren MCP tools for all structured state changes.
- If MCP is unavailable, use Tanren CLI with canonical globals:
  tanren-cli --database-url "${DATABASE_URL}" methodology --phase "${phase}" --spec-id "${SPEC_ID}" --spec-folder "${SPEC_FOLDER}" <noun> <verb> --params-file '<payload.json>'
- Complete this phase fully and emit \`report_phase_outcome\`.
- If blocked, emit a typed blocked outcome (or investigate escalation path).
- Never hand-edit orchestrator-owned artifacts.
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
    run_shell_command "harness:${phase}" "$cmd"
}

mark_task_guard() {
    local task_id="$1"
    local guard="$2"
    local payload
    payload="$(printf '{"schema_version":"1.0.0","task_id":"%s","guard":"%s"}' "$task_id" "$guard")"
    local cmd
    cmd="tanren-cli --database-url $(printf '%q' "$DATABASE_URL") methodology"
    cmd+=" --methodology-config $(printf '%q' "$CONFIG_PATH")"
    cmd+=" --phase do-task"
    cmd+=" --spec-id $(printf '%q' "$SPEC_ID")"
    cmd+=" --spec-folder $(printf '%q' "$SPEC_FOLDER")"
    cmd+=" task guard --json $(printf '%q' "$payload")"
    run_shell_command "task_guard_${guard}" "$cmd"
}

prompt_checkpoint() {
    local headline="$1"
    local detail="$2"
    finish_silent_line
    printf '\n[phase0] %s\n%s\n\n' "$headline" "$detail"
}

phase_step_verb() {
    case "$1" in
        task_do_task) printf 'implementing' ;;
        task_gate) printf 'gate-checking' ;;
        task_audit) printf 'auditing' ;;
        task_adhere) printf 'adhering' ;;
        spec_pipeline) printf 'spec-validating' ;;
        *) printf 'working' ;;
    esac
}

derive_next_step_fallback() {
    local status_json="$1"
    local tasks_json="$2"
    local next_task_id="$3"

    local from_status
    from_status="$(jq -r '.next_step // empty' <<<"$status_json")"
    if [[ -n "$from_status" ]]; then
        printf '%s' "$from_status"
        return
    fi

    if [[ -z "$next_task_id" ]]; then
        printf 'spec_pipeline'
        return
    fi

    local state
    state="$(jq -r --arg tid "$next_task_id" '.tasks[] | select(.id == $tid) | .status.state' <<<"$tasks_json")"
    case "$state" in
        pending|in_progress)
            printf 'task_do_task'
            ;;
        implemented)
            local gate_checked audited adherent
            gate_checked="$(jq -r --arg tid "$next_task_id" '.tasks[] | select(.id == $tid) | .status.guards.gate_checked // false' <<<"$tasks_json")"
            audited="$(jq -r --arg tid "$next_task_id" '.tasks[] | select(.id == $tid) | .status.guards.audited // false' <<<"$tasks_json")"
            adherent="$(jq -r --arg tid "$next_task_id" '.tasks[] | select(.id == $tid) | .status.guards.adherent // false' <<<"$tasks_json")"
            if [[ "$gate_checked" != "true" ]]; then
                printf 'task_gate'
            elif [[ "$audited" != "true" ]]; then
                printf 'task_audit'
            elif [[ "$adherent" != "true" ]]; then
                printf 'task_adhere'
            else
                printf 'spec_pipeline'
            fi
            ;;
        *)
            printf 'spec_pipeline'
            ;;
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
need_cmd uv
need_cmd codex

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
RUN_DEMO_HOOK="$(resolve_hook "run_demo_hook" "run-demo" "$SPEC_HOOK")"
AUDIT_SPEC_HOOK="$(resolve_hook "audit_spec_hook" "audit-spec" "$SPEC_HOOK")"
ADHERE_SPEC_HOOK="$(resolve_hook "adhere_spec_hook" "adhere-spec" "$SPEC_HOOK")"

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
RUN_DEMO_HOOK=${RUN_DEMO_HOOK}
AUDIT_SPEC_HOOK=${AUDIT_SPEC_HOOK}
ADHERE_SPEC_HOOK=${ADHERE_SPEC_HOOK}
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

    next_action="$(jq -r '.next_action' <<<"$status_json")"
    signature="$(jq -c '{next_action,next_task_id,next_step,pending_required_guards,total_tasks,pending_tasks,in_progress_tasks,implemented_tasks,completed_tasks,abandoned_tasks,blockers_active}' <<<"$status_json")"
    if [[ "$signature" == "$last_signature" ]]; then
        stagnant=$((stagnant + 1))
    else
        stagnant=0
    fi
    last_signature="$signature"
    if ((stagnant >= 3)); then
        die "orchestration made no state progress across 3 cycles; inspect ${RUN_DIR}/status-cycle-*.json and resolve manually"
    fi

    case "$next_action" in
        shape_spec_required)
            prompt_checkpoint \
                "Spec Not Found (manual checkpoint: shape-spec)" \
                "Spec ${SPEC_ID} has no methodology state yet. Use your harness to run shape-spec, then re-run this orchestrator.

Suggested harness command:
  ${HARNESS_CMD} '/shape-spec for spec ${SPEC_ID} in ${SPEC_FOLDER}'

CLI fallback for typed mutations:
  tanren-cli --database-url ${DATABASE_URL} methodology --phase shape-spec --spec-id ${SPEC_ID} --spec-folder ${SPEC_FOLDER} <noun> <verb> --params-file \"<payload.json>\""
            exit 20
            ;;
        resolve_blockers_required)
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
        run_loop)
            next_task_id="$(jq -r '.next_task_id // empty' <<<"$status_json")"
            next_step="$(jq -r '.next_step // empty' <<<"$status_json")"
            next_step_reason="$(jq -r '.next_step_reason // empty' <<<"$status_json")"

            if [[ -n "$next_task_id" ]]; then
                tasks_json="$(list_tasks_json)"
                next_step="$(derive_next_step_fallback "$status_json" "$tasks_json" "$next_task_id")"

                task_total="$(jq -r '.total_tasks' <<<"$status_json")"
                task_index="$(jq -r --arg tid "$next_task_id" '.tasks | to_entries[] | select(.value.id == $tid) | (.key + 1)' <<<"$tasks_json")"
                task_title="$(jq -r --arg tid "$next_task_id" '.tasks[] | select(.id == $tid) | .title' <<<"$tasks_json")"
                task_description="$(jq -r --arg tid "$next_task_id" '.tasks[] | select(.id == $tid) | .description // empty' <<<"$tasks_json")"
                task_deliverable="$(jq -r --arg tid "$next_task_id" '.tasks[] | select(.id == $tid) | ((.acceptance_criteria[0].measurable // .acceptance_criteria[0].description) // empty)' <<<"$tasks_json")"
                phase_verb="$(phase_step_verb "$next_step")"

                if [[ "$OUTPUT_MODE" == "silent" ]]; then
                    print_progress "task ${task_index}/${task_total} - ${phase_verb}"
                elif [[ "$OUTPUT_MODE" == "quiet" ]]; then
                    quiet_task_summary "$CYCLE" "$task_index" "$task_total" "$phase_verb" "$next_task_id" "$task_title" "$task_description" "$task_deliverable" "$next_step_reason"
                else
                    print_line "cycle ${CYCLE}: task ${task_index}/${task_total} step=${next_step} task_id=${next_task_id}"
                    if [[ -n "$next_step_reason" ]]; then
                        print_line "routing: ${next_step_reason}"
                    fi
                fi

                case "$next_step" in
                    task_do_task)
                        run_harness_phase "do-task" "$next_task_id"
                        ;;
                    task_gate)
                        run_hook "task_verification_hook" "$TASK_HOOK"
                        mark_task_guard "$next_task_id" "gate_checked"
                        ;;
                    task_audit)
                        run_harness_phase "audit-task" "$next_task_id"
                        run_hook "audit_task_hook" "$AUDIT_TASK_HOOK"
                        ;;
                    task_adhere)
                        run_harness_phase "adhere-task" "$next_task_id"
                        run_hook "adhere_task_hook" "$ADHERE_TASK_HOOK"
                        ;;
                    spec_pipeline)
                        run_hook "spec_verification_hook" "$SPEC_HOOK"
                        run_harness_phase "run-demo"
                        run_hook "run_demo_hook" "$RUN_DEMO_HOOK"
                        run_harness_phase "audit-spec"
                        run_hook "audit_spec_hook" "$AUDIT_SPEC_HOOK"
                        run_harness_phase "adhere-spec"
                        run_hook "adhere_spec_hook" "$ADHERE_SPEC_HOOK"
                        ;;
                    *)
                        die "unknown next_step from spec status: ${next_step}"
                        ;;
                esac
            else
                next_step="$(derive_next_step_fallback "$status_json" '{"tasks":[]}' '')"
                phase_verb="$(phase_step_verb "$next_step")"
                print_progress "spec - ${phase_verb}"
                if [[ "$OUTPUT_MODE" == "verbose" ]]; then
                    print_line "cycle ${CYCLE}: spec-level pipeline"
                fi
                run_hook "spec_verification_hook" "$SPEC_HOOK"
                run_harness_phase "run-demo"
                run_hook "run_demo_hook" "$RUN_DEMO_HOOK"
                run_harness_phase "audit-spec"
                run_hook "audit_spec_hook" "$AUDIT_SPEC_HOOK"
                run_harness_phase "adhere-spec"
                run_hook "adhere_spec_hook" "$ADHERE_SPEC_HOOK"
            fi
            if [[ "$DRY_RUN" == "1" ]]; then
                finish_silent_line
                print_line "dry-run completed one simulated autonomous cycle; exiting without persistence"
                exit 0
            fi
            ;;
        *)
            die "unknown next_action from spec status: ${next_action}"
            ;;
    esac
done

die "max cycles (${MAX_CYCLES}) reached without terminal checkpoint"

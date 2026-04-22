#!/usr/bin/env bash
# Phase 0 orchestration driver (CLI entrypoint, Codex-first harness).
#
# Flow policy:
# - interactive checkpoints: shape-spec, resolve-blockers, walk-spec
# - autonomous loop: do-task -> task gates -> spec gates/checks
# - resume source of truth: tanren-cli methodology spec status

set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/orchestration/phase0.sh --spec-id <uuid> [options]

Options:
  --spec-id <uuid>          Required spec id.
  --spec-folder <path>      Spec folder path (default: <spec_root>/<spec-id> from tanren.yml).
  --database-url <url>      Tanren DB URL (default: sqlite:tanren.db).
  --config <path>           tanren.yml path (default: tanren.yml).
  --harness-model <model>   Optional harness model override.
  --max-cycles <n>          Max autonomous cycles before fail (default: 64).
  --dry-run                 Print intended actions without executing harness/hooks.
  -h, --help                Show help.
EOF
}

log() {
    printf '[phase0] %s\n' "$*"
}

die() {
    printf '[phase0] ERROR: %s\n' "$*" >&2
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
        in_methodology && $0 !~ /^  / { in_methodology=0; in_variables=0 }
        in_methodology && /^  variables:[[:space:]]*$/ { in_variables=1; next }
        in_variables && $0 !~ /^    / { in_variables=0 }
        in_variables {
            pattern = "^[[:space:]]{4}" key ":[[:space:]]*(.*)$"
            if (match($0, pattern, m)) {
                print m[1]
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
        in_environment && $0 !~ /^  / { in_environment=0; in_default=0; in_hooks=0 }
        in_environment && /^  default:[[:space:]]*$/ { in_default=1; next }
        in_default && $0 !~ /^    / { in_default=0; in_hooks=0 }
        in_default && /^    verification_hooks:[[:space:]]*$/ { in_hooks=1; next }
        in_hooks && $0 !~ /^      / { in_hooks=0 }
        in_hooks {
            pattern = "^[[:space:]]{6}" phase_key ":[[:space:]]*(.*)$"
            if (match($0, pattern, m)) {
                print m[1]
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
        in_methodology && $0 !~ /^  / { in_methodology=0; in_mcp=0; in_security=0 }
        in_methodology && /^  mcp:[[:space:]]*$/ { in_mcp=1; next }
        in_mcp && $0 !~ /^    / { in_mcp=0; in_security=0 }
        in_mcp && /^    security:[[:space:]]*$/ { in_security=1; next }
        in_security && $0 !~ /^      / { in_security=0 }
        in_security {
            pattern = "^[[:space:]]{6}" key ":[[:space:]]*(.*)$"
            if (match($0, pattern, m)) {
                print m[1]
                exit
            }
        }
    ' "$config_path"
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

run_shell_command() {
    local label="$1"
    local command="$2"
    if [[ "$DRY_RUN" == "1" ]]; then
        log "[dry-run] ${label}: ${command}"
        return 0
    fi
    log "${label}: ${command}"
    bash -lc "$command"
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

    cat >"$prompt_file" <<EOF
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
EOF

    if [[ "$DRY_RUN" == "1" ]]; then
        log "[dry-run] harness phase ${phase} (prompt: ${prompt_file})"
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

prompt_checkpoint() {
    local headline="$1"
    local detail="$2"
    printf '\n[phase0] %s\n%s\n\n' "$headline" "$detail"
}

SPEC_ID=""
SPEC_FOLDER=""
DATABASE_URL="sqlite:tanren.db"
CONFIG_PATH="tanren.yml"
HARNESS_CMD="codex exec"
HARNESS_MODEL="${TANREN_PHASE0_HARNESS_MODEL:-}"
STATUS_PHASE="do-task"
MAX_CYCLES=64
DRY_RUN=0

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

cat >"${RUN_DIR}/resolved-config.env" <<EOF
SPEC_ID=${SPEC_ID}
SPEC_FOLDER=${SPEC_FOLDER}
DATABASE_URL=${DATABASE_URL}
CONFIG_PATH=${CONFIG_PATH}
HARNESS_CMD=${HARNESS_CMD}
HARNESS_MODEL=${HARNESS_MODEL}
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
EOF

log "spec_id=${SPEC_ID}"
log "spec_folder=${SPEC_FOLDER}"
log "harness=${HARNESS_CMD}${HARNESS_MODEL:+ (model=${HARNESS_MODEL})}"
log "task_hook=${TASK_HOOK}"
log "spec_hook=${SPEC_HOOK}"
log "run_dir=${RUN_DIR}"

last_signature=""
stagnant=0

for ((CYCLE = 1; CYCLE <= MAX_CYCLES; CYCLE++)); do
    log "cycle ${CYCLE}: querying spec status"
    status_json="$(spec_status_json)"
    printf '%s\n' "$status_json" >"${RUN_DIR}/last-status.json"
    printf '%s\n' "$status_json" >"${RUN_DIR}/status-cycle-${CYCLE}.json"

    next_action="$(jq -r '.next_action' <<<"$status_json")"
    signature="$(jq -c '{next_action,next_task_id,total_tasks,pending_tasks,in_progress_tasks,implemented_tasks,completed_tasks,abandoned_tasks,blockers_active}' <<<"$status_json")"
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
            log "spec ${SPEC_ID} already completed walk-spec; nothing else to run"
            exit 0
            ;;
        run_loop)
            next_task_id="$(jq -r '.next_task_id // empty' <<<"$status_json")"
            if [[ -n "$next_task_id" ]]; then
                log "cycle ${CYCLE}: task pipeline for task_id=${next_task_id}"
                run_harness_phase "do-task" "$next_task_id"
                run_hook "task_verification_hook" "$TASK_HOOK"
                run_harness_phase "audit-task" "$next_task_id"
                run_hook "audit_task_hook" "$AUDIT_TASK_HOOK"
                run_harness_phase "adhere-task" "$next_task_id"
                run_hook "adhere_task_hook" "$ADHERE_TASK_HOOK"
            else
                log "cycle ${CYCLE}: spec-level pipeline"
                run_hook "spec_verification_hook" "$SPEC_HOOK"
                run_harness_phase "run-demo"
                run_hook "run_demo_hook" "$RUN_DEMO_HOOK"
                run_harness_phase "audit-spec"
                run_hook "audit_spec_hook" "$AUDIT_SPEC_HOOK"
                run_harness_phase "adhere-spec"
                run_hook "adhere_spec_hook" "$ADHERE_SPEC_HOOK"
            fi
            ;;
        *)
            die "unknown next_action from spec status: ${next_action}"
            ;;
    esac
done

die "max cycles (${MAX_CYCLES}) reached without terminal checkpoint"

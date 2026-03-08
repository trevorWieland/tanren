#!/usr/bin/env bash
#
# orchestrate.sh — Automated spec implementation loop
#
# Sequences: do-task → task gate → audit-task → (repeat) → spec gate → run-demo → audit-spec
# The intelligence lives in the agents. This script only does sequencing, gating, and routing.
#
# Usage:
#   ./orchestrate.sh <spec-folder>
#   ./orchestrate.sh <spec-folder> --config orchestrate.conf
#
# The spec folder must contain spec.md and plan.md (output of shape-spec).
#
# Configuration (via env vars, config file, or CLI flags):
#
#   Per-command CLI and model (implementation vs audit use different tools):
#     ORCH_CLI           - Fallback CLI for implementation commands (default: "claude -p --dangerously-skip-permissions")
#     ORCH_DO_CLI        - CLI for do-task (default: ORCH_CLI)
#     ORCH_AUDIT_CLI     - CLI for audit-task (default: "codex exec --yolo")
#     ORCH_DEMO_CLI      - CLI for run-demo (default: ORCH_CLI)
#     ORCH_SPEC_CLI      - CLI for audit-spec (default: "codex exec --yolo")
#     ORCH_DO_MODEL      - Model for do-task (default: "sonnet")
#     ORCH_AUDIT_MODEL   - Model for audit-task (default: "gpt-5.3-codex")
#     ORCH_DEMO_MODEL    - Model for run-demo (default: "sonnet")
#     ORCH_SPEC_MODEL    - Model for audit-spec (default: "gpt-5.3-codex")
#
#   Gates and limits:
#     ORCH_TASK_GATE     - Task-level verification command (default: "make check")
#     ORCH_SPEC_GATE     - Spec-level verification command (default: "make all")
#     ORCH_MAX_CYCLES    - Safety limit on full cycles (default: 10, staleness detector is primary)
#     ORCH_COMMANDS_DIR  - Path to command markdown files (default: ".claude/commands/tanren")
#     ORCH_AGENT_TIMEOUT - Per-agent invocation timeout in seconds (default: 1800)
#     ORCH_MAX_TASK_RETRIES - Max attempts for a single task before aborting (default: 3)
#     ORCH_STALE_LIMIT   - Cycles with unchanged plan.md before aborting (default: 3)
#     ORCH_MAX_DEMO_RETRIES - Retries when run-demo/audit-spec fail without adding tasks (default: 3)

set -euo pipefail

# --- Colors and formatting ---

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
DIM='\033[2m'
BOLD='\033[1m'
NC='\033[0m'

# tty-safe printf — all UI output goes to /dev/tty so it's never captured by $()
tput() { printf "$@" > /dev/tty 2>/dev/null; }

# Only used for fatal errors that should stop the script
fail() { echo -e "${RED}[orch]${NC} $*"; }

# Play the victory fanfare (best-effort, never blocks on failure)
fanfare() {
    local script_dir
    script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    local wav="$script_dir/fanfare.wav"
    if [[ ! -f "$wav" ]]; then return; fi

    if command -v paplay &>/dev/null; then
        paplay "$wav" &>/dev/null &
    elif command -v aplay &>/dev/null; then
        aplay -q "$wav" &>/dev/null &
    elif command -v mpv &>/dev/null; then
        mpv --no-video --really-quiet "$wav" &>/dev/null &
    elif command -v ffplay &>/dev/null; then
        ffplay -nodisp -autoexit -loglevel quiet "$wav" &>/dev/null &
    elif command -v powershell.exe &>/dev/null; then
        local win_path
        win_path=$(wslpath -w "$wav" 2>/dev/null) || return
        powershell.exe -c "(New-Object Media.SoundPlayer '$win_path').PlaySync()" &>/dev/null &
    fi
}

# --- Progress display ---

SCRIPT_START=$(date +%s)
TIMER_PID=""
AGENT_PID=""
AGENT_PGID=""
AGENT_OUTPUT_FILE=""

SPINNER=("⠋" "⠙" "⠹" "⠸" "⠼" "⠴" "⠦" "⠧" "⠇" "⠏")

cleanup_timer() {
    if [[ -n "$TIMER_PID" ]]; then
        kill "$TIMER_PID" 2>/dev/null || true
        wait "$TIMER_PID" 2>/dev/null || true
        TIMER_PID=""
    fi
}

cleanup_agent() {
    if [[ -n "$AGENT_PGID" ]]; then
        # Kill the entire process group (agent + all descendants) by negated PGID.
        # This is the only reliable way to kill timeout → claude → bash → make → ...
        # chains, because killing individual PIDs leaves orphaned grandchildren.
        kill -TERM -"$AGENT_PGID" 2>/dev/null || true
        # Brief grace period for clean shutdown
        local i=0
        while kill -0 "$AGENT_PID" 2>/dev/null && [[ $i -lt 5 ]]; do
            sleep 1
            i=$((i + 1))
        done
        # Force kill the entire group if anything survived
        kill -KILL -"$AGENT_PGID" 2>/dev/null || true
        wait "$AGENT_PID" 2>/dev/null || true
        AGENT_PID=""
        AGENT_PGID=""
    elif [[ -n "$AGENT_PID" ]]; then
        # Fallback if PGID wasn't captured
        kill -KILL "$AGENT_PID" 2>/dev/null || true
        wait "$AGENT_PID" 2>/dev/null || true
        AGENT_PID=""
    fi
}

cleanup_all() {
    cleanup_timer
    cleanup_agent
    rm -f "${STATUS_FILE:-}" "${SPEC_BACKUP:-}" "${AGENT_OUTPUT_FILE:-}" "${GATE_OUTPUT_FILE:-}" "${PID_FILE:-}" 2>/dev/null || true
    # Clear the spinner line on interrupt so the terminal is clean
    printf "\r\033[K" > /dev/tty 2>/dev/null || true
}
# HUP: terminal closed. Must be trapped or backgrounded children become orphans.
trap cleanup_all EXIT INT TERM HUP

_timer_loop() {
    local label="$1" model="$2" start_ts="$3"
    local i=0
    while true; do
        local now elapsed mins secs frame
        now=$(date +%s)
        elapsed=$((now - start_ts))
        mins=$((elapsed / 60))
        secs=$((elapsed % 60))
        frame="${SPINNER[$((i % ${#SPINNER[@]}))]}"
        if [[ -n "$model" ]]; then
            printf "\r  ${BLUE}%s${NC} %-14s │ %-16s │ %dm %02ds" \
                "$frame" "$label" "$model" "$mins" "$secs" > /dev/tty 2>/dev/null
        else
            printf "\r  ${BLUE}%s${NC} %-14s │ %dm %02ds" \
                "$frame" "$label" "$mins" "$secs" > /dev/tty 2>/dev/null
        fi
        i=$((i + 1))
        sleep 1
    done
}

begin_phase() {
    local label="$1" model="${2:-}"
    _PHASE_START=$(date +%s)
    _PHASE_LABEL="$label"
    _timer_loop "$label" "$model" "$_PHASE_START" > /dev/null 2>&1 &
    TIMER_PID=$!
}

end_phase() {
    local status="${1:-ok}" annotation="${2:-}"
    cleanup_timer

    local elapsed mins secs
    elapsed=$(( $(date +%s) - _PHASE_START ))
    mins=$((elapsed / 60))
    secs=$((elapsed % 60))

    local suffix=""
    if [[ -n "$annotation" ]]; then
        suffix=" · ${annotation}"
    fi

    printf "\r\033[K" > /dev/tty 2>/dev/null
    if [[ "$status" == "ok" ]]; then
        printf "  ${GREEN}✓${NC} %-14s │ %dm %02ds%s\n" "$_PHASE_LABEL" "$mins" "$secs" "$suffix" > /dev/tty 2>/dev/null
    else
        printf "  ${RED}✗${NC} %-14s │ %dm %02ds%s\n" "$_PHASE_LABEL" "$mins" "$secs" "$suffix" > /dev/tty 2>/dev/null
    fi
}

# --- Configuration ---

SPEC_FOLDER="${1:?Usage: orchestrate.sh <spec-folder>}"
shift

# Defaults — implementation uses Claude, auditing uses Codex (different model = independent review)
_DEFAULT_CLI="${ORCH_CLI:-claude -p --dangerously-skip-permissions}"
DO_CLI="${ORCH_DO_CLI:-$_DEFAULT_CLI}"
DO_MODEL="${ORCH_DO_MODEL:-opus}"
AUDIT_CLI="${ORCH_AUDIT_CLI:-codex exec --yolo}"
AUDIT_MODEL="${ORCH_AUDIT_MODEL:-gpt-5.3-codex}"
DEMO_CLI="${ORCH_DEMO_CLI:-$_DEFAULT_CLI}"
DEMO_MODEL="${ORCH_DEMO_MODEL:-opus}"
SPEC_CLI="${ORCH_SPEC_CLI:-codex exec --yolo}"
SPEC_MODEL="${ORCH_SPEC_MODEL:-gpt-5.3-codex}"
TASK_GATE="${ORCH_TASK_GATE:-make check}"
SPEC_GATE="${ORCH_SPEC_GATE:-make all}"
MAX_CYCLES="${ORCH_MAX_CYCLES:-10}"
COMMANDS_DIR="${ORCH_COMMANDS_DIR:-.claude/commands/tanren}"
AGENT_TIMEOUT="${ORCH_AGENT_TIMEOUT:-1800}"
MAX_TASK_RETRIES="${ORCH_MAX_TASK_RETRIES:-5}"
STALE_LIMIT="${ORCH_STALE_LIMIT:-3}"
MAX_DEMO_RETRIES="${ORCH_MAX_DEMO_RETRIES:-3}"

# Load config file if provided
while [[ $# -gt 0 ]]; do
    case "$1" in
        --config)
            if [[ -f "$2" ]]; then
                # shellcheck source=/dev/null
                source "$2"
            else
                fail "Config file not found: $2"
                exit 1
            fi
            shift 2
            ;;
        *)
            fail "Unknown argument: $1"
            exit 1
            ;;
    esac
done

# Validate spec folder
if [[ ! -f "$SPEC_FOLDER/spec.md" ]]; then
    fail "spec.md not found in $SPEC_FOLDER"
    exit 1
fi
if [[ ! -f "$SPEC_FOLDER/plan.md" ]]; then
    fail "plan.md not found in $SPEC_FOLDER"
    exit 1
fi

# --- Concurrency lock ---
# Prevents two orchestrators from running on the same spec simultaneously.
# Uses flock on fd 9 — released automatically when the script exits.

LOCK_FILE="$SPEC_FOLDER/.orchestrate.lock"
exec 9>"$LOCK_FILE"
if ! flock -n 9; then
    fail "Another orchestrator is already running on $SPEC_FOLDER"
    exit 1
fi

# Write PID file so the orchestrator can be found and killed externally.
# The cleanup trap removes it on exit.
PID_FILE="$SPEC_FOLDER/.orchestrate.pid"
echo $$ > "$PID_FILE"

# --- Helper functions ---

next_task_label() {
    local line
    line=$(grep -m1 -P '^\s*- \[ \] Task \d+' "$SPEC_FOLDER/plan.md" 2>/dev/null) || true
    if [[ -n "$line" ]]; then
        echo "$line" | sed 's/^\s*- \[ \] //'
    else
        echo ""
    fi
}

# --- spec.md immutability guard ---
# Snapshot at start, verify after each agent invocation. If an agent modifies
# spec.md, revert it and amend the commit — self-heal rather than abort.

SPEC_MD5=$(md5sum "$SPEC_FOLDER/spec.md" | cut -d' ' -f1)
SPEC_BACKUP=$(mktemp)
cp "$SPEC_FOLDER/spec.md" "$SPEC_BACKUP"

guard_spec_md() {
    local current_md5
    current_md5=$(md5sum "$SPEC_FOLDER/spec.md" | cut -d' ' -f1)
    if [[ "$current_md5" != "$SPEC_MD5" ]]; then
        tput "  ${YELLOW}⚠ Agent modified spec.md — reverting (immutability contract)${NC}\n"
        cp "$SPEC_BACKUP" "$SPEC_FOLDER/spec.md"
        git add "$SPEC_FOLDER/spec.md"
        git commit --amend --no-edit 2>/dev/null || true
    fi
}

# --- File-based agent status ---
# Agents are instructed to write their status signal to this file in addition
# to printing it to stdout. File-based extraction is deterministic; stdout grep
# is kept as a fallback for backwards compatibility.

STATUS_FILE="$SPEC_FOLDER/.agent-status"

clear_status_file() {
    rm -f "$STATUS_FILE"
}

extract_signal() {
    local output="$1"
    local prefix="$2"

    # Primary: read from status file (deterministic, not affected by output format)
    if [[ -f "$STATUS_FILE" ]]; then
        local file_signal
        file_signal=$(grep -oP "${prefix}: \K\w[\w-]*" "$STATUS_FILE" 2>/dev/null | tail -1) || true
        if [[ -n "$file_signal" ]]; then
            echo "$file_signal"
            return
        fi
    fi

    # Fallback: grep stdout (fragile but backwards-compatible)
    echo "$output" | grep -oP "${prefix}: \K\w[\w-]*" | tail -1 || true
}

# invoke_agent — runs an agent command and stores output in AGENT_OUTPUT.
#
# Uses background process + wait instead of command substitution so that
# SIGINT (Ctrl+C) can interrupt the agent cleanly. The agent PID is tracked
# in AGENT_PID so the cleanup trap can kill it on interrupt.
#
# Callers: use AGENT_OUTPUT after calling, not $(invoke_agent ...).
AGENT_OUTPUT=""

invoke_agent() {
    local command_name="$1"
    local cli="$2"
    local model="$3"
    local extra_context="${4:-}"

    local command_file="$COMMANDS_DIR/${command_name}.md"
    if [[ ! -f "$command_file" ]]; then
        fail "Command file not found: $command_file"
        AGENT_OUTPUT=""
        return 1
    fi

    # Clear status file before each invocation
    clear_status_file

    local prompt
    prompt="$(cat "$command_file")"
    prompt="$prompt

---

Spec folder: $SPEC_FOLDER

$extra_context

---

IMPORTANT: Before exiting, write ONLY your exit signal line to this file: $SPEC_FOLDER/.agent-status
For example, write exactly one line like \`${command_name}-status: complete\` to that file using your file-writing tool."

    local cmd="$cli"
    if [[ -n "$model" ]]; then
        cmd="$cmd --model $model"
    fi

    # Use file-based output capture so we don't need command substitution.
    # This keeps the agent in a background process where `wait` is interruptible.
    #
    # setsid gives the agent its own process group so we can kill the entire
    # tree (timeout → claude → bash → make → pytest → ...) with a single
    # `kill -PGID`. Without this, closing the terminal orphans grandchildren.
    AGENT_OUTPUT_FILE=$(mktemp)
    local exit_code

    if [[ "$cli" == *codex* ]]; then
        local last_msg_file
        last_msg_file=$(mktemp)
        setsid bash -c "eval \"timeout $AGENT_TIMEOUT $cmd -o '$last_msg_file'\"" <<< "$prompt" > /dev/null 2>&1 &
        AGENT_PID=$!
        AGENT_PGID=$AGENT_PID  # setsid makes PID == PGID
        wait "$AGENT_PID" && exit_code=0 || exit_code=$?
        AGENT_PID=""
        AGENT_PGID=""

        if [[ $exit_code -eq 124 ]]; then
            fail "Agent timed out after ${AGENT_TIMEOUT}s: $command_name"
            rm -f "$last_msg_file"
            AGENT_OUTPUT=""
            return
        fi
        if [[ -s "$last_msg_file" ]]; then
            AGENT_OUTPUT=$(<"$last_msg_file")
        else
            AGENT_OUTPUT=""
        fi
        rm -f "$last_msg_file"
    else
        setsid bash -c "eval \"timeout $AGENT_TIMEOUT $cmd\"" <<< "$prompt" > "$AGENT_OUTPUT_FILE" 2>&1 &
        AGENT_PID=$!
        AGENT_PGID=$AGENT_PID  # setsid makes PID == PGID
        wait "$AGENT_PID" && exit_code=0 || exit_code=$?
        AGENT_PID=""
        AGENT_PGID=""

        if [[ $exit_code -eq 124 ]]; then
            fail "Agent timed out after ${AGENT_TIMEOUT}s: $command_name"
            AGENT_OUTPUT=""
            rm -f "$AGENT_OUTPUT_FILE"
            AGENT_OUTPUT_FILE=""
            return
        fi
        if [[ -s "$AGENT_OUTPUT_FILE" ]]; then
            AGENT_OUTPUT=$(<"$AGENT_OUTPUT_FILE")
        else
            AGENT_OUTPUT=""
        fi
    fi

    rm -f "$AGENT_OUTPUT_FILE"
    AGENT_OUTPUT_FILE=""
}

# Run a gate command silently with spinner. Output stored in LAST_GATE_OUTPUT.
# Uses setsid so the gate's entire process tree (make → pytest → ...) can be
# killed as a group on interrupt — same pattern as invoke_agent.
LAST_GATE_OUTPUT=""
GATE_OUTPUT_FILE=""
run_gate() {
    local label="$1"
    local gate_cmd="$2"

    begin_phase "$label"

    GATE_OUTPUT_FILE=$(mktemp)
    local exit_code

    setsid bash -c "eval \"$gate_cmd\"" > "$GATE_OUTPUT_FILE" 2>&1 &
    AGENT_PID=$!
    AGENT_PGID=$AGENT_PID
    wait "$AGENT_PID" && exit_code=0 || exit_code=$?
    AGENT_PID=""
    AGENT_PGID=""

    if [[ -s "$GATE_OUTPUT_FILE" ]]; then
        LAST_GATE_OUTPUT=$(<"$GATE_OUTPUT_FILE")
    else
        LAST_GATE_OUTPUT=""
    fi
    rm -f "$GATE_OUTPUT_FILE"
    GATE_OUTPUT_FILE=""

    if [[ $exit_code -eq 0 ]]; then
        end_phase "ok"
    else
        end_phase "fail"
    fi

    return $exit_code
}

count_unchecked() {
    # Count only Task lines — fix items are sub-items of their parent task,
    # not independently workable. Must match the same pattern as next_task_label
    # to avoid mismatch bugs (e.g. orphaned fix items keeping the loop alive
    # when no actionable task exists).
    local count
    count=$(grep -cP '^\s*- \[ \] Task \d+' "$SPEC_FOLDER/plan.md" 2>/dev/null) || count=0
    echo "$count"
}

plan_snapshot() {
    md5sum "$SPEC_FOLDER/plan.md" 2>/dev/null | cut -d' ' -f1 || true
}

audit_status() {
    head -1 "$SPEC_FOLDER/audit.md" 2>/dev/null | grep -oP 'status: \K\w+' || echo "unknown"
}

MAX_GATE_RETRIES=3

# --- Main loop ---

tput "\n${BOLD}═══ Orchestrator ═══${NC}\n\n"
tput "  ${DIM}Spec:${NC}    %s\n" "$SPEC_FOLDER"
tput "  ${DIM}Impl:${NC}    %s ${DIM}(%s)${NC}\n" "$DO_MODEL" "${DO_CLI%% *}"
tput "  ${DIM}Audit:${NC}   %s ${DIM}(%s)${NC}\n" "$AUDIT_MODEL" "${AUDIT_CLI%% *}"
tput "  ${DIM}Gates:${NC}   %s │ %s\n" "$TASK_GATE" "$SPEC_GATE"
tput "  ${DIM}Limits:${NC}  %ss timeout │ %d task retries │ %d stale cycles │ %d demo retries\n" "$AGENT_TIMEOUT" "$MAX_TASK_RETRIES" "$STALE_LIMIT" "$MAX_DEMO_RETRIES"

cycle=0
prev_snapshot=""
stale_count=0

while true; do
    cycle=$((cycle + 1))

    if [[ $cycle -gt $MAX_CYCLES ]]; then
        fail "Safety limit reached ($MAX_CYCLES cycles)."
        exit 1
    fi

    # Staleness detection — tracks plan.md changes across cycles.
    # Skip the check on cycle 1 (no previous snapshot to compare against).
    current_snapshot=$(plan_snapshot)
    if [[ -n "$prev_snapshot" && "$current_snapshot" == "$prev_snapshot" ]]; then
        stale_count=$((stale_count + 1))
        if [[ $stale_count -ge $STALE_LIMIT ]]; then
            fail "Stale — plan.md unchanged for $stale_count cycles. See signposts.md / audit-log.md."
            exit 1
        fi
        tput "  ${YELLOW}⚠ plan.md unchanged (%d/%d)${NC}\n" "$stale_count" "$STALE_LIMIT"
    else
        stale_count=0
    fi
    prev_snapshot="$current_snapshot"

    unchecked_at_cycle_start=$(count_unchecked)
    tput "\n${BOLD}── Cycle %d ──${NC} %d tasks remaining\n" "$cycle" "$unchecked_at_cycle_start"

    # ─── Phase 1: Task Loop ───

    if [[ "$unchecked_at_cycle_start" -eq 0 ]]; then
        tput "  ${DIM}All tasks complete — skipping to verification${NC}\n"
    fi

    task_attempts=0
    prev_task=""

    while true; do
        unchecked=$(count_unchecked)
        if [[ "$unchecked" -eq 0 ]]; then
            break
        fi

        task_label=$(next_task_label)

        # Guard: if no Task line found but count_unchecked > 0, something
        # is out of sync (shouldn't happen now that both use the same
        # pattern, but defense-in-depth).
        if [[ -z "$task_label" ]]; then
            tput "  ${YELLOW}⚠ No actionable task found — advancing to verification${NC}\n"
            break
        fi

        # Inner-loop retry limit: detect when the same task is being
        # retried repeatedly (e.g., do-task ↔ audit-task ping-pong)
        if [[ "$task_label" == "$prev_task" ]]; then
            task_attempts=$((task_attempts + 1))
            if [[ $task_attempts -gt $MAX_TASK_RETRIES ]]; then
                fail "Task stuck after $MAX_TASK_RETRIES attempts: $task_label"
                fail "See signposts.md and audit-log.md for details."
                exit 1
            fi
            tput "  ${YELLOW}⚠ Retrying task (%d/%d)${NC}\n" "$task_attempts" "$MAX_TASK_RETRIES"
        else
            task_attempts=1
            prev_task="$task_label"
        fi

        if [[ -n "$task_label" ]]; then
            tput "\n  ${BOLD}► %s${NC}\n" "$task_label"
        fi

        # do-task
        begin_phase "do-task" "$DO_MODEL"
        invoke_agent "do-task" "$DO_CLI" "$DO_MODEL"
        output="$AGENT_OUTPUT"
        guard_spec_md
        signal=$(extract_signal "$output" "do-task-status")

        case "$signal" in
            complete)
                end_phase "ok" "task completed"
                ;;
            all-done)
                end_phase "ok" "all tasks done"
                break
                ;;
            blocked)
                end_phase "fail" "blocked"
                fail "Human intervention needed. See signposts.md."
                exit 1
                ;;
            error)
                end_phase "fail" "error"
                echo "$output" | tail -20
                exit 1
                ;;
            "")
                end_phase "fail" "no signal detected"
                tput "  ${YELLOW}⚠ do-task did not emit a status signal — continuing (gate will verify)${NC}\n"
                ;;
            *)
                end_phase "ok" "signal: $signal"
                tput "  ${YELLOW}⚠ Unrecognized do-task signal: %s${NC}\n" "$signal"
                ;;
        esac

        # Task gate with retry loop
        gate_attempt=0
        gate_passed=false

        while [[ "$gate_passed" == "false" ]]; do
            gate_attempt=$((gate_attempt + 1))

            if run_gate "make check" "$TASK_GATE"; then
                gate_passed=true
            else
                if [[ $gate_attempt -ge $MAX_GATE_RETRIES ]]; then
                    fail "Task gate failing after $MAX_GATE_RETRIES attempts:"
                    echo "$LAST_GATE_OUTPUT" | tail -30
                    exit 1
                fi

                # Re-invoke do-task with gate errors
                begin_phase "do-task" "$DO_MODEL"
                invoke_agent "do-task" "$DO_CLI" "$DO_MODEL" \
                    "GATE FAILURE — '$TASK_GATE' FAILED. Fix these errors, run the gate yourself, then exit with do-task-status: complete.

\`\`\`
$LAST_GATE_OUTPUT
\`\`\`"
                output="$AGENT_OUTPUT"
                guard_spec_md
                signal=$(extract_signal "$output" "do-task-status")
                end_phase "ok" "gate fix"

                if [[ "$signal" == "blocked" || "$signal" == "error" ]]; then
                    fail "do-task: '$signal' while fixing gate. See signposts.md."
                    exit 1
                fi
            fi
        done

        # audit-task
        begin_phase "audit-task" "$AUDIT_MODEL"
        invoke_agent "audit-task" "$AUDIT_CLI" "$AUDIT_MODEL"
        output="$AGENT_OUTPUT"
        guard_spec_md
        signal=$(extract_signal "$output" "audit-task-status")

        case "$signal" in
            pass)
                end_phase "ok" "passed"
                # Self-heal: if both do-task and audit-task agree the task is
                # done but the checkbox wasn't persisted, check it off now.
                # Uses next_task_label (same function as the loop) for consistency.
                still_next=$(next_task_label)
                if [[ -n "$task_label" && "$still_next" == "$task_label" ]]; then
                    tput "  ${YELLOW}⚠ Task still unchecked after audit pass — checking it off${NC}\n"
                    awk -v lbl="$task_label" '!done && index($0, "- [ ] " lbl) { sub(/- \[ \]/, "- [x]"); done=1 } 1' \
                        "$SPEC_FOLDER/plan.md" > "$SPEC_FOLDER/plan.md.tmp" \
                        && mv "$SPEC_FOLDER/plan.md.tmp" "$SPEC_FOLDER/plan.md"
                    # Also check off any orphaned fix items under this task
                    awk -v lbl="$task_label" '
                        /- \[x\]/ && index($0, lbl) { found=1; print; next }
                        found && /^[[:space:]]+- \[ \] Fix:/ { sub(/- \[ \]/, "- [x]"); print; next }
                        found && /^[[:space:]]*- \[/ { found=0 }
                        found && !/^[[:space:]]/ { found=0 }
                        { print }
                    ' "$SPEC_FOLDER/plan.md" > "$SPEC_FOLDER/plan.md.tmp" \
                        && mv "$SPEC_FOLDER/plan.md.tmp" "$SPEC_FOLDER/plan.md"
                    git add "$SPEC_FOLDER/plan.md"
                    git commit --amend --no-edit 2>/dev/null || git commit -m "Fix: check off $task_label (bookkeeping)" 2>/dev/null || true
                fi
                ;;
            fail)
                end_phase "ok" "issues found — fix items added"
                ;;
            error)
                end_phase "fail" "error"
                echo "$output" | tail -20
                exit 1
                ;;
            "")
                end_phase "fail" "no signal detected"
                tput "  ${YELLOW}⚠ audit-task did not emit a status signal — continuing${NC}\n"
                ;;
            *)
                end_phase "ok" "signal: $signal"
                tput "  ${YELLOW}⚠ Unrecognized audit-task signal: %s${NC}\n" "$signal"
                ;;
        esac
    done

    # ─── Phase 2: Spec Gate ───

    if ! run_gate "make all" "$SPEC_GATE"; then
        tput "  ${YELLOW}↳ spec gate failed, invoking do-task to fix${NC}\n"
        begin_phase "do-task" "$DO_MODEL"
        invoke_agent "do-task" "$DO_CLI" "$DO_MODEL" \
            "The full verification gate ($SPEC_GATE) failed after all tasks were completed. Diagnose and fix.

\`\`\`
$LAST_GATE_OUTPUT
\`\`\`"
        output="$AGENT_OUTPUT"
        guard_spec_md
        end_phase "ok" "gate fix applied"
        continue
    fi

    # ─── Phase 3: Demo ───

    pre_demo_tasks=$(count_unchecked)
    begin_phase "run-demo" "$DEMO_MODEL"
    invoke_agent "run-demo" "$DEMO_CLI" "$DEMO_MODEL"
    output="$AGENT_OUTPUT"
    guard_spec_md
    signal=$(extract_signal "$output" "run-demo-status")

    case "$signal" in
        pass)
            # Warn if the agent skipped steps — a PASS with skipped steps
            # may mean the agent didn't actually exercise the demo.
            skip_count=0
            if [[ -f "$SPEC_FOLDER/demo.md" ]]; then
                # Count SKIP/SKIPPED lines in the last "### Run" block
                skip_count=$(awk '
                    /^### Run [0-9]+/ { block=""; next }
                    { block = block "\n" $0 }
                    END { print block }
                ' "$SPEC_FOLDER/demo.md" | grep -ciP 'SKIP' 2>/dev/null) || skip_count=0
            fi
            if [[ "$skip_count" -gt 0 ]]; then
                end_phase "ok" "passed ($skip_count steps skipped)"
                tput "  ${YELLOW}⚠ Demo PASS but %d step(s) were skipped — verify manually if needed${NC}\n" "$skip_count"
            else
                end_phase "ok" "all steps passed"
            fi
            ;;
        fail)
            # Check whether run-demo actually added new tasks.
            new_unchecked=$(count_unchecked)
            if [[ "$new_unchecked" -gt "$pre_demo_tasks" ]]; then
                end_phase "ok" "failed — $new_unchecked fix tasks added"
                continue
            fi

            # No tasks added — retry with forceful context demanding tasks.
            end_phase "fail" "failed — no tasks added, retrying"
            demo_retry=0
            demo_resolved=false
            while [[ $demo_retry -lt $MAX_DEMO_RETRIES ]]; do
                demo_retry=$((demo_retry + 1))
                tput "  ${YELLOW}↳ run-demo retry %d/%d — demanding fix tasks${NC}\n" "$demo_retry" "$MAX_DEMO_RETRIES"

                begin_phase "run-demo" "$DEMO_MODEL"
                invoke_agent "run-demo" "$DEMO_CLI" "$DEMO_MODEL" \
                    "CRITICAL: You signalled run-demo-status: fail but added NO new tasks to plan.md.
This causes the orchestrator to loop infinitely. You MUST do one of the following:

1. Add concrete fix tasks to plan.md as \`- [ ] Task N: <description>\` (use the next sequential task number).
2. If the demo actually passes on re-examination, signal run-demo-status: pass instead.

Do NOT dismiss failures as 'not a code defect' or 'environmental' without thorough investigation.
If the issue is truly environmental (e.g. wrong URL, missing service), add a task for a workaround,
mock, or graceful degradation so the demo can pass.

Previously unchecked tasks: $pre_demo_tasks. Current unchecked: $(count_unchecked).
You must increase the task count or change your signal to pass."
                output="$AGENT_OUTPUT"
                guard_spec_md
                signal=$(extract_signal "$output" "run-demo-status")

                if [[ "$signal" == "pass" ]]; then
                    end_phase "ok" "passed on retry $demo_retry"
                    demo_resolved=true
                    break
                fi

                new_unchecked=$(count_unchecked)
                if [[ "$new_unchecked" -gt "$pre_demo_tasks" ]]; then
                    end_phase "ok" "retry $demo_retry — $new_unchecked fix tasks added"
                    demo_resolved=true
                    break
                fi

                end_phase "fail" "retry $demo_retry — still no tasks"
            done

            if [[ "$demo_resolved" == "true" && "$signal" == "pass" ]]; then
                # Fall through to audit-spec (don't continue)
                :
            elif [[ "$demo_resolved" == "true" ]]; then
                continue
            else
                # Retries exhausted — let staleness detection catch it next cycle
                tput "  ${YELLOW}⚠ run-demo retries exhausted — deferring to staleness detection${NC}\n"
                continue
            fi
            ;;
        error)
            end_phase "fail" "error"
            echo "$output" | tail -20
            exit 1
            ;;
        "")
            end_phase "fail" "no signal detected"
            tput "  ${YELLOW}⚠ run-demo did not emit a status signal${NC}\n"
            echo "$output" | tail -20
            exit 1
            ;;
        *)
            end_phase "ok" "signal: $signal"
            tput "  ${YELLOW}⚠ Unrecognized run-demo signal: %s${NC}\n" "$signal"
            ;;
    esac

    # ─── Phase 4: Spec Audit ───

    # Snapshot audit.md mtime to detect stale reads from a previous cycle
    audit_md_before=""
    if [[ -f "$SPEC_FOLDER/audit.md" ]]; then
        audit_md_before=$(stat -c %Y "$SPEC_FOLDER/audit.md" 2>/dev/null) || true
    fi

    pre_audit_tasks=$(count_unchecked)
    begin_phase "audit-spec" "$SPEC_MODEL"
    invoke_agent "audit-spec" "$SPEC_CLI" "$SPEC_MODEL"
    output="$AGENT_OUTPUT"
    guard_spec_md

    # Verify audit.md was actually written/updated by this invocation
    audit_md_after=""
    if [[ -f "$SPEC_FOLDER/audit.md" ]]; then
        audit_md_after=$(stat -c %Y "$SPEC_FOLDER/audit.md" 2>/dev/null) || true
    fi

    if [[ -z "$audit_md_after" ]]; then
        end_phase "fail" "audit.md not written"
        fail "audit-spec did not create audit.md"
        echo "$output" | tail -20
        exit 1
    fi

    if [[ -n "$audit_md_before" && "$audit_md_before" == "$audit_md_after" ]]; then
        end_phase "fail" "audit.md not updated (stale)"
        fail "audit-spec did not update audit.md — possible stale result from previous run"
        exit 1
    fi

    status=$(audit_status)

    case "$status" in
        pass)
            end_phase "ok" "PASS"
            ;;
        fail)
            new_unchecked=$(count_unchecked)
            if [[ "$new_unchecked" -gt "$pre_audit_tasks" ]]; then
                end_phase "ok" "FAIL — $new_unchecked fix items added"
                continue
            fi

            # No tasks added — retry with forceful context demanding fix items.
            end_phase "fail" "FAIL — no tasks added, retrying"
            audit_retry=0
            audit_resolved=false
            while [[ $audit_retry -lt $MAX_DEMO_RETRIES ]]; do
                audit_retry=$((audit_retry + 1))
                tput "  ${YELLOW}↳ audit-spec retry %d/%d — demanding fix items${NC}\n" "$audit_retry" "$MAX_DEMO_RETRIES"

                begin_phase "audit-spec" "$SPEC_MODEL"
                invoke_agent "audit-spec" "$SPEC_CLI" "$SPEC_MODEL" \
                    "CRITICAL: You wrote audit.md with status: fail but added NO new unchecked tasks to plan.md.
This causes the orchestrator to loop infinitely. You MUST do one of the following:

1. Uncheck existing tasks that need rework and add \`- [ ] Fix: <description>\` sub-items under them.
2. Add new \`- [ ] Task N: <description>\` entries for issues not covered by existing tasks.
3. If the implementation actually passes on re-examination, update audit.md to \`status: pass\`.

Previously unchecked tasks: $pre_audit_tasks. Current unchecked: $(count_unchecked).
You must increase the task count or change audit.md status to pass."
                output="$AGENT_OUTPUT"
                guard_spec_md

                status=$(audit_status)

                if [[ "$status" == "pass" ]]; then
                    end_phase "ok" "PASS on retry $audit_retry"
                    audit_resolved=true
                    break
                fi

                new_unchecked=$(count_unchecked)
                if [[ "$new_unchecked" -gt "$pre_audit_tasks" ]]; then
                    end_phase "ok" "retry $audit_retry — $new_unchecked fix items added"
                    audit_resolved=true
                    break
                fi

                end_phase "fail" "retry $audit_retry — still no tasks"
            done

            if [[ "$audit_resolved" == "true" && "$status" == "pass" ]]; then
                # Fall through to break (spec passed)
                :
            elif [[ "$audit_resolved" == "true" ]]; then
                continue
            else
                # Retries exhausted — let staleness detection catch it next cycle
                tput "  ${YELLOW}⚠ audit-spec retries exhausted — deferring to staleness detection${NC}\n"
                continue
            fi
            ;;
        *)
            end_phase "fail" "unknown status: $status"
            echo "$output" | tail -20
            exit 1
            ;;
    esac

    break
done

# ─── Done ───

fanfare

# Clean up orchestrator artifacts
rm -f "$STATUS_FILE" 2>/dev/null || true

total_elapsed=$(( $(date +%s) - SCRIPT_START ))
total_mins=$((total_elapsed / 60))
total_secs=$((total_elapsed % 60))

tput "\n${GREEN}${BOLD}═══ Complete ═══${NC} %dm %02ds\n\n" "$total_mins" "$total_secs"
tput "  Next: run ${BOLD}/walk-spec${NC} for interactive demo + PR\n"
tput "  Spec: %s\n\n" "$SPEC_FOLDER"

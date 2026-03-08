#!/usr/bin/env bash
#
# audit-standards.sh — Audit codebase compliance against all standards
#
# Spins up parallel codex agents to audit every standard in tanren/standards/.
# Each agent reads the standard + codebase and writes a structured report.
#
# Usage:
#   ./tanren/scripts/audit-standards.sh
#   ./tanren/scripts/audit-standards.sh --concurrency 5
#   ./tanren/scripts/audit-standards.sh --config audit.conf
#   ./tanren/scripts/audit-standards.sh --standards async-first-design,naming-conventions
#
# Configuration (via env vars, config file, or CLI flags):
#
#   AUDIT_CLI           - CLI command for agents (default: "codex exec")
#   AUDIT_MODEL         - Model for audit agents (default: "gpt-5.3-codex-spark")
#   AUDIT_CONCURRENCY   - Max parallel agents (default: 3)
#   AUDIT_TIMEOUT       - Per-agent timeout in seconds (default: 900)
#   AUDIT_STANDARDS_DIR - Path to standards directory (default: "tanren/standards")
#   AUDIT_OUTPUT_DIR    - Output directory (default: "tanren/audits/YYYY-MM-DD")

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
fail() { echo -e "${RED}[audit]${NC} $*"; }

# --- Progress display ---

SCRIPT_START=$(date +%s)

SPINNER=("⠋" "⠙" "⠹" "⠸" "⠼" "⠴" "⠦" "⠧" "⠇" "⠏")

# Track all background PIDs for cleanup
declare -a AGENT_PIDS=()
declare -a TIMER_PIDS=()

cleanup_all() {
    # Kill all spinner timers
    for pid in "${TIMER_PIDS[@]}"; do
        kill "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
    done

    # Kill all agent process groups
    for pid in "${AGENT_PIDS[@]}"; do
        kill -TERM -"$pid" 2>/dev/null || true
    done
    # Grace period
    sleep 1
    for pid in "${AGENT_PIDS[@]}"; do
        kill -KILL -"$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
    done

    # Clear spinner line
    printf "\r\033[K" > /dev/tty 2>/dev/null || true

    # Clean up temp files
    rm -f "${RESULTS_FILE:-}" 2>/dev/null || true
}
trap cleanup_all EXIT INT TERM HUP

_timer_loop() {
    local label="$1" start_ts="$2" line_num="$3"
    local i=0
    while true; do
        local now elapsed mins secs frame
        now=$(date +%s)
        elapsed=$((now - start_ts))
        mins=$((elapsed / 60))
        secs=$((elapsed % 60))
        frame="${SPINNER[$((i % ${#SPINNER[@]}))]}"
        # Save cursor, move to line, print, restore cursor
        printf "\033[s\033[%d;0H\r  ${BLUE}%s${NC} %-40s │ %dm %02ds\033[K\033[u" \
            "$line_num" "$frame" "$label" "$mins" "$secs" > /dev/tty 2>/dev/null
        i=$((i + 1))
        sleep 1
    done
}

# --- Configuration ---

CLI="${AUDIT_CLI:-codex exec}"
MODEL="${AUDIT_MODEL:-gpt-5.3-codex-spark}"
CONCURRENCY="${AUDIT_CONCURRENCY:-3}"
TIMEOUT="${AUDIT_TIMEOUT:-900}"
STANDARDS_DIR="${AUDIT_STANDARDS_DIR:-tanren/standards}"
TODAY=$(date +%Y-%m-%d)
OUTPUT_DIR="${AUDIT_OUTPUT_DIR:-tanren/audits/$TODAY}"
FILTER_STANDARDS=""  # comma-separated list of slugs to audit (empty = all)

# Parse CLI args
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
        --concurrency)
            CONCURRENCY="$2"
            shift 2
            ;;
        --model)
            MODEL="$2"
            shift 2
            ;;
        --timeout)
            TIMEOUT="$2"
            shift 2
            ;;
        --output)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        --standards)
            FILTER_STANDARDS="$2"
            shift 2
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        *)
            fail "Unknown argument: $1"
            echo "Usage: audit-standards.sh [--concurrency N] [--model M] [--timeout S] [--output DIR] [--standards slug1,slug2] [--config FILE] [--dry-run]"
            exit 1
            ;;
    esac
done

# Validate standards directory
if [[ ! -f "$STANDARDS_DIR/index.yml" ]]; then
    fail "Standards index not found: $STANDARDS_DIR/index.yml"
    exit 1
fi

# Read the audit report template
TEMPLATE_FILE="tanren/audits/standard-audit-template.md"
if [[ ! -f "$TEMPLATE_FILE" ]]; then
    fail "Audit template not found: $TEMPLATE_FILE"
    exit 1
fi
TEMPLATE=$(<"$TEMPLATE_FILE")

# --- Parse standards from index.yml ---
# Uses awk to parse the YAML without requiring yq

declare -a STANDARD_SLUGS=()
declare -A STANDARD_CATEGORIES=()
declare -A STANDARD_DESCRIPTIONS=()

parse_standards_index() {
    local current_category=""
    local last_accepted=""  # tracks whether last slug was accepted (for description lines)

    while IFS= read -r line; do
        # Skip empty lines and comments
        [[ -z "$line" || "$line" =~ ^[[:space:]]*# ]] && continue

        # Category line: no leading whitespace, ends with ':'
        if [[ "$line" =~ ^([a-z_-]+):$ ]]; then
            current_category="${BASH_REMATCH[1]}"
            last_accepted=""
            continue
        fi

        # Standard slug line: 2-space indent, ends with ':'
        if [[ "$line" =~ ^[[:space:]]{2}([a-z0-9_-]+):$ ]]; then
            local slug="${BASH_REMATCH[1]}"
            last_accepted=""

            # Apply filter if set
            if [[ -n "$FILTER_STANDARDS" ]]; then
                if [[ ! ",$FILTER_STANDARDS," == *",$slug,"* ]]; then
                    continue
                fi
            fi

            STANDARD_SLUGS+=("$slug")
            STANDARD_CATEGORIES["$slug"]="$current_category"
            last_accepted="$slug"
            continue
        fi

        # Description line: 4-space indent, starts with 'description:'
        # Only store if the preceding slug was accepted
        if [[ -n "$last_accepted" && "$line" =~ ^[[:space:]]{4}description:[[:space:]]*(.*) ]]; then
            local desc="${BASH_REMATCH[1]}"
            # Strip surrounding quotes if present
            desc="${desc#\"}"
            desc="${desc%\"}"
            STANDARD_DESCRIPTIONS["$last_accepted"]="$desc"
            continue
        fi
    done < "$STANDARDS_DIR/index.yml"
}

parse_standards_index

TOTAL=${#STANDARD_SLUGS[@]}

if [[ $TOTAL -eq 0 ]]; then
    fail "No standards found to audit"
    exit 1
fi

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Results tracking file
RESULTS_FILE=$(mktemp)

# --- Header ---

tput "\n${BOLD}═══ Standards Audit ═══${NC}\n\n"
tput "  ${DIM}Standards:${NC}   %d in %s\n" "$TOTAL" "$STANDARDS_DIR"
tput "  ${DIM}Output:${NC}     %s\n" "$OUTPUT_DIR"
tput "  ${DIM}Model:${NC}      %s ${DIM}(%s)${NC}\n" "$MODEL" "${CLI%% *}"
tput "  ${DIM}Concurrency:${NC} %d agents │ %ss timeout\n" "$CONCURRENCY" "$TIMEOUT"

if [[ -n "$FILTER_STANDARDS" ]]; then
    tput "  ${DIM}Filter:${NC}     %s\n" "$FILTER_STANDARDS"
fi

if [[ "${DRY_RUN:-}" == "true" ]]; then
    tput "\n  ${YELLOW}DRY RUN — showing what would be audited:${NC}\n\n"
    for slug in "${STANDARD_SLUGS[@]}"; do
        tput "  %-40s │ %s\n" "$slug" "${STANDARD_CATEGORIES[$slug]}"
    done
    tput "\n"
    exit 0
fi

tput "\n"

# --- Run audits ---

audit_one() {
    local slug="$1"
    local category="${STANDARD_CATEGORIES[$slug]}"
    local description="${STANDARD_DESCRIPTIONS[$slug]:-}"
    local standard_file="$STANDARDS_DIR/$category/$slug.md"
    local output_file="$OUTPUT_DIR/$slug.md"

    # Verify standard file exists
    if [[ ! -f "$standard_file" ]]; then
        echo "SKIP $slug (file not found: $standard_file)" >> "$RESULTS_FILE"
        return
    fi

    local standard_content
    standard_content=$(<"$standard_file")

    # Build the audit prompt
    local prompt
    prompt="You are auditing a codebase for compliance with a specific coding standard.

## Standard: $slug
**Category:** $category
**Description:** $description

### Standard Content:

$standard_content

### Report Template:

$TEMPLATE

## Instructions:

1. Search the codebase for all files relevant to this standard.
2. Check each relevant file for compliance with the standard.
3. Record violations with exact file:line references, evidence (code snippets), and severity.
4. Score compliance 0-100 based on: coverage (% of relevant code that complies), severity of violations, and practical risk.
5. Write your report to: $output_file

**Important:**
- Only report real violations with concrete evidence. Do not speculate.
- Replace all {placeholder} fields in the template with actual values.
- The score should reflect reality: 100 = full compliance, 0 = completely ignored.
- Set importance based on how critical this standard is to project health:
  - Critical: violations cause bugs, security issues, or architectural rot
  - High: violations cause maintenance burden or inconsistency
  - Medium: violations are suboptimal but functional
  - Low: nice-to-have, stylistic
- Set status to 'clean' if score >= 90 and no Critical/High violations, otherwise 'violations-found'.
- Write the report file when done. That is your only output."

    # Build command
    local cmd="$CLI"
    if [[ -n "$MODEL" ]]; then
        cmd="$cmd --model $MODEL"
    fi

    local last_msg_file
    last_msg_file=$(mktemp)

    local exit_code
    setsid bash -c "eval \"timeout $TIMEOUT $cmd -o '$last_msg_file'\"" <<< "$prompt" > /dev/null 2>&1 &
    local agent_pid=$!
    AGENT_PIDS+=("$agent_pid")

    wait "$agent_pid" && exit_code=0 || exit_code=$?

    rm -f "$last_msg_file"

    # Check result
    if [[ $exit_code -eq 124 ]]; then
        echo "TIMEOUT $slug" >> "$RESULTS_FILE"
    elif [[ -f "$output_file" && -s "$output_file" ]]; then
        echo "PASS $slug" >> "$RESULTS_FILE"
    else
        echo "FAIL $slug (exit=$exit_code)" >> "$RESULTS_FILE"
    fi
}

# Concurrency semaphore using wait -n (bash 4.3+)
active=0

for slug in "${STANDARD_SLUGS[@]}"; do
    # Wait for a slot if at concurrency limit
    while [[ $active -ge $CONCURRENCY ]]; do
        wait -n 2>/dev/null || true
        active=$((active - 1))
    done

    category="${STANDARD_CATEGORIES[$slug]}"
    tput "  ${BLUE}⠋${NC} Starting: %-30s │ %s\n" "$slug" "$category"

    audit_one "$slug" &
    active=$((active + 1))
done

# Wait for all remaining agents
while [[ $active -gt 0 ]]; do
    wait -n 2>/dev/null || true
    active=$((active - 1))
done

# --- Results summary ---

pass_count=0
fail_count=0
skip_count=0
timeout_count=0

while IFS= read -r line; do
    case "$line" in
        PASS*)    pass_count=$((pass_count + 1)) ;;
        FAIL*)    fail_count=$((fail_count + 1)) ;;
        SKIP*)    skip_count=$((skip_count + 1)) ;;
        TIMEOUT*) timeout_count=$((timeout_count + 1)) ;;
    esac
done < "$RESULTS_FILE"

total_elapsed=$(( $(date +%s) - SCRIPT_START ))
total_mins=$((total_elapsed / 60))
total_secs=$((total_elapsed % 60))

tput "\n${BOLD}═══ Results ═══${NC}\n\n"

# Print per-standard results
while IFS= read -r line; do
    status="${line%% *}"
    rest="${line#* }"
    case "$status" in
        PASS)    tput "  ${GREEN}✓${NC} %s\n" "$rest" ;;
        FAIL)    tput "  ${RED}✗${NC} %s\n" "$rest" ;;
        SKIP)    tput "  ${YELLOW}○${NC} %s\n" "$rest" ;;
        TIMEOUT) tput "  ${RED}⏱${NC} %s (timed out)\n" "$rest" ;;
    esac
done < "$RESULTS_FILE"

tput "\n  ${DIM}────────────────────────────────────${NC}\n"
tput "  ${GREEN}Pass:${NC}    %d\n" "$pass_count"
tput "  ${RED}Fail:${NC}    %d\n" "$fail_count"
tput "  ${YELLOW}Skip:${NC}    %d\n" "$skip_count"
tput "  ${RED}Timeout:${NC} %d\n" "$timeout_count"
tput "  ${DIM}Total:${NC}   %d │ %dm %02ds\n" "$TOTAL" "$total_mins" "$total_secs"
tput "\n  Reports: %s/\n" "$OUTPUT_DIR"
tput "  Next: run ${BOLD}/triage-audits${NC} to review findings and create issues\n\n"

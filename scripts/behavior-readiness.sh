#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEFAULT_OUTPUT="docs/implementation/readiness.json"
DEFAULT_RUN_ROOT="artifacts/behavior/readiness/runs"
DEFAULT_MODEL="gpt-5.4-mini"
DEFAULT_REASONING_EFFORT="medium"
DEFAULT_JOBS="4"

usage() {
    cat <<'EOF'
Usage: scripts/behavior-readiness.sh [options]

Run read-only parallel Codex static analysis for accepted behavior docs whose
verification_status is below asserted. The script writes durable per-behavior
reports as workers finish, then writes one aggregate JSON artifact.

Options:
  --output PATH        Aggregate report path
                       (default: docs/implementation/readiness.json)
  --jobs N            Parallel Codex workers (default: 4)
  --model MODEL       Codex model (default: gpt-5.4-mini)
  --reasoning-effort LEVEL
                       Codex reasoning effort (default: medium)
  --run-dir PATH      Durable run directory; reuse to resume an interrupted run
                       (default: artifacts/behavior/readiness/runs/<timestamp>)
  --behavior-id ID    Limit to one behavior ID; repeatable
  --limit N           Limit selected behaviors after sorting, useful for trials
  --force             Re-run behavior reports that already exist in --run-dir
  --progress          Show a single-line progress bar while workers run
  --dry-run           Print selected behavior IDs and exit without calling Codex
  -h, --help          Show this help

Environment overrides:
  TANREN_BEHAVIOR_READINESS_OUTPUT
  TANREN_BEHAVIOR_READINESS_JOBS
  TANREN_BEHAVIOR_READINESS_MODEL
  TANREN_BEHAVIOR_READINESS_REASONING_EFFORT
  TANREN_BEHAVIOR_READINESS_RUN_DIR
EOF
}

die() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

need_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

frontmatter_field() {
    local file="$1"
    local key="$2"
    awk -v key="$key" '
        NR == 1 && $0 == "---" { in_fm = 1; next }
        in_fm && $0 == "---" { exit }
        in_fm {
            split($0, parts, ":")
            if (parts[1] == key) {
                sub("^[^:]+:[[:space:]]*", "")
                print
                exit
            }
        }
    ' "$file"
}

write_schema() {
    local schema_path="$1"
    cat >"$schema_path" <<'EOF'
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "required": [
    "schema_version",
    "artifact",
    "behavior_id",
    "behavior_path",
    "title",
    "area",
    "product_status",
    "verification_status",
    "readiness_status",
    "recommended_verification_status",
    "confidence",
    "summary",
    "similar_behaviors",
    "implementation_evidence",
    "test_evidence",
    "documentation_evidence",
    "architecture_alignment",
    "architecture_evidence",
    "architecture_gaps",
    "gaps",
    "suggested_next_items",
    "roadmap_dependencies",
    "notes"
  ],
  "properties": {
    "schema_version": { "type": "string" },
    "artifact": { "enum": ["behavior_implementation_readiness_report"] },
    "behavior_id": { "type": "string", "pattern": "^B-[0-9]{4}$" },
    "behavior_path": { "type": "string" },
    "title": { "type": "string" },
    "area": { "type": "string" },
    "product_status": { "enum": ["accepted"] },
    "verification_status": { "enum": ["unimplemented", "implemented"] },
    "readiness_status": {
      "enum": [
        "already_implemented",
        "close_needs_work",
        "partial_foundation",
        "not_started",
        "unclear",
        "analysis_failed"
      ]
    },
    "recommended_verification_status": {
      "enum": ["unimplemented", "implemented", "asserted_candidate", "unknown"]
    },
    "confidence": { "type": "string", "enum": ["high", "medium", "low"] },
    "summary": { "type": "string" },
    "similar_behaviors": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["behavior_id", "reason"],
        "properties": {
          "behavior_id": { "type": "string" },
          "reason": { "type": "string" }
        },
        "additionalProperties": false
      }
    },
    "implementation_evidence": { "$ref": "#/$defs/evidence_list" },
    "test_evidence": { "$ref": "#/$defs/evidence_list" },
    "documentation_evidence": { "$ref": "#/$defs/evidence_list" },
    "architecture_alignment": {
      "type": "string",
      "enum": ["aligned", "divergent", "unclear", "not_applicable"]
    },
    "architecture_evidence": { "$ref": "#/$defs/evidence_list" },
    "architecture_gaps": { "$ref": "#/$defs/gap_list" },
    "gaps": {
      "$ref": "#/$defs/gap_list"
    },
    "suggested_next_items": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["title", "kind", "rationale", "depends_on"],
        "properties": {
          "title": { "type": "string" },
          "kind": {
            "type": "string",
            "enum": ["implementation", "bdd_positive", "bdd_falsification", "documentation", "investigation"]
          },
          "rationale": { "type": "string" },
          "depends_on": {
            "type": "array",
            "items": { "type": "string" }
          }
        },
        "additionalProperties": false
      }
    },
    "roadmap_dependencies": {
      "type": "array",
      "items": { "type": "string" }
    },
    "notes": { "type": "array", "items": { "type": "string" } }
  },
  "$defs": {
    "evidence_list": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["path", "line", "summary", "strength"],
        "properties": {
          "path": { "type": "string" },
          "line": { "type": ["integer", "null"] },
          "summary": { "type": "string" },
          "strength": { "type": "string", "enum": ["strong", "partial", "weak"] }
        },
        "additionalProperties": false
      }
    },
    "gap_list": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["gap", "impact", "likely_files"],
        "properties": {
          "gap": { "type": "string" },
          "impact": { "type": "string" },
          "likely_files": {
            "type": "array",
            "items": { "type": "string" }
          }
        },
        "additionalProperties": false
      }
    }
  },
  "additionalProperties": false
}
EOF
}

behavior_selected() {
    local behavior_id="$1"
    if [[ "$FILTER_ID_COUNT" -eq 0 ]]; then
        return 0
    fi
    local selected
    for selected in "${FILTER_IDS[@]}"; do
        if [[ "$selected" == "$behavior_id" ]]; then
            return 0
        fi
    done
    return 1
}

collect_candidates() {
    local file
    find "$ROOT/docs/behaviors" -maxdepth 1 -type f -name 'B-*.md' -print |
        LC_ALL=C sort |
        while IFS= read -r file; do
            local id title area product_status verification_status path
            id="$(frontmatter_field "$file" id)"
            title="$(frontmatter_field "$file" title)"
            area="$(frontmatter_field "$file" area)"
            product_status="$(frontmatter_field "$file" product_status)"
            verification_status="$(frontmatter_field "$file" verification_status)"
            path="${file#"$ROOT"/}"
            if [[ "$product_status" != "accepted" ]]; then
                continue
            fi
            case "$verification_status" in
                unimplemented|implemented)
                    ;;
                *)
                    continue
                    ;;
            esac
            behavior_selected "$id" || continue
            printf '%s\t%s\t%s\t%s\t%s\n' "$id" "$path" "$title" "$area" "$verification_status"
        done
}

aggregate_reports() {
    local output_path="$1"
    local run_dir="$2"
    local model="$3"
    local reasoning_effort="$4"
    local behavior_count="$5"
    local report_files="$run_dir/report-files.txt"
    local reports_json="$run_dir/reports.json"
    local tmp_output="${output_path}.tmp.$$"
    local generated_at report_count
    local report_args=()
    local report_file

    find "$run_dir/reports" -maxdepth 1 -type f -name 'B-*.json' -print | LC_ALL=C sort >"$report_files"
    while IFS= read -r report_file; do
        [[ -n "$report_file" ]] || continue
        report_args+=("$report_file")
    done <"$report_files"

    report_count="${#report_args[@]}"
    if [[ "$report_count" -gt 0 ]]; then
        jq -s 'sort_by(.behavior_id)' "${report_args[@]}" >"$reports_json"
    else
        printf '[]\n' >"$reports_json"
    fi

    mkdir -p "$(dirname "$output_path")"
    generated_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    jq -n \
        --slurpfile reports "$reports_json" \
        --arg generated_at "$generated_at" \
        --arg model "$model" \
        --arg reasoning_effort "$reasoning_effort" \
        --arg output_path "${output_path#"$ROOT"/}" \
        --arg run_dir "${run_dir#"$ROOT"/}" \
        --argjson behavior_count "$behavior_count" \
        --argjson report_count "$report_count" \
        '{
            schema_version: "1.0.0",
            artifact: "behavior_implementation_readiness",
            generated_at: $generated_at,
            model: $model,
            reasoning_effort: $reasoning_effort,
            output_path: $output_path,
            run_dir: $run_dir,
            source_filter: {
                behavior_root: "docs/behaviors",
                product_status: "accepted",
                verification_status: ["unimplemented", "implemented"]
            },
            behavior_count: $behavior_count,
            report_count: $report_count,
            complete: ($behavior_count == $report_count),
            reports: $reports[0]
        }' >"$tmp_output"
    mv "$tmp_output" "$output_path"
}

progress_done_count() {
    local progress_dir="$1"
    find "$progress_dir" -maxdepth 1 -type f -name '*.done' -print 2>/dev/null |
        wc -l |
        tr -d '[:space:]'
}

print_progress_bar() {
    local done_count="$1"
    local total_count="$2"
    local width=30
    local filled percent empty

    if [[ "$total_count" -gt 0 ]]; then
        filled=$((done_count * width / total_count))
        percent=$((done_count * 100 / total_count))
    else
        filled=0
        percent=100
    fi
    empty=$((width - filled))
    printf '\rbehavior readiness ['
    printf '%*s' "$filled" '' | tr ' ' '#'
    printf '%*s' "$empty" '' | tr ' ' '-'
    printf '] %s/%s %s%% checked' "$done_count" "$total_count" "$percent"
}

progress_monitor() {
    local progress_dir="$1"
    local total_count="$2"
    local done_count

    while true; do
        done_count="$(progress_done_count "$progress_dir")"
        print_progress_bar "$done_count" "$total_count"
        sleep 1
    done
}

stop_progress_monitor() {
    if [[ -n "${PROGRESS_PID:-}" ]]; then
        kill "$PROGRESS_PID" >/dev/null 2>&1 || true
        wait "$PROGRESS_PID" >/dev/null 2>&1 || true
        PROGRESS_PID=""
    fi
}

on_exit() {
    local status="$1"
    stop_progress_monitor
    if [[ "${AGGREGATE_ON_EXIT:-0}" == "1" && -n "${OUTPUT_PATH:-}" && -n "${RUN_DIR_PATH:-}" ]]; then
        aggregate_reports "$OUTPUT_PATH" "$RUN_DIR_PATH" "$MODEL" "$REASONING_EFFORT" "$BEHAVIOR_COUNT" >/dev/null 2>&1 || true
    fi
    exit "$status"
}

private_worker() {
    local tmp_dir="$1"
    local model="$2"
    local repo_root="$3"
    local schema_path="$4"
    local reasoning_effort="$5"
    local force="$6"
    local progress_dir="$7"
    local behavior_id="$8"
    local report_path="$tmp_dir/reports/${behavior_id}.json"
    local tmp_report_path="$tmp_dir/reports/${behavior_id}.json.tmp.$$"
    local log_path="$tmp_dir/logs/${behavior_id}.log"
    local prompt_path="$tmp_dir/prompts/${behavior_id}.md"
    local doc_path relative_path title area verification_status product_status

    if [[ -s "$report_path" && "$force" != "1" ]]; then
        touch "$progress_dir/${behavior_id}.done"
        return 0
    fi

    doc_path="$(find "$repo_root/docs/behaviors" -maxdepth 1 -type f -name "${behavior_id}-*.md" -print | LC_ALL=C sort | head -n 1)"
    if [[ -z "$doc_path" ]]; then
        jq -n \
            --arg behavior_id "$behavior_id" \
            --arg summary "Behavior document was not found." \
            '{
                schema_version: "1.0.0",
                artifact: "behavior_implementation_readiness_report",
                behavior_id: $behavior_id,
                behavior_path: "",
                title: "",
                area: "",
                product_status: "accepted",
                verification_status: "unimplemented",
                readiness_status: "analysis_failed",
                recommended_verification_status: "unknown",
                confidence: "low",
                summary: $summary,
                similar_behaviors: [],
                implementation_evidence: [],
                test_evidence: [],
                documentation_evidence: [],
                architecture_alignment: "unclear",
                architecture_evidence: [],
                architecture_gaps: [],
                gaps: [],
                suggested_next_items: [],
                roadmap_dependencies: [],
                notes: []
            }' >"$tmp_report_path"
        mv "$tmp_report_path" "$report_path"
        touch "$progress_dir/${behavior_id}.done"
        return 0
    fi

    relative_path="${doc_path#"$repo_root"/}"
    title="$(frontmatter_field "$doc_path" title)"
    area="$(frontmatter_field "$doc_path" area)"
    product_status="$(frontmatter_field "$doc_path" product_status)"
    verification_status="$(frontmatter_field "$doc_path" verification_status)"

    cat >"$prompt_path" <<EOF
You are producing a thin implementation-readiness report for one Tanren behavior.

Hard constraints:
- Operate in read-only static analysis mode.
- Do not edit, create, delete, format, or rewrite repository files.
- Do not run tests, builds, formatters, fixers, installers, or commands expected to mutate artifacts.
- Prefer fast read-only commands such as rg, sed, find, git grep, git status, and cargo metadata if needed.
- Return only JSON matching the provided output schema.

Behavior under review:
- id: ${behavior_id}
- path: ${relative_path}
- title: ${title}
- area: ${area}
- product_status: ${product_status}
- verification_status: ${verification_status}

Required reading:
- The behavior file above.
- Similar behavior files: same area, directly related IDs in the Related section, superseded/superseding IDs if present, and behavior files with overlapping personas/interfaces/context when relevant.
- Read docs/behaviors/index.md, docs/product/concepts.md, docs/product/personas.md, docs/architecture/subsystems/runtime-actors.md, docs/architecture/subsystems/interfaces.md, and docs/implementation/verification.md.
- Read the accepted architecture before classifying readiness: docs/architecture/system.md, docs/architecture/technology.md, docs/architecture/delivery.md, docs/architecture/operations.md, and every relevant docs/architecture/subsystems/*.md file.
- If needed for this behavior, also read high-level planning docs that exist in this checkout, especially README.md, docs/roadmap/roadmap.md, and docs/roadmap/dag.json.
- Code, tests, command docs, BDD features, and architecture docs that look relevant after searching for behavior terms, related IDs, area terms, and domain concepts.

Classify readiness from the current repository state:
- already_implemented: code appears to support the behavior end to end and the implementation is broadly aligned with the accepted architecture, but assertion evidence may still be missing.
- close_needs_work: clear implementation surface exists and only bounded gaps remain, or behavior support exists but architecture alignment gaps must be fixed before it should count as implemented.
- partial_foundation: adjacent primitives exist, but meaningful product behavior remains.
- not_started: little or no relevant implementation exists.
- unclear: evidence is contradictory or too thin.

Architecture alignment rules:
- Set architecture_alignment to aligned when implementation evidence follows the accepted architecture closely enough for roadmap planning.
- Set architecture_alignment to divergent when code appears to support the behavior through the wrong layer, bypasses the intended tool surface, skips required persistence or evidence paths, violates adapter/runtime boundaries, or conflicts with accepted delivery/operations constraints.
- Set architecture_alignment to unclear when the architecture docs or code evidence are too thin to judge.
- Set architecture_alignment to not_applicable only when the behavior has no meaningful implementation architecture concern.
- Do not use readiness_status already_implemented when architecture_alignment is divergent or unclear. Use close_needs_work or partial_foundation and record architecture_gaps instead.

Use recommended_verification_status this way:
- implemented when implementation appears credible, architecture_alignment is aligned or not_applicable, and BDD assertion is missing.
- unimplemented when behavior should remain below implemented.
- asserted_candidate only when implementation, architecture alignment, and both positive/falsification BDD evidence appear present but the behavior doc is not asserted.
- unknown when analysis failed or evidence is too contradictory.

Keep the report concise. Evidence entries should cite repository-relative paths and line numbers when you have them. Suggested next items should be roadmap-friendly line items, not implementation essays.
EOF

    set +e
    codex exec \
        --cd "$repo_root" \
        --model "$model" \
        -c "model_reasoning_effort=\"$reasoning_effort\"" \
        --sandbox read-only \
        --ephemeral \
        --output-schema "$schema_path" \
        --output-last-message "$tmp_report_path" \
        - <"$prompt_path" >"$log_path" 2>&1
    local status="$?"
    set -e

    if [[ "$status" -eq 0 ]] && jq -e '.artifact == "behavior_implementation_readiness_report"' "$tmp_report_path" >/dev/null 2>&1; then
        mv "$tmp_report_path" "$report_path"
        touch "$progress_dir/${behavior_id}.done"
        return 0
    fi

    local error_text
    error_text="$(tail -n 40 "$log_path" 2>/dev/null || true)"
    jq -n \
        --arg behavior_id "$behavior_id" \
        --arg behavior_path "$relative_path" \
        --arg title "$title" \
        --arg area "$area" \
        --arg product_status "$product_status" \
        --arg verification_status "$verification_status" \
        --arg error_text "$error_text" \
        '{
            schema_version: "1.0.0",
            artifact: "behavior_implementation_readiness_report",
            behavior_id: $behavior_id,
            behavior_path: $behavior_path,
            title: $title,
            area: $area,
            product_status: $product_status,
            verification_status: $verification_status,
            readiness_status: "analysis_failed",
            recommended_verification_status: "unknown",
            confidence: "low",
            summary: "Codex static analysis failed or returned invalid JSON.",
            similar_behaviors: [],
            implementation_evidence: [],
            test_evidence: [],
            documentation_evidence: [],
            architecture_alignment: "unclear",
            architecture_evidence: [],
            architecture_gaps: [
                {
                    gap: "Architecture alignment was not assessed because readiness analysis did not complete.",
                    impact: $error_text,
                    likely_files: [$behavior_path]
                }
            ],
            gaps: [
                {
                    gap: "Readiness analysis did not complete.",
                    impact: $error_text,
                    likely_files: [$behavior_path]
                }
            ],
            suggested_next_items: [
                {
                    title: "Rerun readiness analysis for this behavior",
                    kind: "investigation",
                    rationale: "The parallel Codex worker failed before producing a valid report.",
                    depends_on: []
                }
            ],
            roadmap_dependencies: [],
            notes: []
        }' >"$tmp_report_path"
    mv "$tmp_report_path" "$report_path"
    touch "$progress_dir/${behavior_id}.done"
}

if [[ "${1:-}" == "__worker" ]]; then
    shift
    private_worker "$@"
    exit 0
fi

OUTPUT="${TANREN_BEHAVIOR_READINESS_OUTPUT:-$DEFAULT_OUTPUT}"
JOBS="${TANREN_BEHAVIOR_READINESS_JOBS:-$DEFAULT_JOBS}"
MODEL="${TANREN_BEHAVIOR_READINESS_MODEL:-$DEFAULT_MODEL}"
REASONING_EFFORT="${TANREN_BEHAVIOR_READINESS_REASONING_EFFORT:-$DEFAULT_REASONING_EFFORT}"
RUN_DIR="${TANREN_BEHAVIOR_READINESS_RUN_DIR:-}"
LIMIT=""
DRY_RUN=0
FORCE_REPORTS=0
SHOW_PROGRESS=0
PROGRESS_ENABLED=0
FILTER_IDS=()
FILTER_ID_COUNT=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --output)
            OUTPUT="${2:-}"
            shift 2
            ;;
        --jobs)
            JOBS="${2:-}"
            shift 2
            ;;
        --model)
            MODEL="${2:-}"
            shift 2
            ;;
        --reasoning-effort)
            REASONING_EFFORT="${2:-}"
            shift 2
            ;;
        --run-dir)
            RUN_DIR="${2:-}"
            shift 2
            ;;
        --behavior-id)
            FILTER_IDS+=("${2:-}")
            FILTER_ID_COUNT=$((FILTER_ID_COUNT + 1))
            shift 2
            ;;
        --limit)
            LIMIT="${2:-}"
            shift 2
            ;;
        --force)
            FORCE_REPORTS=1
            shift
            ;;
        --progress)
            SHOW_PROGRESS=1
            shift
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

[[ -n "$OUTPUT" ]] || die "--output requires a path"
[[ -n "$MODEL" ]] || die "--model requires a value"
[[ -n "$REASONING_EFFORT" ]] || die "--reasoning-effort requires a value"
[[ "$JOBS" =~ ^[1-9][0-9]*$ ]] || die "--jobs must be a positive integer"
if [[ -n "$LIMIT" && ! "$LIMIT" =~ ^[1-9][0-9]*$ ]]; then
    die "--limit must be a positive integer"
fi
case "$REASONING_EFFORT" in
    low|medium|high|xhigh)
        ;;
    *)
        die "--reasoning-effort must be one of: low, medium, high, xhigh"
        ;;
esac
if [[ "$FILTER_ID_COUNT" -gt 0 ]]; then
    for behavior_id in "${FILTER_IDS[@]}"; do
        [[ "$behavior_id" =~ ^B-[0-9]{4}$ ]] || die "invalid behavior id: $behavior_id"
    done
fi

need_cmd find
need_cmd awk

if [[ -z "$RUN_DIR" ]]; then
    RUN_DIR="$DEFAULT_RUN_ROOT/$(date -u +%Y%m%dT%H%M%SZ)"
fi
if [[ "$RUN_DIR" != /* ]]; then
    RUN_DIR_PATH="$ROOT/$RUN_DIR"
else
    RUN_DIR_PATH="$RUN_DIR"
fi

if [[ "$OUTPUT" != /* ]]; then
    OUTPUT_PATH="$ROOT/$OUTPUT"
else
    OUTPUT_PATH="$OUTPUT"
fi

if [[ "$DRY_RUN" == "1" ]]; then
    CANDIDATE_DIR="$(mktemp -d "${TMPDIR:-/tmp}/tanren-behavior-readiness.XXXXXX")"
    trap 'rm -rf "$CANDIDATE_DIR"' EXIT
else
    mkdir -p "$RUN_DIR_PATH/prompts" "$RUN_DIR_PATH/reports" "$RUN_DIR_PATH/logs" "$RUN_DIR_PATH/progress"
    CANDIDATE_DIR="$RUN_DIR_PATH"
fi

CANDIDATES="$CANDIDATE_DIR/candidates.tsv"
collect_candidates >"$CANDIDATES"
if [[ -n "$LIMIT" ]]; then
    head -n "$LIMIT" "$CANDIDATES" >"$CANDIDATE_DIR/candidates.limited.tsv"
    mv "$CANDIDATE_DIR/candidates.limited.tsv" "$CANDIDATES"
fi

BEHAVIOR_COUNT="$(wc -l <"$CANDIDATES" | tr -d '[:space:]')"
if [[ "$BEHAVIOR_COUNT" == "0" ]]; then
    die "no accepted behaviors below asserted matched the selection"
fi

if [[ "$DRY_RUN" == "1" ]]; then
    cut -f1 "$CANDIDATES"
    printf 'run_dir=%s\n' "${RUN_DIR_PATH#"$ROOT"/}" >&2
    exit 0
fi

need_cmd codex
need_cmd jq
need_cmd xargs

AGGREGATE_ON_EXIT=1
trap 'on_exit "$?"' EXIT
trap 'on_exit 130' INT
trap 'on_exit 143' TERM

SCHEMA_PATH="$RUN_DIR_PATH/report.schema.json"
write_schema "$SCHEMA_PATH"

PROGRESS_DIR="$RUN_DIR_PATH/progress"
if [[ "$FORCE_REPORTS" == "1" ]]; then
    find "$PROGRESS_DIR" -maxdepth 1 -type f -name '*.done' -delete
else
    while IFS=$'\t' read -r behavior_id _rest; do
        if [[ -s "$RUN_DIR_PATH/reports/${behavior_id}.json" ]]; then
            touch "$PROGRESS_DIR/${behavior_id}.done"
        fi
    done <"$CANDIDATES"
fi

if [[ "$SHOW_PROGRESS" == "1" && -t 1 ]]; then
    PROGRESS_ENABLED=1
    progress_monitor "$PROGRESS_DIR" "$BEHAVIOR_COUNT" &
    PROGRESS_PID="$!"
fi

cut -f1 "$CANDIDATES" |
    xargs -n 1 -P "$JOBS" bash "$0" __worker "$RUN_DIR_PATH" "$MODEL" "$ROOT" "$SCHEMA_PATH" "$REASONING_EFFORT" "$FORCE_REPORTS" "$PROGRESS_DIR"

stop_progress_monitor
if [[ "$PROGRESS_ENABLED" == "1" ]]; then
    print_progress_bar "$(progress_done_count "$PROGRESS_DIR")" "$BEHAVIOR_COUNT"
    printf '\n'
fi

aggregate_reports "$OUTPUT_PATH" "$RUN_DIR_PATH" "$MODEL" "$REASONING_EFFORT" "$BEHAVIOR_COUNT"

printf 'wrote %s (%s/%s behavior reports, run_dir=%s)\n' \
    "${OUTPUT_PATH#"$ROOT"/}" \
    "$(find "$RUN_DIR_PATH/reports" -maxdepth 1 -type f -name 'B-*.json' -print | wc -l | tr -d '[:space:]')" \
    "$BEHAVIOR_COUNT" \
    "${RUN_DIR_PATH#"$ROOT"/}"

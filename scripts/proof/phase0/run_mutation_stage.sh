#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
cd "${REPO_ROOT}"

ENFORCE_STRICT="${PHASE0_MUTATION_ENFORCE:-0}"
if [[ "${ENFORCE_STRICT}" == "1" || "${ENFORCE_STRICT}" == "true" ]]; then
    DEFAULT_OUTPUT_ROOT="${REPO_ROOT}/artifacts/phase0-mutation/enforced"
else
    DEFAULT_OUTPUT_ROOT="${REPO_ROOT}/artifacts/phase0-mutation/staged"
fi
OUTPUT_ROOT="${PHASE0_MUTATION_OUTPUT_ROOT:-${DEFAULT_OUTPUT_ROOT}}"
TRACEABILITY_PATH="${PHASE0_BEHAVIOR_TRACEABILITY_FILE:-${REPO_ROOT}/docs/rewrite/PHASE0_BEHAVIOR_TRACEABILITY.json}"
SHARD="${PHASE0_MUTATION_SHARD:-0/32}"
TIMEOUT_SECS="${PHASE0_MUTATION_TIMEOUT_SECS:-180}"

RUN_STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
RUN_DIR="${OUTPUT_ROOT}/${RUN_STAMP}"
mkdir -p "${RUN_DIR}"

MUTANTS_OUT_DIR="${RUN_DIR}/mutants.out"
STATUS="skipped_missing_tool"
EXIT_CODE=0
VERSION=""

if command -v cargo-mutants >/dev/null 2>&1; then
    STATUS="executed"
    VERSION="$(cargo-mutants --version 2>/dev/null || true)"

    cmd=(
        cargo mutants
        --output "${MUTANTS_OUT_DIR}"
        --in-place
        --baseline=skip
        --no-shuffle
        --timeout "${TIMEOUT_SECS}"
        --shard "${SHARD}"
        --package tanren-bdd-phase0
        --test-package tanren-bdd-phase0
        --file crates/tanren-bdd-phase0/src/main.rs
        --file crates/tanren-bdd-phase0/src/wave_b_steps.rs
        --file crates/tanren-bdd-phase0/src/wave_c_steps.rs
    )

    printf '%q ' "${cmd[@]}" >"${RUN_DIR}/command.txt"
    printf '\n' >>"${RUN_DIR}/command.txt"

    set +e
    "${cmd[@]}" >"${RUN_DIR}/cargo-mutants.stdout.log" 2>"${RUN_DIR}/cargo-mutants.stderr.log"
    EXIT_CODE=$?
    set -e

    if [[ ${EXIT_CODE} -eq 0 ]]; then
        STATUS="executed_clean"
    else
        STATUS="executed_nonzero"
    fi
else
    printf '%s\n' "cargo-mutants --version" >"${RUN_DIR}/command.txt"
    printf '%s\n' "Phase 0 mutation stage: cargo-mutants is not installed; emitting triage scaffold only." >"${RUN_DIR}/cargo-mutants.stderr.log"
fi

uv run python scripts/proof/phase0/render_mutation_triage.py \
    --traceability "${TRACEABILITY_PATH}" \
    --run-dir "${RUN_DIR}" \
    --status "${STATUS}" \
    --exit-code "${EXIT_CODE}" \
    --version "${VERSION}" \
    --shard "${SHARD}" \
    --mutants-out "${MUTANTS_OUT_DIR}"

mkdir -p "${OUTPUT_ROOT}"
ln -sfn "${RUN_STAMP}" "${OUTPUT_ROOT}/latest"

printf '%s\n' "Phase 0 mutation stage artifact: ${OUTPUT_ROOT}/latest/triage.json"

if [[ "${STATUS}" == "executed_nonzero" ]]; then
    if [[ "${ENFORCE_STRICT}" == "1" || "${ENFORCE_STRICT}" == "true" ]]; then
        printf '%s\n' "Phase 0 mutation gate: cargo-mutants exited ${EXIT_CODE}; triage captured."
    else
        printf '%s\n' "Phase 0 mutation stage: cargo-mutants exited ${EXIT_CODE}; triage captured (non-blocking in staged mode)."
    fi
fi

if [[ "${ENFORCE_STRICT}" == "1" || "${ENFORCE_STRICT}" == "true" ]] && [[ "${STATUS}" != "executed_clean" ]]; then
    triage_path="${RUN_DIR}/triage.json"
    if [[ -f "${triage_path}" ]] && command -v jq >/dev/null 2>&1; then
        missed_count="$(jq -r '.outcomes.missed_count // 0' "${triage_path}")"
        unviable_count="$(jq -r '.outcomes.unviable_count // 0' "${triage_path}")"
        tested_count="$(jq -r '.outcomes.tested_count // 0' "${triage_path}")"
        printf '%s\n' \
            "Phase 0 mutation gate summary: tested=${tested_count} missed=${missed_count} unviable=${unviable_count}"
        if [[ "${missed_count}" != "0" ]]; then
            printf '%s\n' "Phase 0 mutation missed mutants (first 5):"
            jq -r '.survivors[] | select(.outcome == "missed") | .mutant' "${triage_path}" | head -n 5 | sed 's/^/  - /'
        fi
    fi
    strict_exit="${EXIT_CODE}"
    if [[ "${strict_exit}" -eq 0 ]]; then
        strict_exit=1
    fi
    printf '%s\n' \
        "FAIL: Phase 0 mutation gate requires clean execution (status=${STATUS}, exit=${EXIT_CODE})."
    exit "${strict_exit}"
fi

exit 0

#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
cd "${REPO_ROOT}"

ENFORCE_STRICT="${PHASE0_COVERAGE_ENFORCE:-0}"
if [[ "${ENFORCE_STRICT}" == "1" || "${ENFORCE_STRICT}" == "true" ]]; then
    DEFAULT_OUTPUT_ROOT="${REPO_ROOT}/artifacts/phase0-coverage/enforced"
else
    DEFAULT_OUTPUT_ROOT="${REPO_ROOT}/artifacts/phase0-coverage/staged"
fi
OUTPUT_ROOT="${PHASE0_COVERAGE_OUTPUT_ROOT:-${DEFAULT_OUTPUT_ROOT}}"
TRACEABILITY_PATH="${PHASE0_BEHAVIOR_TRACEABILITY_FILE:-${REPO_ROOT}/docs/rewrite/PHASE0_BEHAVIOR_TRACEABILITY.json}"

RUN_STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
RUN_DIR="${OUTPUT_ROOT}/${RUN_STAMP}"
mkdir -p "${RUN_DIR}"

FEATURE_EXECUTIONS_PATH="${RUN_DIR}/feature-executions.ndjson"
COVERAGE_SUMMARY_PATH="${RUN_DIR}/coverage-summary.json"

STATUS="skipped_missing_tool"
EXIT_CODE=0
VERSION=""

feature_files=(
    "tests/bdd/phase0/smoke.feature"
    "tests/bdd/phase0/feature-1-typed-control-plane-state.feature"
    "tests/bdd/phase0/feature-2-event-history.feature"
    "tests/bdd/phase0/feature-3-contract-derived-interface.feature"
    "tests/bdd/phase0/feature-4-methodology-boundary.feature"
    "tests/bdd/phase0/feature-5-task-completion-guards.feature"
    "tests/bdd/phase0/feature-6-tool-surface-contract.feature"
    "tests/bdd/phase0/feature-7-installer-determinism.feature"
    "tests/bdd/phase0/feature-8-manual-methodology-walkthrough.feature"
)

: >"${FEATURE_EXECUTIONS_PATH}"

if command -v cargo-llvm-cov >/dev/null 2>&1 || cargo llvm-cov --version >/dev/null 2>&1; then
    STATUS="executed"
    VERSION="$(cargo llvm-cov --version 2>/dev/null || true)"

    {
        printf '%s\n' "cargo llvm-cov clean --workspace"
        printf '%s\n' \
            "TANREN_BDD_PHASE0_FEATURE_PATH=tests/bdd/phase0 cargo llvm-cov run --package tanren-bdd-phase0 --bin tanren-bdd-phase0 --locked --quiet --summary-only --json --output-path ${COVERAGE_SUMMARY_PATH}"
        printf '%s\n' \
            "cargo llvm-cov report --package tanren-bdd-phase0 --lcov --output-path ${RUN_DIR}/lcov.info --locked"
    } >"${RUN_DIR}/command.txt"

    set +e
    cargo llvm-cov clean --workspace >"${RUN_DIR}/coverage-clean.stdout.log" \
        2>"${RUN_DIR}/coverage-clean.stderr.log"
    clean_exit=$?
    set -e
    if [[ ${clean_exit} -ne 0 ]]; then
        STATUS="executed_nonzero"
        EXIT_CODE=${clean_exit}
    fi

    set +e
    TANREN_BDD_PHASE0_FEATURE_PATH="tests/bdd/phase0" \
        cargo llvm-cov run \
            --package tanren-bdd-phase0 \
            --bin tanren-bdd-phase0 \
            --locked \
            --quiet \
            --summary-only \
            --json \
            --output-path "${COVERAGE_SUMMARY_PATH}" \
            >"${RUN_DIR}/coverage-run.stdout.log" \
            2>"${RUN_DIR}/coverage-run.stderr.log"
    coverage_run_exit=$?
    set -e
    if [[ ${coverage_run_exit} -ne 0 ]]; then
        STATUS="executed_nonzero"
        if [[ ${EXIT_CODE} -eq 0 ]]; then
            EXIT_CODE=${coverage_run_exit}
        fi
    fi

    set +e
    cargo llvm-cov report \
        --package tanren-bdd-phase0 \
        --lcov \
        --output-path "${RUN_DIR}/lcov.info" \
        --locked \
        >"${RUN_DIR}/coverage-lcov.stdout.log" \
        2>"${RUN_DIR}/coverage-lcov.stderr.log"
    lcov_exit=$?
    set -e
    if [[ ${lcov_exit} -ne 0 ]]; then
        STATUS="executed_nonzero"
        if [[ ${EXIT_CODE} -eq 0 ]]; then
            EXIT_CODE=${lcov_exit}
        fi
    fi

    for feature_file in "${feature_files[@]}"; do
        if [[ ! -f "${feature_file}" ]]; then
            printf '%s\n' \
                "Phase 0 coverage stage: missing feature file ${feature_file}." \
                >>"${RUN_DIR}/coverage-missing-feature.log"
            printf '{"feature_file":"%s","status":"missing_file","exit_code":127}\n' \
                "${feature_file}" >>"${FEATURE_EXECUTIONS_PATH}"
            STATUS="executed_nonzero"
            if [[ ${EXIT_CODE} -eq 0 ]]; then
                EXIT_CODE=1
            fi
            continue
        fi

        if [[ ${coverage_run_exit} -eq 0 ]]; then
            feature_status="passed"
            feature_exit=0
        else
            feature_status="failed"
            feature_exit=${coverage_run_exit}
        fi

        printf '{"feature_file":"%s","status":"%s","exit_code":%d}\n' \
            "${feature_file}" "${feature_status}" "${feature_exit}" >>"${FEATURE_EXECUTIONS_PATH}"
    done
else
    printf '%s\n' \
        "cargo llvm-cov --version" >"${RUN_DIR}/command.txt"
    printf '%s\n' \
        "Phase 0 coverage stage: cargo-llvm-cov is not installed; emitting classification scaffold only." \
        >"${RUN_DIR}/coverage-run.stderr.log"
fi

UV_CACHE_DIR="${UV_CACHE_DIR:-/tmp/uv-cache}" \
    uv run python scripts/proof/phase0/render_coverage_classification.py \
        --traceability "${TRACEABILITY_PATH}" \
        --run-dir "${RUN_DIR}" \
        --status "${STATUS}" \
        --exit-code "${EXIT_CODE}" \
        --version "${VERSION}" \
        --coverage-summary "${COVERAGE_SUMMARY_PATH}" \
        --feature-executions "${FEATURE_EXECUTIONS_PATH}"

mkdir -p "${OUTPUT_ROOT}"
ln -sfn "${RUN_STAMP}" "${OUTPUT_ROOT}/latest"

printf '%s\n' "Phase 0 coverage classification artifact: ${OUTPUT_ROOT}/latest/classification.json"

if [[ "${STATUS}" == "executed_nonzero" ]]; then
    if [[ "${ENFORCE_STRICT}" == "1" || "${ENFORCE_STRICT}" == "true" ]]; then
        printf '%s\n' \
            "Phase 0 coverage gate: one or more coverage commands failed (exit=${EXIT_CODE}); classification captured."
    else
        printf '%s\n' \
            "Phase 0 coverage stage: one or more coverage commands failed (exit=${EXIT_CODE}); classification captured (non-blocking in staged mode)."
    fi
fi

if [[ "${ENFORCE_STRICT}" == "1" || "${ENFORCE_STRICT}" == "true" ]] && [[ "${STATUS}" != "executed" ]]; then
    strict_exit="${EXIT_CODE}"
    if [[ "${strict_exit}" -eq 0 ]]; then
        strict_exit=1
    fi
    printf '%s\n' \
        "FAIL: Phase 0 coverage gate requires successful execution (status=${STATUS}, exit=${EXIT_CODE})."
    exit "${strict_exit}"
fi

exit 0

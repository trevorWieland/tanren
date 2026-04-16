#!/usr/bin/env bash
#
# Postgres integration wrapper for `just ci`.
#
# Runs the two Postgres-backed nextest suites (`tanren-store` and
# `tanren-cli`) against either an explicit TANREN_TEST_POSTGRES_URL or a
# container-runtime-started ephemeral Postgres.
#
# Fail-hard policy (per lane 0.4 audit remediation): this script never
# skips silently. If neither a URL nor a container runtime is available,
# it exits non-zero with actionable guidance so `just ci` cannot pass on
# a host that cannot actually verify Postgres behavior.

set -euo pipefail

readonly CARGO="${CARGO:-cargo}"
readonly POSTGRES_URL_ENV="TANREN_TEST_POSTGRES_URL"

preflight() {
    if [[ -n "${!POSTGRES_URL_ENV:-}" ]]; then
        echo "==> Postgres integration: using ${POSTGRES_URL_ENV}"
        return 0
    fi

    if command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
        echo "==> Postgres integration: docker is available; fixture will spin a container"
        return 0
    fi

    if command -v podman >/dev/null 2>&1 && podman info >/dev/null 2>&1; then
        echo "==> Postgres integration: podman is available; fixture will spin a container"
        return 0
    fi

    cat >&2 <<EOF
ERROR: Postgres integration is required but no runtime is available.
       Set ${POSTGRES_URL_ENV}=postgres://user:pass@host:port/db
       or start Docker/Podman/Colima and re-run.

       Rationale: lane 0.4 requires 'just ci' to be reproducibly green
       including the Postgres-backed suites that exercise SKIP LOCKED,
       deadlock classification, and scoped-index EXPLAIN plans. These
       semantics cannot be validated against SQLite.
EOF
    exit 1
}

run_store_postgres_tests() {
    echo "==> Running tanren-store Postgres integration tests"
    PGSSLMODE="${PGSSLMODE:-disable}" \
    RUSTFLAGS="-D warnings" \
        "${CARGO}" nextest run \
            -p tanren-store \
            --features tanren-store/test-hooks,tanren-store/postgres-integration \
            --no-tests=pass
}

run_cli_postgres_tests() {
    echo "==> Running tanren-cli Postgres integration tests"
    PGSSLMODE="${PGSSLMODE:-disable}" \
    RUSTFLAGS="-D warnings" \
        "${CARGO}" nextest run \
            -p tanren-cli \
            --features tanren-cli/postgres-integration \
            --no-tests=pass
}

main() {
    preflight
    run_store_postgres_tests
    run_cli_postgres_tests
    echo "==> All Postgres integration tests passed"
}

main "$@"

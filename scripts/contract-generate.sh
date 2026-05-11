#!/usr/bin/env bash
# scripts/contract-generate.sh — canonical interface-owned contract generation
#
# Builds the Rust OpenAPI binary (tanren-api-openapi) with locked
# dependencies, emits an OpenAPI JSON document, and converts it to
# TypeScript via openapi-typescript.  All first-party clients that
# consume the contract (web, future SDKs) call this script — they do
# not own independent generation logic.
#
# Usage:
#   scripts/contract-generate.sh [<output-ts-path>]
#
# When <output-ts-path> is omitted the default projection is written to
# apps/web/src/lib/generated/api-contract.ts.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
output_ts="${1:-${repo_root}/apps/web/src/lib/generated/api-contract.ts}"
openapi_json="$(mktemp -t tanren-openapi.XXXXXX.json)"

cleanup() {
    rm -f "${openapi_json}"
}
trap cleanup EXIT

cd "${repo_root}"

CARGO_INCREMENTAL=0 cargo build -p tanren-api-app --bin tanren-api-openapi --locked --quiet
cargo run -q -p tanren-api-app --bin tanren-api-openapi --locked > "${openapi_json}"
pnpm --filter @tanren/web exec openapi-typescript "${openapi_json}" --output "${output_ts}"
pnpm --filter @tanren/web exec prettier --write "${output_ts}"

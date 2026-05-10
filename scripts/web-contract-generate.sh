#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
openapi_json="$(mktemp -t tanren-openapi.XXXXXX.json)"
contract_ts="${repo_root}/apps/web/src/lib/generated/api-contract.ts"

cleanup() {
    rm -f "${openapi_json}"
}
trap cleanup EXIT

cd "${repo_root}"

CARGO_INCREMENTAL=0 cargo build -p tanren-api-app --bin tanren-api-openapi --locked --quiet
cargo run -q -p tanren-api-app --bin tanren-api-openapi --locked > "${openapi_json}"
pnpm --filter @tanren/web exec openapi-typescript "${openapi_json}" --output "${contract_ts}"
pnpm --filter @tanren/web exec prettier --write "${contract_ts}"

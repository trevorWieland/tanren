#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'USAGE'
Usage: scripts/runtime/install-runtime.sh [--root <repo-root>] [--skip-verify]

Install tanren runtime binaries into Cargo's bin directory using locked deps:
- tanren-cli
- tanren-mcp
USAGE
}

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SKIP_VERIFY=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --root)
            ROOT="$2"
            shift 2
            ;;
        --skip-verify)
            SKIP_VERIFY=1
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown arg: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

cd "${ROOT}"

cargo install --locked --path bin/tanren-cli --bin tanren-cli --force
cargo install --locked --path bin/tanren-mcp --bin tanren-mcp --force

if [[ "${SKIP_VERIFY}" == "0" ]]; then
    "${ROOT}/scripts/runtime/verify-installed-runtime.sh"
fi

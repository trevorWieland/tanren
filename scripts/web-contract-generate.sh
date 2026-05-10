#!/usr/bin/env bash
# scripts/web-contract-generate.sh — web projection of the interface-owned
# contract generation path.
#
# Delegates to the canonical entrypoint (scripts/contract-generate.sh)
# which builds the Rust OpenAPI binary and converts to TypeScript.
# The web package does not own generation logic.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "${script_dir}/contract-generate.sh" "$@"

#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'USAGE'
Usage: scripts/runtime/verify-installed-runtime.sh

Verify tanren runtime binaries are installed and PATH-callable.
Emits a JSON report to stdout.
USAGE
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
    usage
    exit 0
fi

need_cmd() {
    command -v "$1" >/dev/null 2>&1
}

runtime_ok=true

cli_path="$(command -v tanren-cli || true)"
mcp_path="$(command -v tanren-mcp || true)"
cli_executable=false
mcp_executable=false

if [[ -n "${cli_path}" && -x "${cli_path}" ]]; then
    cli_executable=true
else
    runtime_ok=false
fi
if [[ -n "${mcp_path}" && -x "${mcp_path}" ]]; then
    mcp_executable=true
else
    runtime_ok=false
fi

cli_version=""
mcp_version=""
if [[ -n "${cli_path}" && -x "${cli_path}" ]]; then
    cli_version="$(tanren-cli --version 2>/dev/null || true)"
fi
if [[ -n "${mcp_path}" && -x "${mcp_path}" ]]; then
    mcp_version="$(tanren-mcp --version 2>/dev/null || true)"
fi

if ! need_cmd jq; then
    echo "verify-installed-runtime.sh requires jq" >&2
    exit 2
fi

jq -n \
  --arg generated_at_utc "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg cli_path "${cli_path}" \
  --argjson cli_executable "${cli_executable}" \
  --arg cli_version "${cli_version}" \
  --arg mcp_path "${mcp_path}" \
  --argjson mcp_executable "${mcp_executable}" \
  --arg mcp_version "${mcp_version}" \
  --argjson runtime_ok "${runtime_ok}" \
  '{
    generated_at_utc: $generated_at_utc,
    runtime_ok: $runtime_ok,
    tanren_cli: {
      path: $cli_path,
      executable: $cli_executable,
      version: $cli_version
    },
    tanren_mcp: {
      path: $mcp_path,
      executable: $mcp_executable,
      version: $mcp_version
    }
  }'

if [[ "${runtime_ok}" != "true" ]]; then
    exit 1
fi

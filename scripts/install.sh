#!/usr/bin/env bash
set -euo pipefail

cat >&2 <<'EOF'
[tanren] ERROR: scripts/install.sh is permanently deprecated for the rewrite runtime.

Use the supported install flow instead:
  1) scripts/runtime/install-runtime.sh
  2) scripts/runtime/verify-installed-runtime.sh
  3) tanren-cli install --config tanren.yml

Legacy `scripts/install.sh --profile ...` is intentionally disabled.
EOF

exit 2

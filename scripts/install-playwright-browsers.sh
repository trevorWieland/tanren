#!/usr/bin/env bash
set -euo pipefail

browser_path="${PLAYWRIGHT_BROWSERS_PATH:-}"
if [[ -z "$browser_path" ]]; then
    case "$(uname -s)" in
        Darwin)  browser_path="$HOME/Library/Caches/ms-playwright" ;;
        Linux)   browser_path="$HOME/.cache/ms-playwright" ;;
        MINGW*|MSYS*|CYGWIN*|Windows*) browser_path="${LOCALAPPDATA:-$HOME/AppData/Local}/ms-playwright" ;;
        *)       browser_path="$HOME/.cache/ms-playwright" ;;
    esac
fi

if compgen -G "${browser_path}/chromium-*" &>/dev/null; then
    exit 0
fi

echo "==> Installing Playwright Chromium browser..."

if [[ "$(id -u)" -eq 0 ]]; then
    pnpm --filter @tanren/web exec playwright install --with-deps chromium
elif sudo -n true 2>/dev/null; then
    pnpm --filter @tanren/web exec playwright install --with-deps chromium
else
    pnpm --filter @tanren/web exec playwright install chromium
fi

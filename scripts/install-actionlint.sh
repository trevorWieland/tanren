#!/usr/bin/env bash
set -euo pipefail

if command -v actionlint &>/dev/null; then
    exit 0
fi

install_dir="${ACTIONLINT_INSTALL_DIR:-${CARGO_HOME:-$HOME/.cargo}/bin}"
version="${ACTIONLINT_VERSION:-}"
if [[ -z "$version" ]]; then
    version="$(curl -fsSL https://api.github.com/repos/rhysd/actionlint/releases/latest \
        | sed -n 's/.*"tag_name":[[:space:]]*"v\([^"]*\)".*/\1/p')"
fi
version="${version#v}"
if [[ -z "$version" ]]; then
    echo "FAIL: could not resolve latest actionlint version" >&2
    exit 1
fi

os="$(uname -s | tr '[:upper:]' '[:lower:]')"
archive_ext="tar.gz"
actionlint_bin="actionlint"
case "$os" in
    darwin|linux) ;;
    mingw*|msys*|cygwin*|windows*)
        os="windows"
        archive_ext="zip"
        actionlint_bin="actionlint.exe"
        ;;
    *)
        echo "FAIL: unsupported actionlint OS: $os" >&2
        exit 1
        ;;
esac

arch="$(uname -m)"
case "$arch" in
    i386|i686|x86) arch="386" ;;
    x86_64|amd64) arch="amd64" ;;
    arm64|aarch64) arch="arm64" ;;
    armv6l|armv6) arch="armv6" ;;
    *)
        echo "FAIL: unsupported actionlint architecture: $arch" >&2
        exit 1
        ;;
esac

tmpdir="$(mktemp -d)"
cleanup() {
    rm -rf "$tmpdir"
}
trap cleanup EXIT

url="https://github.com/rhysd/actionlint/releases/download/v${version}/actionlint_${version}_${os}_${arch}.${archive_ext}"
if [[ "$archive_ext" == "zip" ]]; then
    archive="$tmpdir/actionlint.zip"
    curl -fsSL "$url" -o "$archive"
    if command -v unzip &>/dev/null; then
        unzip -q "$archive" "$actionlint_bin" -d "$tmpdir"
    elif command -v powershell.exe &>/dev/null; then
        powershell.exe -NoProfile -Command "Expand-Archive -LiteralPath '$archive' -DestinationPath '$tmpdir' -Force" >/dev/null
    elif command -v pwsh &>/dev/null; then
        pwsh -NoProfile -Command "Expand-Archive -LiteralPath '$archive' -DestinationPath '$tmpdir' -Force" >/dev/null
    else
        echo "FAIL: unzip, powershell.exe, or pwsh is required to install actionlint on Windows" >&2
        exit 1
    fi
else
    curl -fsSL "$url" | tar -xz -C "$tmpdir" "$actionlint_bin"
fi

if [[ ! -f "$tmpdir/$actionlint_bin" ]]; then
    echo "FAIL: actionlint archive did not contain $actionlint_bin" >&2
    exit 1
fi

mkdir -p "$install_dir"
cp "$tmpdir/$actionlint_bin" "$install_dir/$actionlint_bin"
chmod +x "$install_dir/$actionlint_bin"

#!/usr/bin/env bash
#
# install.sh — Install or update tanren in a target project
#
# Run from inside a target project directory.
#
# Usage:
#   ~/github/tanren/scripts/install.sh
#   ~/github/tanren/scripts/install.sh --profile python-uv
#   ~/github/tanren/scripts/install.sh --tanren-path /path/to/tanren
#

set -euo pipefail

# --- Colors ---
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m'

info()  { echo -e "${BLUE}[tanren]${NC} $*"; }
ok()    { echo -e "${GREEN}[tanren]${NC} $*"; }
warn()  { echo -e "${YELLOW}[tanren]${NC} $*"; }
fail()  { echo -e "${RED}[tanren]${NC} $*"; exit 1; }

# --- Defaults ---
TANREN_PATH="${HOME}/github/tanren"
PROFILE="python-uv"

# --- Parse arguments ---
while [[ $# -gt 0 ]]; do
    case "$1" in
        --tanren-path)
            TANREN_PATH="$2"
            shift 2
            ;;
        --profile)
            PROFILE="$2"
            shift 2
            ;;
        --help)
            echo "Usage: install.sh [--tanren-path PATH] [--profile NAME]"
            echo ""
            echo "  --tanren-path PATH   Path to tanren repo (default: ~/github/tanren)"
            echo "  --profile NAME       Standards profile to use (default: python-uv)"
            echo "  --help               Show this help"
            exit 0
            ;;
        *)
            fail "Unknown argument: $1"
            ;;
    esac
done

# --- Validate tanren path ---
if [[ ! -d "$TANREN_PATH" ]]; then
    fail "tanren repo not found at: $TANREN_PATH"
fi
if [[ ! -f "$TANREN_PATH/config.yml" ]]; then
    fail "Not a valid tanren repo (missing config.yml): $TANREN_PATH"
fi

# --- Detect install mode ---
if [[ -d "tanren" ]]; then
    MODE="update"
    info "Detected existing tanren installation — update mode"
else
    MODE="fresh"
    info "No existing tanren directory — fresh install"
fi

# --- Validate profile ---
PROFILE_DIR="$TANREN_PATH/profiles/$PROFILE"
if [[ ! -d "$PROFILE_DIR" ]]; then
    warn "Profile '$PROFILE' not found at $PROFILE_DIR"
    if [[ -d "$TANREN_PATH/profiles/default" ]]; then
        warn "Falling back to 'default' profile"
        PROFILE="default"
        PROFILE_DIR="$TANREN_PATH/profiles/default"
    else
        fail "No profiles found in $TANREN_PATH/profiles/"
    fi
fi

# --- Fresh install: create project structure ---
if [[ "$MODE" == "fresh" ]]; then
    info "Creating tanren directory structure..."
    mkdir -p tanren/{product,standards,specs,audits,scripts}

    # Copy template product docs
    if [[ -d "$TANREN_PATH/templates/product" ]]; then
        cp "$TANREN_PATH/templates/product/"*.md tanren/product/
        ok "Copied product templates"
    fi

    # Copy audit template
    if [[ -d "$TANREN_PATH/templates/audits" ]]; then
        cp "$TANREN_PATH/templates/audits/"*.md tanren/audits/
        ok "Copied audit template"
    fi

    # Copy profile standards
    info "Installing standards from profile: $PROFILE"
    # Copy preserving subdirectory structure
    cd "$PROFILE_DIR"
    find . -name "*.md" -type f | while read -r file; do
        target_dir="$(pwd)/../../.."
        # We need to go back to the project dir
        dest_dir="tanren/standards/$(dirname "$file")"
        echo "$file -> $dest_dir"
    done
    cd - > /dev/null

    # Actually copy with directory structure
    for subdir in $(find "$PROFILE_DIR" -mindepth 1 -maxdepth 1 -type d); do
        dirname=$(basename "$subdir")
        mkdir -p "tanren/standards/$dirname"
        cp "$subdir/"*.md "tanren/standards/$dirname/" 2>/dev/null || true
    done
    # Copy any root-level files
    cp "$PROFILE_DIR/"*.md tanren/standards/ 2>/dev/null || true
    ok "Installed $(find tanren/standards -name '*.md' -type f | wc -l) standards"

    # Generate initial index.yml
    info "Generating standards index..."
    {
        echo "# Standards Index"
        echo ""
        for subdir in $(find tanren/standards -mindepth 1 -maxdepth 1 -type d | sort); do
            dirname=$(basename "$subdir")
            echo "$dirname:"
            for file in $(find "$subdir" -name "*.md" -type f | sort); do
                slug=$(basename "$file" .md)
                # Extract first line (title) from the file
                title=$(head -1 "$file" | sed 's/^#\s*//')
                echo "  $slug:"
                echo "    description: $title"
            done
            echo ""
        done
    } > tanren/standards/index.yml
    ok "Generated standards index"
fi

# --- Both install and update: copy scripts ---
info "Copying scripts..."
mkdir -p tanren/scripts
for script in orchestrate.sh list-candidates.py audit-standards.sh fanfare.wav; do
    if [[ -f "$TANREN_PATH/scripts/$script" ]]; then
        cp "$TANREN_PATH/scripts/$script" "tanren/scripts/$script"
    fi
done
chmod +x tanren/scripts/orchestrate.sh tanren/scripts/audit-standards.sh 2>/dev/null || true
ok "Scripts updated"

# --- Both install and update: copy commands ---
info "Copying commands..."
mkdir -p .claude/commands/tanren
mkdir -p .opencode/commands/tanren
for cmd in "$TANREN_PATH/commands/"*.md; do
    if [[ -f "$cmd" ]]; then
        cp "$cmd" ".claude/commands/tanren/"
        cp "$cmd" ".opencode/commands/tanren/"
    fi
done
cmd_count=$(ls .claude/commands/tanren/*.md 2>/dev/null | wc -l)
ok "Installed $cmd_count commands to .claude/commands/tanren/ and .opencode/commands/tanren/"

# --- Write tanren.yml ---
cat > tanren.yml <<EOF
version: 0.1.0
profile: $PROFILE
installed: $(date +%Y-%m-%d)
EOF
ok "Wrote tanren.yml"

# --- Summary ---
echo ""
echo -e "${BOLD}tanren installation complete${NC}"
echo ""
if [[ "$MODE" == "fresh" ]]; then
    echo "  Mode:     Fresh install"
    echo "  Profile:  $PROFILE"
    echo "  Standards: $(find tanren/standards -name '*.md' -type f | wc -l) files"
fi
echo "  Commands: $cmd_count files (in .claude/ and .opencode/)"
echo "  Scripts:  tanren/scripts/"
echo "  Config:   tanren.yml"
echo ""
if [[ "$MODE" == "fresh" ]]; then
    echo "Next steps:"
    echo "  1. Review tanren/product/ templates and fill in your project details"
    echo "  2. Review tanren/standards/ and adjust for your project"
    echo "  3. Run /shape-spec to start your first spec"
fi

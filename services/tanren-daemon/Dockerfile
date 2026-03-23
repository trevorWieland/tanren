# syntax=docker/dockerfile:1
# Build context: repo root (docker build -f services/tanren-daemon/Dockerfile .)
#
# Extras (default "all" includes every adapter; pass "" for a slim image):
#   docker build -f services/tanren-daemon/Dockerfile .                           # all adapters
#   docker build --build-arg EXTRAS="hetzner" -f services/tanren-daemon/Dockerfile .  # hetzner only
#   docker build --build-arg EXTRAS="" -f services/tanren-daemon/Dockerfile .     # no cloud adapters

# ── Build stage ──────────────────────────────────────────────────────────────
FROM python:3.14-slim-bookworm AS build

ARG EXTRAS="all"

COPY --from=ghcr.io/astral-sh/uv:latest /uv /usr/local/bin/uv

WORKDIR /app

# Layer 1: workspace pyproject.toml files + lockfile (cached until deps change)
COPY pyproject.toml uv.lock ./
COPY packages/tanren-core/pyproject.toml packages/tanren-core/pyproject.toml
COPY services/tanren-api/pyproject.toml  services/tanren-api/pyproject.toml
COPY services/tanren-cli/pyproject.toml  services/tanren-cli/pyproject.toml
COPY services/tanren-daemon/pyproject.toml services/tanren-daemon/pyproject.toml

# Layer 2: install third-party deps only (expensive, cached until lockfile changes)
RUN uv sync --frozen --no-cache --no-dev --no-editable --package tanren-daemon --no-install-workspace \
    $(for extra in $EXTRAS; do echo "--extra $extra"; done)

# Layer 3: copy workspace source
COPY packages/tanren-core/src packages/tanren-core/src
COPY services/tanren-daemon/src services/tanren-daemon/src

# Layer 4: install workspace packages from source (fast)
RUN uv sync --frozen --no-cache --no-dev --no-editable --package tanren-daemon \
    $(for extra in $EXTRAS; do echo "--extra $extra"; done)

# ── Runtime stage ────────────────────────────────────────────────────────────
FROM python:3.14-slim-bookworm

# git: worktree operations; openssh-client: SSH connections to VMs
RUN apt-get update \
 && apt-get install -y --no-install-recommends git openssh-client \
 && rm -rf /var/lib/apt/lists/*

RUN groupadd --gid 10001 app \
 && useradd --uid 10001 --gid app --no-create-home --shell /usr/sbin/nologin app

COPY --from=build /app/.venv /app/.venv

ENV PATH="/app/.venv/bin:$PATH"

WORKDIR /app

# Default data directories for volume mounts
RUN mkdir -p /data /repos \
 && chown -R app:app /app /data /repos

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD kill -0 1 || exit 1

USER app

ENTRYPOINT ["tanren-daemon"]

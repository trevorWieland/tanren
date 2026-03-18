# syntax=docker/dockerfile:1
# Build context: repo root (docker build -f services/tanren-api/Dockerfile .)

# ── Build stage ──────────────────────────────────────────────────────────────
FROM python:3.14-slim-bookworm AS build

COPY --from=ghcr.io/astral-sh/uv:latest /uv /usr/local/bin/uv

WORKDIR /app

# Layer 1: workspace pyproject.toml files + lockfile (cached until deps change)
COPY pyproject.toml uv.lock ./
COPY packages/tanren-core/pyproject.toml packages/tanren-core/pyproject.toml
COPY services/tanren-api/pyproject.toml  services/tanren-api/pyproject.toml
COPY services/tanren-cli/pyproject.toml  services/tanren-cli/pyproject.toml
COPY services/tanren-daemon/pyproject.toml services/tanren-daemon/pyproject.toml

# Layer 2: install third-party deps only (expensive, cached until lockfile changes)
RUN uv sync --frozen --no-cache --no-dev --no-editable --package tanren-api --no-install-workspace

# Layer 3: copy workspace source
COPY packages/tanren-core/src packages/tanren-core/src
COPY services/tanren-api/src  services/tanren-api/src

# Layer 4: install workspace packages from source (fast)
RUN uv sync --frozen --no-cache --no-dev --no-editable --package tanren-api

# ── Runtime stage ────────────────────────────────────────────────────────────
FROM python:3.14-slim-bookworm

RUN groupadd --gid 10001 app \
 && useradd --uid 10001 --gid app --no-create-home --shell /usr/sbin/nologin app

COPY --from=build /app/.venv /app/.venv

ENV PATH="/app/.venv/bin:$PATH"

WORKDIR /app
RUN chown app:app /app

EXPOSE 8000

HEALTHCHECK --interval=10s --timeout=3s --start-period=5s --retries=3 \
  CMD python -c "from urllib.request import urlopen; urlopen('http://localhost:8000/api/v1/health/ready')"

USER app

ENTRYPOINT ["tanren-api"]

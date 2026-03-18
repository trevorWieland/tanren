ARG PYTHON_VERSION=3.14

FROM python:${PYTHON_VERSION}-slim AS base

# Extras to install (space-separated, e.g. "hetzner gcp" or "" for none)
ARG EXTRAS=""

COPY --from=ghcr.io/astral-sh/uv:latest /uv /uvx /usr/local/bin/

WORKDIR /app

# Copy workspace metadata first for layer caching
COPY pyproject.toml uv.lock ./
COPY packages/tanren-core/pyproject.toml packages/tanren-core/pyproject.toml
COPY services/tanren-api/pyproject.toml services/tanren-api/pyproject.toml
COPY services/tanren-cli/pyproject.toml services/tanren-cli/pyproject.toml
COPY services/tanren-daemon/pyproject.toml services/tanren-daemon/pyproject.toml

# Copy source
COPY packages/ packages/
COPY services/ services/

# Install runtime dependencies only (no dev group)
RUN uv sync --locked --no-dev --no-editable \
    $(for extra in $EXTRAS; do echo "--extra $extra"; done)

# Runtime
ENV PATH="/app/.venv/bin:$PATH"

EXPOSE 8000

CMD ["worker-manager"]

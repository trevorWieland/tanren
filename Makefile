.PHONY: install format format-check lint lint-check type unit integration docs-check check ci openapi clean \
	docker docker-slim docker-daemon docker-daemon-slim import-check alembic-check arch-check

install:
	uv sync --upgrade

format:
	uv run ruff format .
	uv run ruff check --fix .

format-check:
	uv run ruff format --check .

lint-check:
	uv run ruff check .

lint:
	uv run ruff check --fix .

type:
	uv run ty check

unit:
	uv run pytest tests/unit -q --tb=short --timeout=30 \
		--cov=packages --cov=services --cov-fail-under=80

integration:
	uv run pytest tests/integration -q --tb=short --timeout=30 \
		-m "not ssh and not local_env and not hetzner and not gcp and not postgres and not github and not linear and not docker" \
		--cov=packages --cov=services --cov-fail-under=75

docs-check:
	uv run python -m tanren_core.docs_links

import-check:
	uv run lint-imports

alembic-check:
	cd packages/tanren-core && uv run alembic upgrade head && uv run alembic check

arch-check: import-check alembic-check
	uv run python scripts/check_store_bypass.py
	uv run python scripts/check_thin_interfaces.py

check:
	$(MAKE) format-check
	$(MAKE) lint-check
	$(MAKE) type
	$(MAKE) arch-check
	$(MAKE) unit
	$(MAKE) integration
	$(MAKE) docs-check

ci:
	$(MAKE) check

openapi:
	uv run tanren-openapi

docker:
	docker build -f services/tanren-api/Dockerfile -t tanren-api:latest .

docker-slim:
	docker build -f services/tanren-api/Dockerfile --build-arg EXTRAS="" -t tanren-api:slim .

docker-daemon:
	docker build -f services/tanren-daemon/Dockerfile -t tanren-daemon:latest .

docker-daemon-slim:
	docker build -f services/tanren-daemon/Dockerfile --build-arg EXTRAS="" -t tanren-daemon:slim .

clean:
	find . -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true
	find . -type d -name .pytest_cache -exec rm -rf {} + 2>/dev/null || true
	find . -type d -name .ruff_cache -exec rm -rf {} + 2>/dev/null || true
	rm -f .*.log

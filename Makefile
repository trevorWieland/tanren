.PHONY: install format lint lint-check type unit integration docs-check check ci openapi clean

install:
	uv sync --upgrade

format:
	uv run ruff format .
	uv run ruff check --fix .

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
		-m "not ssh and not local_env" \
		--cov=packages --cov=services --cov-fail-under=75

docs-check:
	uv run python -m tanren_core.docs_links

check:
	$(MAKE) format
	$(MAKE) lint-check
	$(MAKE) type
	$(MAKE) unit
	$(MAKE) integration
	$(MAKE) docs-check

ci:
	$(MAKE) check

openapi:
	uv run tanren-openapi

clean:
	find . -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true
	find . -type d -name .pytest_cache -exec rm -rf {} + 2>/dev/null || true
	find . -type d -name .ruff_cache -exec rm -rf {} + 2>/dev/null || true
	rm -f .*.log

.PHONY: check docs-check lint format test ci all integration integration-ssh integration-local

check: docs-check lint typecheck test

docs-check:
	uv run python -m worker_manager.docs_links --repo-root ..

lint:
	uv run ruff check src/ tests/

typecheck:
	uv run ty check src/

format:
	uv run ruff format src/ tests/
	uv run ruff check --fix src/ tests/

test:
	uv run pytest tests/unit/ -x -q

integration:
	uv run pytest tests/integration/ -x -q --no-cov -m "not ssh and not local_env"

integration-ssh:
	uv run pytest tests/integration/ -x -q --no-cov -m ssh

integration-local:
	uv run pytest tests/integration/ -x -q --no-cov -m local_env

ci: check
	uv run pytest tests/integration/ -x -q --no-cov -m "not ssh and not local_env"

all: ci

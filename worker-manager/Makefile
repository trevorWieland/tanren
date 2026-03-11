.PHONY: check lint format test ci all integration integration-ssh integration-local

check: lint typecheck test

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
	uv run pytest tests/integration/ -x -q -m "not ssh and not local_env"

integration-ssh:
	uv run pytest tests/integration/ -x -q -m ssh

integration-local:
	uv run pytest tests/integration/ -x -q -m local_env

ci: check
	uv run pytest tests/integration/ -x -q -m "not ssh and not local_env"

all: ci

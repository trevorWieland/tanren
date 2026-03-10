.PHONY: check lint format test ci all

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

ci: check
	uv run pytest tests/integration/ -x -q

all: ci

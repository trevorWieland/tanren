# Three-Tier Test Structure

All tests live under `tests/unit`, `tests/integration`, or `tests/quality`. No exceptions.

```
tests/
├── unit/           # <250ms per test, mocks only, no external services
│   ├── core/
│   ├── cli/
│   └── tui/
├── integration/    # <5s per test, minimal mocks, real services, NO LLMs
│   ├── core/
│   ├── cli/
│   └── tui/
└── quality/        # <30s per test, minimal mocks, real services, REAL LLMs
    ├── core/
    └── cli/
```

**Tier definitions:**

**Unit tests** (`tests/unit/`):
- Fast: <250ms per test
- Mocks allowed and encouraged
- No external services (no network, no LLMs, no databases)
- Test isolated logic and algorithms

**Integration tests** (`tests/integration/`):
- Moderate speed: <5s per test
- Minimal mocks (only when unavoidable)
- Real services (storage, vector store, file system)
- **NO LLMs** - use mock model adapters for LLM calls
- BDD-style (Given/When/Then)

**Quality tests** (`tests/quality/`):
- Slower but bounded: <30s per test
- Minimal mocks (only when unavoidable)
- Real services (storage, vector store, file system)
- **REAL LLMs** - actual model calls, not mocked
- BDD-style (Given/When/Then)

**Package structure mirrors source:**
- `tests/unit/core/` tests `src/core/`
- `tests/integration/cli/` tests `src/cli/`
- Mirror your source package layout

**Never place tests:**
- Outside the three tier folders
- In source code directories
- In ad-hoc locations (scripts, benchmarks, etc.)

**CI execution:**
- Unit tests: Run on every PR, fast feedback
- Integration tests: Run on every PR or schedule
- Quality tests: Run on schedule or manual trigger (slower)

**Why:** Clear purpose and scope for each test tier, and enables selective execution by tier (unit fast/run frequently, integration/quality slower/run selectively).

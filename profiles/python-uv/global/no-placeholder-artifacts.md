# No Placeholder Artifacts

Never commit placeholder values. All artifacts must be functional and verified at the time of commit.

## For implementers

- **Hashes must be real** — Compute from actual file contents. Never commit empty-file hashes (`e3b0c44298fc...`)
- **Tests must execute** — Never mark `pytest.mark.skip` tests as "complete". Tests either run and pass or don't exist yet.
- **Values must be derived** — If a value should be computed (rankings from Elo, IDs from content), implement the derivation. Don't accept caller-supplied placeholders.
- **Paths must resolve** — File paths, URLs, and resource references must point to real, accessible resources

## For auditors

- **Check for sentinel values** — `e3b0c44298fc...` (empty SHA-256), `TODO`, `FIXME`, `placeholder`, `dummy`
- **Run skipped tests** — If tests exist with `skip` markers, they are incomplete work, not completed work
- **Verify derivation** — If a function accepts a value that should be computed internally, check that it's actually computed
- **Execute, don't just read** — `make check` catches what code review misses

## Anti-patterns

```python
# BAD: placeholder hash committed as real
{"hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"}

# BAD: skipped test counted as coverage
@pytest.mark.skip(reason="will implement later")
def test_feature(): ...

# BAD: caller-supplied value instead of derived
def build_report(overall_ranking: list[str]):  # should derive from Elo
```

## Why

Benchmark-harness Tasks 3, 5, and 6 each passed initial audits with placeholder values. Each required additional audit rounds to catch and fix. Placeholder artifacts create the illusion of progress while masking incomplete work.

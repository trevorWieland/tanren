# Tanren Behavior Features

Active behavior proof lives under `tests/bdd/features`.

Every active scenario must reference exactly one asserted `B-XXXX` behavior ID
from `docs/behaviors` and exactly one witness tag: `@positive` or
`@falsification`.

Phase names, wave names, proof IDs such as `BEH-P0-*`, and skipped or pending
scenario tags are not allowed in the active behavior suite.

`just tests` runs inventory validation, the BDD suite, and behavior coverage.
Mutation is intentionally separated into `just mutation` and scheduled CI
because full product mutation is too expensive for every PR gate.

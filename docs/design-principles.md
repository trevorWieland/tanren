# Design Principles

1. Opinionated workflow, pluggable integrations.
2. Execution framework and methodology system are both mandatory.
3. Remote environments are untrusted sandboxes.
4. Configuration is resolved locally before remote execution.
5. Strict typing and explicit protocol boundaries.
6. No hardcoded provider-specific values in core logic.
7. Provider-agnostic interfaces, provider-specific adapters.
8. Optional dependencies for non-core providers.
9. Secrets must never leak or persist outside intended scope.
10. New projects should be bootstrappable with a repeatable documented flow.

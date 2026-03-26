"""Hierarchical scope matching for permission-based auth."""

from __future__ import annotations

# ── Known scopes ─────────────────────────────────────────────────────────

VALID_SCOPES: frozenset[str] = frozenset({
    # Dispatch
    "dispatch:create",
    "dispatch:read",
    "dispatch:cancel",
    # VM
    "vm:provision",
    "vm:read",
    "vm:release",
    # Run (multi-step)
    "run:provision",
    "run:execute",
    "run:teardown",
    "run:full",
    "run:read",
    # Read-only endpoints
    "events:read",
    "config:read",
    "metrics:read",
    # Admin
    "admin:keys",
    "admin:users",
})

# Convenience bundles — NOT checked in code, only used as defaults when
# creating keys.  Auth always checks actual scopes on the key.
DEFAULT_SCOPES: dict[str, list[str]] = {
    "admin": ["*"],
    "developer": [
        "dispatch:*",
        "vm:*",
        "run:*",
        "events:read",
        "config:read",
        "metrics:read",
    ],
    "readonly": [
        "dispatch:read",
        "vm:read",
        "run:read",
        "events:read",
        "config:read",
        "metrics:read",
    ],
}


# ── Matching ─────────────────────────────────────────────────────────────


def has_scope(granted: frozenset[str], required: str) -> bool:
    """Check if *granted* scopes satisfy *required*.

    Supports wildcards:
    - ``*`` matches everything.
    - ``dispatch:*`` matches ``dispatch:create``, ``dispatch:read``, etc.
    """
    if "*" in granted:
        return True
    if required in granted:
        return True
    # Check namespace wildcard
    ns = required.split(":", maxsplit=1)[0]
    return f"{ns}:*" in granted


def validate_scopes(scopes: list[str]) -> list[str]:
    """Validate that all scopes are recognised (or valid wildcards).

    Returns:
        The input list unchanged.

    Raises:
        ValueError: If any scope is unrecognised.
    """
    for s in scopes:
        if s == "*":
            continue
        if s.endswith(":*"):
            ns = s.split(":")[0]
            if not any(v.startswith(f"{ns}:") for v in VALID_SCOPES):
                msg = f"Unknown scope namespace: {s}"
                raise ValueError(msg)
            continue
        if s not in VALID_SCOPES:
            msg = f"Unknown scope: {s}"
            raise ValueError(msg)
    return scopes

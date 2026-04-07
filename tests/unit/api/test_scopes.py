"""Tests for scope matching and validation logic."""

from __future__ import annotations

import pytest

from tanren_api.scopes import VALID_SCOPES, has_scope, validate_scopes


class TestHasScope:
    """Test the has_scope matching function."""

    def test_exact_match(self) -> None:
        granted = frozenset({"dispatch:create", "vm:read"})
        assert has_scope(granted, "dispatch:create") is True

    def test_exact_match_different_scope(self) -> None:
        granted = frozenset({"dispatch:create", "vm:read"})
        assert has_scope(granted, "vm:read") is True

    def test_wildcard_matches_everything(self) -> None:
        granted = frozenset({"*"})
        assert has_scope(granted, "dispatch:create") is True
        assert has_scope(granted, "admin:users") is True
        assert has_scope(granted, "vm:provision") is True

    def test_namespace_wildcard(self) -> None:
        granted = frozenset({"dispatch:*"})
        assert has_scope(granted, "dispatch:create") is True
        assert has_scope(granted, "dispatch:read") is True
        assert has_scope(granted, "dispatch:cancel") is True

    def test_namespace_wildcard_does_not_cross_namespaces(self) -> None:
        granted = frozenset({"dispatch:*"})
        assert has_scope(granted, "vm:read") is False
        assert has_scope(granted, "admin:keys") is False

    def test_mismatch_returns_false(self) -> None:
        granted = frozenset({"dispatch:create"})
        assert has_scope(granted, "dispatch:read") is False
        assert has_scope(granted, "vm:read") is False
        assert has_scope(granted, "admin:users") is False

    def test_empty_granted_returns_false(self) -> None:
        granted: frozenset[str] = frozenset()
        assert has_scope(granted, "dispatch:create") is False

    def test_multiple_namespace_wildcards(self) -> None:
        granted = frozenset({"dispatch:*", "vm:*"})
        assert has_scope(granted, "dispatch:create") is True
        assert has_scope(granted, "vm:provision") is True
        assert has_scope(granted, "admin:users") is False


class TestValidateScopes:
    """Test the validate_scopes validation function."""

    def test_valid_scopes_accepted(self) -> None:
        scopes = ["dispatch:create", "vm:read", "events:read"]
        result = validate_scopes(scopes)
        assert result == scopes

    def test_wildcard_accepted(self) -> None:
        result = validate_scopes(["*"])
        assert result == ["*"]

    def test_namespace_wildcard_accepted(self) -> None:
        result = validate_scopes(["dispatch:*"])
        assert result == ["dispatch:*"]

    def test_multiple_namespace_wildcards_accepted(self) -> None:
        result = validate_scopes(["dispatch:*", "vm:*", "run:*"])
        assert result == ["dispatch:*", "vm:*", "run:*"]

    def test_invalid_scope_rejected(self) -> None:
        with pytest.raises(ValueError, match="Unknown scope"):
            validate_scopes(["nonexistent:scope"])

    def test_invalid_namespace_wildcard_rejected(self) -> None:
        with pytest.raises(ValueError, match="Unknown scope namespace"):
            validate_scopes(["bogus:*"])

    def test_mixed_valid_and_invalid_rejected(self) -> None:
        with pytest.raises(ValueError, match="Unknown scope"):
            validate_scopes(["dispatch:create", "totally:invalid"])

    def test_empty_list_accepted(self) -> None:
        result = validate_scopes([])
        assert result == []

    def test_all_valid_scopes_accepted(self) -> None:
        result = validate_scopes(list(VALID_SCOPES))
        assert set(result) == VALID_SCOPES

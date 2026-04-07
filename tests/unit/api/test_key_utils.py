"""Tests for API key generation and hashing utilities."""

from __future__ import annotations

import hashlib

from tanren_api.key_utils import generate_api_key, hash_api_key


class TestGenerateApiKey:
    """Test the generate_api_key function."""

    def test_returns_three_part_tuple(self) -> None:
        full_key, prefix, key_hash = generate_api_key()
        assert isinstance(full_key, str)
        assert isinstance(prefix, str)
        assert isinstance(key_hash, str)

    def test_full_key_has_tnrn_prefix_format(self) -> None:
        full_key, prefix, _key_hash = generate_api_key()
        assert full_key.startswith("tnrn_")
        parts = full_key.split("_", maxsplit=2)
        assert len(parts) == 3
        assert parts[0] == "tnrn"
        assert parts[1] == prefix

    def test_prefix_is_8_hex_chars(self) -> None:
        _full_key, prefix, _key_hash = generate_api_key()
        assert len(prefix) == 8
        # Validate it's valid hex
        int(prefix, 16)

    def test_hash_is_deterministic_for_key(self) -> None:
        full_key, _prefix, key_hash = generate_api_key()
        # Re-hashing the same key should produce the same hash
        assert hash_api_key(full_key) == key_hash

    def test_keys_are_unique(self) -> None:
        keys = {generate_api_key()[0] for _ in range(10)}
        assert len(keys) == 10

    def test_hashes_are_unique(self) -> None:
        hashes = {generate_api_key()[2] for _ in range(10)}
        assert len(hashes) == 10


class TestHashApiKey:
    """Test the hash_api_key function."""

    def test_returns_sha256_hex_digest(self) -> None:
        key = "tnrn_abcd1234_somesecretdata"
        result = hash_api_key(key)
        expected = hashlib.sha256(key.encode()).hexdigest()
        assert result == expected

    def test_hash_length_is_64(self) -> None:
        result = hash_api_key("any-key-value")
        assert len(result) == 64

    def test_deterministic(self) -> None:
        key = "tnrn_test1234_mytestkey"
        assert hash_api_key(key) == hash_api_key(key)

    def test_different_keys_produce_different_hashes(self) -> None:
        h1 = hash_api_key("key-a")
        h2 = hash_api_key("key-b")
        assert h1 != h2

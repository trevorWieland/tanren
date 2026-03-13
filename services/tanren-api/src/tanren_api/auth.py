"""API authentication via API key header."""

import secrets
from typing import Annotated, Protocol

from fastapi import Header, Request

from tanren_api.errors import AuthenticationError
from tanren_api.settings import APISettings


class AuthVerifier(Protocol):
    """Protocol for pluggable auth verification."""

    async def verify(self, credentials: str) -> None:
        """Verify the provided credentials."""
        ...


class APIKeyVerifier:
    """Verify requests via a static API key."""

    def __init__(self, expected_key: str) -> None:
        """Initialize with the expected API key."""
        self._expected_key = expected_key

    async def verify(self, credentials: str) -> None:
        """Raise AuthenticationError if key doesn't match.

        Raises:
            AuthenticationError: If the credentials are invalid or missing.
        """
        if not self._expected_key or not secrets.compare_digest(credentials, self._expected_key):
            raise AuthenticationError("Invalid API key")


async def verify_api_key(
    request: Request,
    x_api_key: Annotated[str, Header()],
) -> str:
    """FastAPI dependency that validates the X-API-Key header.

    Returns:
        The validated API key string.
    """
    settings: APISettings = request.app.state.settings
    verifier = APIKeyVerifier(settings.api_key)
    await verifier.verify(x_api_key)
    return x_api_key

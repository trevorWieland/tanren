"""API server configuration.

``APIConfig`` carries only what the stateless API needs: a database URL,
an API key, and HTTP server settings.  No filesystem paths, no CLI binary
paths, no execution environment configuration.
"""

from __future__ import annotations

from pydantic import Field
from pydantic_settings import BaseSettings, SettingsConfigDict


class APIConfig(BaseSettings):
    """Configuration for the tanren API service.

    Loaded from ``TANREN_API_*`` environment variables or a ``.env`` file.
    The API has no filesystem access to project repos — profile resolution
    is the caller's responsibility.
    """

    model_config = SettingsConfigDict(
        env_prefix="TANREN_API_",
        env_file=".env",
        extra="ignore",
    )

    # Storage — required (the API is always backed by a DB)
    db_url: str = Field(
        default="",
        description="SQLite path or postgresql:// URL for the unified store",
    )

    # Auth
    api_key: str = Field(
        default="",
        description="API key for request authentication",
    )

    # Server
    host: str = Field(
        default="0.0.0.0",  # noqa: S104
        description="Bind address",
    )
    port: int = Field(
        default=8000,
        ge=1,
        le=65535,
        description="Bind port",
    )

    # CORS
    cors_origins: list[str] = Field(
        default_factory=list,
        description="Allowed CORS origins",
    )

    # Logging
    log_level: str = Field(
        default="info",
        description="Log level",
    )

"""API settings loaded from environment variables."""

from pydantic import model_validator
from pydantic_settings import BaseSettings, SettingsConfigDict


class APISettings(BaseSettings):
    """Configuration for the tanren API service."""

    model_config = SettingsConfigDict(env_prefix="TANREN_API_", env_file=".env")

    host: str = "0.0.0.0"  # noqa: S104 — binding to all interfaces is intentional for container deployment
    port: int = 8000
    api_key: str = ""
    workers: int = 1
    log_level: str = "info"
    cors_origins: list[str] = []
    events_db: str | None = None

    @model_validator(mode="after")
    def _check_single_worker(self) -> APISettings:
        if self.workers > 1:
            raise ValueError(f"workers must be 1 with in-memory state store; got {self.workers}")
        return self

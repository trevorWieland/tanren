"""Pydantic models for the tanren.yml env block."""

from enum import StrEnum

from pydantic import BaseModel, Field


class OnMissing(StrEnum):
    ERROR = "error"
    WARN = "warn"
    PROMPT = "prompt"


class RequiredEnvVar(BaseModel):
    key: str
    description: str = ""
    pattern: str | None = None  # regex for re.fullmatch()
    hint: str = ""


class OptionalEnvVar(BaseModel):
    key: str
    description: str = ""
    pattern: str | None = None
    default: str | None = None


class EnvBlock(BaseModel):
    on_missing: OnMissing = OnMissing.ERROR
    required: list[RequiredEnvVar] = Field(default_factory=list)
    optional: list[OptionalEnvVar] = Field(default_factory=list)


class TanrenConfig(BaseModel):
    version: str
    profile: str
    installed: str  # YAML may parse dates; coerce via validator
    env: EnvBlock | None = None

    @classmethod
    def model_validate(cls, obj, *args, **kwargs):
        # YAML parses bare dates (2026-01-01) as datetime.date objects
        if isinstance(obj, dict) and "installed" in obj:
            obj = {**obj, "installed": str(obj["installed"])}
        return super().model_validate(obj, *args, **kwargs)

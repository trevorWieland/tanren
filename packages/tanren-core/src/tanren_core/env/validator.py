"""Validate required/optional env vars against loaded layers."""

from __future__ import annotations

import os
import re
from enum import StrEnum
from typing import TYPE_CHECKING

from pydantic import BaseModel, ConfigDict, Field

from tanren_core.env.loader import resolve_env_var

if TYPE_CHECKING:
    from tanren_core.adapters.protocols import SecretProvider
    from tanren_core.env.schema import EnvBlock


class VarStatus(StrEnum):
    """Status of a validated environment variable."""

    PASS = "pass"  # noqa: S105 — enum value name, not a real password
    MISSING = "missing"
    EMPTY = "empty"
    PATTERN_MISMATCH = "pattern_mismatch"
    DEFAULTED = "defaulted"


class VarResult(BaseModel):
    """Validation result for one environment variable."""

    model_config = ConfigDict(extra="forbid")

    key: str = Field(..., description="Environment variable name")
    status: VarStatus = Field(..., description="Validation outcome for this variable")
    description: str = Field(default="", description="Human-readable purpose of this variable")
    hint: str = Field(default="", description="User-facing hint for how to set this variable")
    source: str | None = Field(default=None, description="Where the value was resolved from")
    message: str = Field(default="", description="Validation detail or error message")


class EnvReport(BaseModel):
    """Validation report for all required/optional environment variables."""

    model_config = ConfigDict(extra="forbid")

    passed: bool = Field(..., description="Whether all required variables passed validation")
    required_results: list[VarResult] = Field(
        default_factory=list, description="Validation results for required variables"
    )
    optional_results: list[VarResult] = Field(
        default_factory=list, description="Validation results for optional variables"
    )
    warnings: list[str] = Field(default_factory=list, description="Non-fatal validation warnings")


async def _resolve_secret(
    key: str,
    source: str | None,
    merged_env: dict[str, str],
    source_map: dict[str, str],
    secret_provider: SecretProvider | None,
) -> None:
    """Resolve a secret: source into merged_env if the var is not already set."""
    if not source or not source.startswith("secret:") or secret_provider is None:
        return
    # Only fetch from provider if not already resolved from a higher-priority source
    if key in merged_env or os.environ.get(key) is not None:
        return
    secret_name = source[len("secret:") :]
    fetched = await secret_provider.get_secret(secret_name)
    if fetched is not None:
        merged_env[key] = fetched
        source_map[key] = f"secret:{secret_name}"


async def validate_env(
    env_block: EnvBlock,
    merged_env: dict[str, str],
    source_map: dict[str, str],
    *,
    secret_provider: SecretProvider | None = None,
) -> EnvReport:
    """Validate env vars against schema.

    Required vars must exist and be non-empty. If pattern defined,
    re.search(pattern, value) must succeed.

    Optional vars: if absent, inject default into merged env (status=DEFAULTED).
    If present + pattern, validate.

    When a secret_provider is given, vars with ``source: "secret:X"`` are
    resolved from the provider before checking the normal env layers.
    Priority: os.environ > dotenv layers > secret provider.

    Never logs full secret values -- redacted to first 4 chars + '...'.

    Returns:
        EnvReport with validation results for all vars.
    """
    required_results: list[VarResult] = []
    optional_results: list[VarResult] = []
    warnings: list[str] = []
    all_pass = True

    for var in env_block.required:
        # Resolve from secret provider if source declared and var not already set
        await _resolve_secret(var.key, var.source, merged_env, source_map, secret_provider)

        value, source = resolve_env_var(var.key, merged_env, source_map)

        if value is None:
            required_results.append(
                VarResult(
                    key=var.key,
                    status=VarStatus.MISSING,
                    description=var.description,
                    hint=var.hint,
                    message="Required variable is not set",
                )
            )
            all_pass = False
            continue

        if not value:
            required_results.append(
                VarResult(
                    key=var.key,
                    status=VarStatus.EMPTY,
                    description=var.description,
                    hint=var.hint,
                    source=source,
                    message="Required variable is empty",
                )
            )
            all_pass = False
            continue

        if var.pattern and not re.search(var.pattern, value):
            redacted = _redact(value)
            required_results.append(
                VarResult(
                    key=var.key,
                    status=VarStatus.PATTERN_MISMATCH,
                    description=var.description,
                    hint=var.hint,
                    source=source,
                    message=f"Value '{redacted}' does not match pattern '{var.pattern}'",
                )
            )
            all_pass = False
            continue

        required_results.append(
            VarResult(
                key=var.key,
                status=VarStatus.PASS,
                description=var.description,
                hint=var.hint,
                source=source,
            )
        )

    for var in env_block.optional:
        # Resolve from secret provider if source declared and var not already set
        await _resolve_secret(var.key, var.source, merged_env, source_map, secret_provider)

        value, source = resolve_env_var(var.key, merged_env, source_map)

        if value is None:
            if var.default is not None:
                merged_env[var.key] = var.default
                source_map[var.key] = "default"
                optional_results.append(
                    VarResult(
                        key=var.key,
                        status=VarStatus.DEFAULTED,
                        description=var.description,
                        source="default",
                        message=f"Using default: {var.default}",
                    )
                )
            else:
                optional_results.append(
                    VarResult(
                        key=var.key,
                        status=VarStatus.MISSING,
                        description=var.description,
                        message="Optional variable is not set (no default)",
                    )
                )
            continue

        if value and var.pattern and not re.search(var.pattern, value):
            redacted = _redact(value)
            optional_results.append(
                VarResult(
                    key=var.key,
                    status=VarStatus.PATTERN_MISMATCH,
                    description=var.description,
                    source=source,
                    message=f"Value '{redacted}' does not match pattern '{var.pattern}'",
                )
            )
            warnings.append(
                f"Optional var {var.key} has value not matching pattern '{var.pattern}'"
            )
            continue

        optional_results.append(
            VarResult(
                key=var.key,
                status=VarStatus.PASS,
                description=var.description,
                source=source,
            )
        )

    return EnvReport(
        passed=all_pass,
        required_results=required_results,
        optional_results=optional_results,
        warnings=warnings,
    )


def _redact(value: str) -> str:
    """Redact a value for safe logging: first 4 chars + '...'.

    Returns:
        Redacted string.
    """
    if len(value) < 6:
        return "****"
    return value[:4] + "..."

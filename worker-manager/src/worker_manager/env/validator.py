"""Validate required/optional env vars against loaded layers."""

import re
from dataclasses import dataclass, field
from enum import StrEnum

from worker_manager.env.loader import resolve_env_var
from worker_manager.env.schema import EnvBlock


class VarStatus(StrEnum):
    PASS = "pass"
    MISSING = "missing"
    EMPTY = "empty"
    PATTERN_MISMATCH = "pattern_mismatch"
    DEFAULTED = "defaulted"


@dataclass
class VarResult:
    key: str
    status: VarStatus
    description: str = ""
    hint: str = ""
    source: str | None = None
    message: str = ""


@dataclass
class EnvReport:
    passed: bool
    required_results: list[VarResult] = field(default_factory=list)
    optional_results: list[VarResult] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)


def validate_env(
    env_block: EnvBlock,
    merged_env: dict[str, str],
    source_map: dict[str, str],
) -> EnvReport:
    """Validate env vars against schema.

    Required vars must exist and be non-empty. If pattern defined,
    re.search(pattern, value) must succeed.

    Optional vars: if absent, inject default into merged env (status=DEFAULTED).
    If present + pattern, validate.

    Never logs full secret values — redacted to first 4 chars + '...'.
    """
    required_results: list[VarResult] = []
    optional_results: list[VarResult] = []
    warnings: list[str] = []
    all_pass = True

    for var in env_block.required:
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
    """Redact a value for safe logging: first 4 chars + '...'."""
    if len(value) < 6:
        return "****"
    return value[:4] + "..."

"""Classify agent errors as transient, fatal, or ambiguous for retry decisions."""

import enum
import re


class ErrorClass(enum.Enum):
    TRANSIENT = "transient"
    FATAL = "fatal"
    AMBIGUOUS = "ambiguous"


TRANSIENT_PATTERNS = [
    r"rate.?limit",
    r"\b429\b",
    r"connection refused",
    r"ECONNRESET",
    r"ETIMEDOUT",
    r"timeout",
    r"service unavailable",
    r"\b503\b",
    r"server error",
    r"\b500\b",
]

FATAL_PATTERNS = [
    r"authentication_error",
    r"\b401\b",
    r"permission denied",
    r"\b403\b",
    r"command not found",
    r"No such file or directory",
]

_TRANSIENT_RE = re.compile("|".join(TRANSIENT_PATTERNS), re.IGNORECASE)
_FATAL_RE = re.compile("|".join(FATAL_PATTERNS), re.IGNORECASE)
TRANSIENT_BACKOFF = (10, 30, 60)  # seconds between retries


def classify_error(
    exit_code: int,
    stdout: str,
    stderr: str,
    signal_value: str | None,
) -> ErrorClass:
    combined = f"{stdout}\n{stderr}"

    # Agent explicitly signaled error → fatal
    if signal_value == "error":
        return ErrorClass.FATAL

    # Exit 137 = OOM/SIGKILL → transient (may succeed on retry)
    if exit_code == 137:
        return ErrorClass.TRANSIENT

    # Pattern matching on output
    if _TRANSIENT_RE.search(combined):
        return ErrorClass.TRANSIENT
    if _FATAL_RE.search(combined):
        return ErrorClass.FATAL

    return ErrorClass.AMBIGUOUS

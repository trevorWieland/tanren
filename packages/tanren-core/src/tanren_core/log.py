"""Structured logging with phase banners and [tanren] prefix."""

import logging
import os
import sys

PREFIX = "[tanren]"


def supports_color() -> bool:
    """Check if stderr supports color output.

    Returns:
        True if stderr supports ANSI color.
    """
    if os.environ.get("NO_COLOR"):
        return False
    return hasattr(sys.stderr, "isatty") and sys.stderr.isatty()


class TanrenFormatter(logging.Formatter):
    """Log formatter with [tanren] prefix."""

    def format(self, record: logging.LogRecord) -> str:
        """Format a log record with timestamp, level, name, and [tanren] prefix.

        Returns:
            Formatted log string.
        """
        ts = self.formatTime(record, "%Y-%m-%d %H:%M:%S")
        return f"{PREFIX} {ts} {record.levelname} {record.name}: {record.getMessage()}"


def configure_logging(level: str = "INFO") -> None:
    """Configure logging with [tanren] prefix format.

    Reads TANREN_LOG_LEVEL env var as override.
    """
    env_level = os.environ.get("TANREN_LOG_LEVEL", level).upper()

    handler = logging.StreamHandler(sys.stderr)
    handler.setFormatter(TanrenFormatter())

    root = logging.getLogger()
    root.handlers.clear()
    root.addHandler(handler)
    root.setLevel(getattr(logging, env_level, logging.INFO))


def phase_banner(phase: str, project: str | None = None, spec: str | None = None) -> None:
    """Log a phase banner line."""
    logger = logging.getLogger("tanren")
    bar = "\u2501" * 40
    parts = [phase.upper()]
    if project:
        parts.append(project)
    if spec:
        parts.append(spec)
    label = " ".join(parts)
    logger.info("\u2501\u2501\u2501 %s %s", label, bar[: 40 - len(label)])


def phase_complete(phase: str, duration_secs: float, outcome: str | None = None) -> None:
    """Log a phase completion line."""
    logger = logging.getLogger("tanren")
    bar = "\u2501" * 40
    parts = [f"{phase.upper()} COMPLETE ({duration_secs:.1f}s)"]
    if outcome:
        parts.append(outcome)
    label = " ".join(parts)
    logger.info("\u2501\u2501\u2501 %s %s", label, bar[: 40 - len(label)])

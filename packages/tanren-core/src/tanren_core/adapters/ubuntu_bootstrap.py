"""Ubuntu VM bootstrapper — installs development tools and marks completion."""

from __future__ import annotations

import logging
import time
from typing import TYPE_CHECKING

from pydantic import BaseModel, ConfigDict, Field

from tanren_core.adapters.remote_types import BootstrapResult
from tanren_core.schemas import Cli

if TYPE_CHECKING:
    from tanren_core.adapters.protocols import RemoteConnection

logger = logging.getLogger(__name__)

_APT_PACKAGES = ("git", "curl", "build-essential", "jq")

_NODE_INSTALL = (
    "curl -fsSL https://deb.nodesource.com/setup_22.x | bash - && apt-get install -y nodejs"
)

# Infrastructure steps — always installed regardless of required CLIs.
_INFRA_STEPS: tuple[tuple[str, str, str], ...] = (
    ("docker", "command -v docker", "curl -fsSL https://get.docker.com | sh"),
    ("node", "command -v node", _NODE_INSTALL),
    (
        "uv",
        "command -v uv",
        "curl -LsSf https://astral.sh/uv/install.sh | sh"
        " && cp $HOME/.local/bin/uv /usr/local/bin/uv",
    ),
)

# Per-CLI install steps — only installed when the CLI is in required_clis.
_CLI_STEPS: dict[Cli, tuple[str, str, str]] = {
    Cli.CLAUDE: ("claude", "command -v claude", "npm install -g @anthropic-ai/claude-code"),
    Cli.OPENCODE: (
        "opencode",
        "command -v opencode",
        "curl -fsSL https://opencode.ai/install | bash"
        " && cp $HOME/.opencode/bin/opencode /usr/local/bin/opencode",
    ),
    Cli.CODEX: ("codex", "command -v codex", "npm install -g @openai/codex"),
}

# Per-CLI ccusage packages.
_CCUSAGE_PACKAGES: dict[Cli, str] = {
    Cli.CLAUDE: "ccusage",
    Cli.CODEX: "@ccusage/codex",
    Cli.OPENCODE: "@ccusage/opencode",
}

_AGENT_USER = "tanren"

_MARKER_PATH = "~/.tanren-bootstrapped"

# Backward-compatible alias: flat list of all steps for tests that import it.
_BOOTSTRAP_STEPS: tuple[tuple[str, str, str], ...] = (
    *_INFRA_STEPS,
    *(_CLI_STEPS[cli] for cli in (Cli.CLAUDE, Cli.OPENCODE, Cli.CODEX)),
)


def _build_ccusage_step(required_clis: frozenset[Cli]) -> tuple[str, str, str]:
    """Build the ccusage install step for the given CLI set.

    Returns:
        Tuple of (name, check_command, install_command).
    """
    packages = [
        _CCUSAGE_PACKAGES[cli]
        for cli in sorted(required_clis, key=lambda c: c.value)
        if cli in _CCUSAGE_PACKAGES
    ]
    if not packages:
        packages = ["ccusage"]
    check_parts = [f"npx {pkg} --version" for pkg in packages]
    return ("ccusage", " && ".join(check_parts), f"npm install -g {' '.join(packages)}")


class BootstrapInstallStep(BaseModel):
    """One conditional bootstrap installation step."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    name: str = Field(...)
    check_command: str = Field(...)
    install_command: str = Field(...)


class BootstrapPlan(BaseModel):
    """Public bootstrap plan metadata for dry-run/introspection."""

    model_config = ConfigDict(extra="forbid", frozen=True)

    apt_packages: tuple[str, ...] = Field(default_factory=tuple)
    install_steps: tuple[BootstrapInstallStep, ...] = Field(default_factory=tuple)


class UbuntuBootstrapper:
    """Bootstraps an Ubuntu VM with development tools.

    Implements the EnvironmentBootstrapper protocol. Idempotent — checks
    for existing installations before running install commands. Writes
    a marker file on completion.
    """

    def __init__(
        self,
        *,
        required_clis: frozenset[Cli],
        extra_script: str | None = None,
    ) -> None:
        """Initialize with required CLIs and an optional extra bootstrap script."""
        self._required_clis = required_clis
        self._extra_script = extra_script

    def _build_steps(self) -> tuple[tuple[str, str, str], ...]:
        """Build the full step list from infra + required CLI steps + ccusage.

        Returns:
            Tuple of (name, check_command, install_command) for each step.
        """
        cli_steps = tuple(
            _CLI_STEPS[cli]
            for cli in sorted(self._required_clis, key=lambda c: c.value)
            if cli in _CLI_STEPS
        )
        ccusage_step = _build_ccusage_step(self._required_clis)
        return (*_INFRA_STEPS, *cli_steps, ccusage_step)

    def plan(self) -> BootstrapPlan:
        """Return the bootstrap plan used by this bootstrapper."""
        steps = self._build_steps()
        return BootstrapPlan(
            apt_packages=_APT_PACKAGES,
            install_steps=tuple(
                BootstrapInstallStep(
                    name=name,
                    check_command=check_cmd,
                    install_command=install_cmd,
                )
                for name, check_cmd, install_cmd in steps
            ),
        )

    async def bootstrap(self, conn: RemoteConnection, *, force: bool = False) -> BootstrapResult:
        """Install development tools on the remote VM.

        Args:
            conn: RemoteConnection to the VM.
            force: If True, re-run all steps even if marker exists.

        Returns:
            BootstrapResult with installed/skipped tools and duration.

        Raises:
            RuntimeError: If apt install, a tool install, or the extra
                bootstrap script fails.
        """
        start = time.monotonic()
        installed: list[str] = []
        skipped: list[str] = []

        if not force:
            marker_check = await conn.run(f"test -f {_MARKER_PATH} && echo exists", timeout=10)
            if "exists" in marker_check.stdout:
                logger.info("VM already bootstrapped (marker exists)")
                return BootstrapResult(duration_secs=0)

        # Step 1: apt packages
        apt_result = await conn.run(
            f"apt-get update -qq && apt-get install -y -qq {' '.join(_APT_PACKAGES)}",
            timeout=300,
        )
        if apt_result.exit_code != 0:
            raise RuntimeError(f"apt install failed: {apt_result.stderr}")
        installed.append("apt-packages")

        # Step 2+: Infra + CLI + ccusage steps
        steps = self._build_steps()
        for name, check_cmd, install_cmd in steps:
            check = await conn.run(check_cmd, timeout=10)
            if check.exit_code == 0 and not force:
                skipped.append(name)
                logger.info("Skipping %s (already installed)", name)
                continue

            logger.info("Installing %s...", name)
            result = await conn.run(install_cmd, timeout=600)
            if result.exit_code != 0:
                raise RuntimeError(f"Failed to install {name}: {result.stderr}")
            installed.append(name)

        # Create workspace directory and agent user
        useradd_cmd = (
            f"id -u {_AGENT_USER} >/dev/null 2>&1"
            f" || useradd --create-home --shell /bin/bash {_AGENT_USER}"
        )
        useradd_result = await conn.run(useradd_cmd, timeout=30)
        if useradd_result.exit_code != 0:
            raise RuntimeError(f"Agent user setup failed: {useradd_result.stderr}")
        workspace_result = await conn.run(
            f"mkdir -p /workspace && chown {_AGENT_USER}:{_AGENT_USER} /workspace",
            timeout=10,
        )
        if workspace_result.exit_code != 0:
            raise RuntimeError(f"Workspace directory setup failed: {workspace_result.stderr}")

        # Extra script (if configured)
        if self._extra_script is not None:
            logger.info("Running extra bootstrap script...")
            await conn.upload_content(self._extra_script, "/tmp/tanren-extra-bootstrap.sh")
            extra_result = await conn.run("bash /tmp/tanren-extra-bootstrap.sh", timeout=600)
            if extra_result.exit_code != 0:
                raise RuntimeError(f"Extra bootstrap script failed: {extra_result.stderr}")
            installed.append("extra-script")

        # Write marker
        await conn.run(f"touch {_MARKER_PATH}", timeout=10)

        duration = int(time.monotonic() - start)
        return BootstrapResult(
            installed=tuple(installed),
            skipped=tuple(skipped),
            duration_secs=duration,
        )

    async def is_bootstrapped(self, conn: RemoteConnection) -> bool:
        """Check if the VM has been bootstrapped.

        Returns:
            True if the bootstrap marker file exists.
        """
        result = await conn.run(f"test -f {_MARKER_PATH} && echo exists", timeout=10)
        return "exists" in result.stdout

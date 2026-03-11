"""Ubuntu VM bootstrapper — installs development tools and marks completion."""

from __future__ import annotations

import logging
import time

from worker_manager.adapters.remote_types import BootstrapResult

logger = logging.getLogger(__name__)

_APT_PACKAGES = ("git", "curl", "build-essential", "jq")

_NODE_INSTALL = (
    "curl -fsSL https://deb.nodesource.com/setup_22.x | bash - && apt-get install -y nodejs"
)

_BOOTSTRAP_STEPS: tuple[tuple[str, str, str], ...] = (
    ("docker", "command -v docker", "curl -fsSL https://get.docker.com | sh"),
    ("node", "command -v node", _NODE_INSTALL),
    ("uv", "command -v uv", "curl -LsSf https://astral.sh/uv/install.sh | sh"),
    ("claude", "command -v claude", "npm install -g @anthropic-ai/claude-code"),
)

_MARKER_PATH = "~/.tanren-bootstrapped"


class UbuntuBootstrapper:
    """Bootstraps an Ubuntu VM with development tools.

    Implements the EnvironmentBootstrapper protocol. Idempotent — checks
    for existing installations before running install commands. Writes
    a marker file on completion.
    """

    def __init__(self, *, extra_script: str | None = None) -> None:
        self._extra_script = extra_script

    async def bootstrap(self, conn, *, force: bool = False) -> BootstrapResult:
        """Install development tools on the remote VM.

        Args:
            conn: RemoteConnection to the VM.
            force: If True, re-run all steps even if marker exists.

        Returns:
            BootstrapResult with installed/skipped tools and duration.
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

        # Step 2-5: Individual tools
        for name, check_cmd, install_cmd in _BOOTSTRAP_STEPS:
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

        # Step 6: Create workspace directory
        await conn.run("mkdir -p /workspace", timeout=10)

        # Step 7: Extra script (if configured)
        if self._extra_script is not None:
            logger.info("Running extra bootstrap script...")
            await conn.upload_content(self._extra_script, "/tmp/tanren-extra-bootstrap.sh")
            extra_result = await conn.run("bash /tmp/tanren-extra-bootstrap.sh", timeout=600)
            if extra_result.exit_code != 0:
                raise RuntimeError(f"Extra bootstrap script failed: {extra_result.stderr}")
            installed.append("extra-script")

        # Step 8: Write marker
        await conn.run(f"touch {_MARKER_PATH}", timeout=10)

        duration = int(time.monotonic() - start)
        return BootstrapResult(
            installed=tuple(installed),
            skipped=tuple(skipped),
            duration_secs=duration,
        )

    async def is_bootstrapped(self, conn) -> bool:
        """Check if the VM has been bootstrapped."""
        result = await conn.run(f"test -f {_MARKER_PATH} && echo exists", timeout=10)
        return "exists" in result.stdout

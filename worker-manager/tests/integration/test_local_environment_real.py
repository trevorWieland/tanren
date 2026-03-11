"""Real local-environment integration tests — requires git on PATH."""

from __future__ import annotations

import shutil

import pytest

pytestmark = pytest.mark.local_env


@pytest.fixture
def _require_git():
    """Skip the entire test when git is not available."""
    if shutil.which("git") is None:
        pytest.skip("git not found on PATH")


@pytest.mark.usefixtures("_require_git")
async def test_real_local_provision(tmp_path):
    """Provision a local environment and verify the worktree is created."""
    # TODO: instantiate LocalEnvironment with a real repo under tmp_path,
    #       call provision(), and assert the worktree directory exists.
    pytest.skip("stub — implement when LocalEnvironment is wired up")


@pytest.mark.usefixtures("_require_git")
async def test_real_local_env_validation(tmp_path):
    """Validate that a local environment rejects invalid configurations."""
    # TODO: pass an invalid repo path or branch, call provision(), and
    #       confirm it raises the expected validation error.
    pytest.skip("stub — implement when LocalEnvironment is wired up")

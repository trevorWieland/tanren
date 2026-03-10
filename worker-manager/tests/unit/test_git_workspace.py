"""Tests for git workspace manager."""

from unittest.mock import AsyncMock

import pytest

from worker_manager.adapters.git_workspace import GitAuthConfig, GitWorkspaceManager
from worker_manager.adapters.remote_types import (
    RemoteResult,
    SecretBundle,
    WorkspacePath,
    WorkspaceSpec,
)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _ok(stdout: str = "") -> RemoteResult:
    return RemoteResult(exit_code=0, stdout=stdout, stderr="")


def _fail(stderr: str = "error") -> RemoteResult:
    return RemoteResult(exit_code=1, stdout="", stderr=stderr)


def _make_conn() -> AsyncMock:
    conn = AsyncMock()
    conn.run = AsyncMock(return_value=_ok())
    conn.upload_content = AsyncMock()
    return conn


def _spec(**overrides) -> WorkspaceSpec:
    defaults = dict(
        project="myapp",
        repo_url="https://github.com/org/myapp.git",
        branch="main",
        setup_commands=(),
    )
    defaults.update(overrides)
    return WorkspaceSpec(**defaults)


def _workspace() -> WorkspacePath:
    return WorkspacePath(path="/workspace/myapp", project="myapp", branch="main")


# ---------------------------------------------------------------------------
# _build_repo_url
# ---------------------------------------------------------------------------

class TestBuildRepoUrl:
    def test_token_auth_injects_token(self):
        mgr = GitWorkspaceManager(GitAuthConfig(auth_method="token", token="ghp_abc"))
        result = mgr._build_repo_url("https://github.com/org/repo.git")
        assert result == "https://ghp_abc@github.com/org/repo.git"

    def test_no_token_returns_unchanged(self):
        mgr = GitWorkspaceManager(GitAuthConfig(auth_method="token", token=None))
        url = "https://github.com/org/repo.git"
        assert mgr._build_repo_url(url) == url

    def test_non_https_returns_unchanged(self):
        mgr = GitWorkspaceManager(GitAuthConfig(auth_method="token", token="ghp_abc"))
        url = "git@github.com:org/repo.git"
        assert mgr._build_repo_url(url) == url


# ---------------------------------------------------------------------------
# setup
# ---------------------------------------------------------------------------

class TestSetup:
    @pytest.mark.asyncio
    async def test_clone_fresh_when_git_dir_missing(self):
        conn = _make_conn()
        conn.run.side_effect = [
            _ok(""),          # test -d .git -> no "exists"
            _ok(),            # git clone
        ]
        mgr = GitWorkspaceManager(GitAuthConfig())
        spec = _spec()
        wp = await mgr.setup(conn, spec)

        assert wp.path == "/workspace/myapp"
        assert wp.project == "myapp"
        assert wp.branch == "main"
        # First call is the existence check, second is clone
        clone_call = conn.run.call_args_list[1]
        assert "git clone" in clone_call.args[0]

    @pytest.mark.asyncio
    async def test_pull_when_git_dir_exists(self):
        conn = _make_conn()
        conn.run.side_effect = [
            _ok("exists"),    # test -d .git -> "exists"
            _ok(),            # git pull
        ]
        mgr = GitWorkspaceManager(GitAuthConfig())
        wp = await mgr.setup(conn, _spec())

        pull_call = conn.run.call_args_list[1]
        assert "git pull" in pull_call.args[0]
        assert wp.path == "/workspace/myapp"

    @pytest.mark.asyncio
    async def test_runs_setup_commands_after_clone(self):
        conn = _make_conn()
        conn.run.side_effect = [
            _ok(""),          # test -d .git
            _ok(),            # git clone
            _ok(),            # setup cmd 1
            _ok(),            # setup cmd 2
        ]
        mgr = GitWorkspaceManager(GitAuthConfig())
        spec = _spec(setup_commands=("make install", "make build"))
        await mgr.setup(conn, spec)

        setup1 = conn.run.call_args_list[2]
        setup2 = conn.run.call_args_list[3]
        assert "make install" in setup1.args[0]
        assert "make build" in setup2.args[0]

    @pytest.mark.asyncio
    async def test_clone_failure_raises(self):
        conn = _make_conn()
        conn.run.side_effect = [
            _ok(""),                          # test -d .git
            _fail("fatal: repo not found"),   # git clone fails
        ]
        mgr = GitWorkspaceManager(GitAuthConfig())
        with pytest.raises(RuntimeError, match="Git clone failed"):
            await mgr.setup(conn, _spec())

    @pytest.mark.asyncio
    async def test_pull_failure_raises(self):
        conn = _make_conn()
        conn.run.side_effect = [
            _ok("exists"),                        # test -d .git
            _fail("error: merge conflict"),       # git pull fails
        ]
        mgr = GitWorkspaceManager(GitAuthConfig())
        with pytest.raises(RuntimeError, match="Git pull failed"):
            await mgr.setup(conn, _spec())


# ---------------------------------------------------------------------------
# inject_secrets
# ---------------------------------------------------------------------------

class TestInjectSecrets:
    @pytest.mark.asyncio
    async def test_writes_developer_secrets(self):
        conn = _make_conn()
        mgr = GitWorkspaceManager(GitAuthConfig())
        secrets = SecretBundle(developer={"API_KEY": "abc123"})
        await mgr.inject_secrets(conn, _workspace(), secrets)

        conn.upload_content.assert_any_call("API_KEY=abc123\n", "/workspace/.developer-secrets")
        conn.run.assert_any_call("chmod 600 /workspace/.developer-secrets", timeout=10)

    @pytest.mark.asyncio
    async def test_writes_project_secrets(self):
        conn = _make_conn()
        mgr = GitWorkspaceManager(GitAuthConfig())
        secrets = SecretBundle(project={"DB_URL": "postgres://localhost"})
        await mgr.inject_secrets(conn, _workspace(), secrets)

        conn.upload_content.assert_any_call(
            "DB_URL=postgres://localhost\n", "/workspace/myapp/.env"
        )
        conn.run.assert_any_call("chmod 600 /workspace/myapp/.env", timeout=10)

    @pytest.mark.asyncio
    async def test_skips_empty_secrets(self):
        conn = _make_conn()
        mgr = GitWorkspaceManager(GitAuthConfig())
        secrets = SecretBundle()  # both developer and project empty
        await mgr.inject_secrets(conn, _workspace(), secrets)

        conn.upload_content.assert_not_called()
        conn.run.assert_not_called()


# ---------------------------------------------------------------------------
# cleanup
# ---------------------------------------------------------------------------

class TestCleanup:
    @pytest.mark.asyncio
    async def test_removes_secret_files(self):
        conn = _make_conn()
        mgr = GitWorkspaceManager(GitAuthConfig())
        await mgr.cleanup(conn, _workspace())

        conn.run.assert_any_call("rm -f /workspace/.developer-secrets", timeout=10)
        conn.run.assert_any_call("rm -f /workspace/myapp/.env", timeout=10)
        assert conn.run.call_count == 2

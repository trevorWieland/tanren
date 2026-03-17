"""Tests for git workspace manager."""

import json
from unittest.mock import AsyncMock

import pytest

from tanren_core.adapters.git_workspace import GitAuthConfig, GitWorkspaceManager
from tanren_core.adapters.remote_types import (
    RemoteResult,
    SecretBundle,
    WorkspacePath,
    WorkspaceSpec,
)
from tanren_core.remote_config import GitAuthMethod
from tanren_core.roles import AuthMode
from tanren_core.schemas import Cli

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


def _run_with_home(home: str = "/root"):
    """Return a side_effect callable that resolves ``echo $HOME`` and returns _ok() otherwise."""

    def _side_effect(cmd: str, **_kwargs: object) -> RemoteResult:
        if cmd == "echo $HOME":
            return _ok(f"{home}\n")
        return _ok()

    return _side_effect


def _spec(**overrides: object) -> WorkspaceSpec:
    defaults: dict[str, object] = {
        "project": "myapp",
        "repo_url": "https://github.com/org/myapp.git",
        "branch": "main",
        "setup_commands": (),
    }
    defaults.update(overrides)
    return WorkspaceSpec.model_validate(defaults)


def _workspace() -> WorkspacePath:
    return WorkspacePath(path="/workspace/myapp", project="myapp", branch="main")


# ---------------------------------------------------------------------------
# _build_repo_url
# ---------------------------------------------------------------------------


class TestGitAuth:
    @pytest.mark.asyncio
    async def test_setup_git_auth_uploads_askpass_script(self):
        conn = _make_conn()
        mgr = GitWorkspaceManager(GitAuthConfig(auth_method=GitAuthMethod.TOKEN, token="ghp_abc"))
        await mgr._setup_git_auth(conn)

        conn.upload_content.assert_called_once()
        script = conn.upload_content.call_args.args[0]
        assert script.startswith("#!/bin/sh\n")
        assert "ghp_abc" in script
        # Token is single-quoted for safety
        assert "'ghp_abc'" in script
        conn.run.assert_any_call("chmod 700 /workspace/.git-askpass", timeout=10)

    @pytest.mark.asyncio
    async def test_setup_git_auth_skipped_without_token(self):
        conn = _make_conn()
        mgr = GitWorkspaceManager(GitAuthConfig(auth_method=GitAuthMethod.TOKEN, token=None))
        await mgr._setup_git_auth(conn)

        conn.upload_content.assert_not_called()

    def test_git_env_prefix_with_token(self):
        mgr = GitWorkspaceManager(GitAuthConfig(auth_method=GitAuthMethod.TOKEN, token="ghp_abc"))
        prefix = mgr._git_env_prefix()
        assert "GIT_ASKPASS=/workspace/.git-askpass" in prefix
        assert "GIT_TERMINAL_PROMPT=0" in prefix

    def test_git_env_prefix_without_token(self):
        mgr = GitWorkspaceManager(GitAuthConfig(auth_method=GitAuthMethod.TOKEN, token=None))
        assert not mgr._git_env_prefix()


# ---------------------------------------------------------------------------
# setup
# ---------------------------------------------------------------------------


class TestSetup:
    @pytest.mark.asyncio
    async def test_clone_fresh_when_git_dir_missing(self):
        conn = _make_conn()
        conn.run.side_effect = [
            _ok(),  # chmod askpass
            _ok(""),  # test -d .git -> no "exists"
            _ok(),  # git clone
        ]
        mgr = GitWorkspaceManager(GitAuthConfig(auth_method=GitAuthMethod.TOKEN, token="ghp_tok"))
        spec = _spec()
        wp = await mgr.setup(conn, spec)

        assert wp.path == "/workspace/myapp"
        assert wp.project == "myapp"
        assert wp.branch == "main"
        # Find the clone call
        clone_calls = [c for c in conn.run.call_args_list if "git clone" in str(c)]
        assert len(clone_calls) == 1
        clone_cmd = clone_calls[0].args[0]
        # Token must NOT be in the clone URL
        assert "ghp_tok" not in clone_cmd
        assert "GIT_ASKPASS" in clone_cmd

    @pytest.mark.asyncio
    async def test_clone_without_token_uses_plain_url(self):
        conn = _make_conn()
        conn.run.side_effect = [
            _ok(""),  # test -d .git -> no "exists"
            _ok(),  # git clone
        ]
        mgr = GitWorkspaceManager(GitAuthConfig(auth_method=GitAuthMethod.TOKEN, token=None))
        await mgr.setup(conn, _spec())

        clone_calls = [c for c in conn.run.call_args_list if "git clone" in str(c)]
        assert len(clone_calls) == 1
        clone_cmd = clone_calls[0].args[0]
        assert "GIT_ASKPASS" not in clone_cmd

    @pytest.mark.asyncio
    async def test_pull_when_git_dir_exists(self):
        conn = _make_conn()
        conn.run.side_effect = [
            _ok("exists"),  # test -d .git -> "exists"
            _ok(),  # git pull
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
            _ok(""),  # test -d .git
            _ok(),  # git clone
            _ok(),  # setup cmd 1
            _ok(),  # setup cmd 2
        ]
        mgr = GitWorkspaceManager(GitAuthConfig())
        spec = _spec(setup_commands=("make install", "make build"))
        await mgr.setup(conn, spec)

        setup_calls = [
            c for c in conn.run.call_args_list if "make install" in str(c) or "make build" in str(c)
        ]
        assert len(setup_calls) == 2

    @pytest.mark.asyncio
    async def test_clone_failure_raises(self):
        conn = _make_conn()
        conn.run.side_effect = [
            _ok(""),  # test -d .git
            _fail("fatal: repo not found"),  # git clone fails
        ]
        mgr = GitWorkspaceManager(GitAuthConfig())
        with pytest.raises(RuntimeError, match="Git clone failed"):
            await mgr.setup(conn, _spec())

    @pytest.mark.asyncio
    async def test_pull_failure_raises(self):
        conn = _make_conn()
        conn.run.side_effect = [
            _ok("exists"),  # test -d .git
            _fail("error: merge conflict"),  # git pull fails
        ]
        mgr = GitWorkspaceManager(GitAuthConfig())
        with pytest.raises(RuntimeError, match="Git pull failed"):
            await mgr.setup(conn, _spec())


# ---------------------------------------------------------------------------
# inject_secrets
# ---------------------------------------------------------------------------


class TestInjectSecrets:
    @pytest.mark.asyncio
    async def test_writes_developer_secrets_shell_quoted(self):
        conn = _make_conn()
        mgr = GitWorkspaceManager(GitAuthConfig())
        secrets = SecretBundle(developer={"API_KEY": "abc123"})
        await mgr.inject_secrets(conn, _workspace(), secrets)

        conn.upload_content.assert_any_call("API_KEY='abc123'\n", "/workspace/.developer-secrets")
        conn.run.assert_any_call("chmod 600 /workspace/.developer-secrets", timeout=10)

    @pytest.mark.asyncio
    async def test_writes_project_secrets_shell_quoted(self):
        conn = _make_conn()
        mgr = GitWorkspaceManager(GitAuthConfig())
        secrets = SecretBundle(project={"DB_URL": "postgres://localhost"})
        await mgr.inject_secrets(conn, _workspace(), secrets)

        conn.upload_content.assert_any_call(
            "DB_URL='postgres://localhost'\n", "/workspace/myapp/.env"
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

    @pytest.mark.asyncio
    async def test_special_characters_escaped(self):
        conn = _make_conn()
        mgr = GitWorkspaceManager(GitAuthConfig())
        secrets = SecretBundle(
            developer={
                "PASS": "it's a $ecret; rm -rf /",
            }
        )
        await mgr.inject_secrets(conn, _workspace(), secrets)

        content = conn.upload_content.call_args_list[0].args[0]
        # Single quotes protect special chars; embedded single quote is escaped
        assert content == "PASS='it'\\''s a $ecret; rm -rf /'\n"

    @pytest.mark.asyncio
    async def test_empty_value_quoted(self):
        conn = _make_conn()
        mgr = GitWorkspaceManager(GitAuthConfig())
        secrets = SecretBundle(developer={"EMPTY": ""})
        await mgr.inject_secrets(conn, _workspace(), secrets)

        content = conn.upload_content.call_args_list[0].args[0]
        assert content == "EMPTY=''\n"


# ---------------------------------------------------------------------------
# push_command
# ---------------------------------------------------------------------------


class TestPushCommand:
    def test_push_command_with_token_includes_auth_env(self):
        mgr = GitWorkspaceManager(GitAuthConfig(auth_method=GitAuthMethod.TOKEN, token="ghp_abc"))
        cmd = mgr.push_command("/workspace/myapp", "feature-1")
        assert "GIT_ASKPASS=/workspace/.git-askpass" in cmd
        assert "GIT_TERMINAL_PROMPT=0" in cmd
        assert "git push origin" in cmd
        assert "feature-1" in cmd
        assert "cd /workspace/myapp" in cmd

    def test_push_command_without_token_no_auth_env(self):
        mgr = GitWorkspaceManager(GitAuthConfig(auth_method=GitAuthMethod.TOKEN, token=None))
        cmd = mgr.push_command("/workspace/myapp", "main")
        assert "GIT_ASKPASS" not in cmd
        assert "git push origin main" in cmd

    def test_push_command_quotes_branch_with_special_chars(self):
        mgr = GitWorkspaceManager(GitAuthConfig())
        cmd = mgr.push_command("/workspace/myapp", "feature/my branch")
        assert "feature/my branch" in cmd
        # Branch should be shell-quoted
        assert "'" in cmd or '"' in cmd


# ---------------------------------------------------------------------------
# cleanup
# ---------------------------------------------------------------------------


class TestCleanup:
    @pytest.mark.asyncio
    async def test_removes_secret_files_askpass_and_cli_auth(self):
        conn = _make_conn()
        mgr = GitWorkspaceManager(GitAuthConfig())
        await mgr.cleanup(conn, _workspace())

        conn.run.assert_any_call("rm -f /workspace/.developer-secrets", timeout=10)
        conn.run.assert_any_call("rm -f /workspace/myapp/.env", timeout=10)
        conn.run.assert_any_call("rm -f /workspace/.git-askpass", timeout=10)
        conn.run.assert_any_call("rm -f ~/.local/share/opencode/auth.json", timeout=10)
        conn.run.assert_any_call("rm -f ~/.claude/.credentials.json", timeout=10)
        conn.run.assert_any_call("rm -f ~/.codex/auth.json", timeout=10)
        assert conn.run.call_count == 6


# ---------------------------------------------------------------------------
# CLI auth injection
# ---------------------------------------------------------------------------


class TestCliAuthInjection:
    @pytest.mark.asyncio
    async def test_opencode_api_key_writes_auth_json(self):
        conn = _make_conn()
        conn.run.side_effect = _run_with_home("/home/deploy")
        mgr = GitWorkspaceManager(GitAuthConfig())
        secrets = SecretBundle(developer={"OPENCODE_ZAI_API_KEY": "zai-key-123"})
        await mgr.inject_secrets(
            conn,
            _workspace(),
            secrets,
            cli_auth=(Cli.OPENCODE, AuthMode.API_KEY),
        )

        # Find the auth.json upload — SFTP uses resolved absolute path
        upload_calls = conn.upload_content.call_args_list
        auth_uploads = [c for c in upload_calls if "auth.json" in str(c)]
        assert len(auth_uploads) == 1
        assert auth_uploads[0].args[1] == "/home/deploy/.local/share/opencode/auth.json"
        content = auth_uploads[0].args[0]
        data = json.loads(content)
        assert data["zai-coding-plan"]["type"] == "api"
        assert data["zai-coding-plan"]["key"] == "zai-key-123"

        # Shell commands use tilde path
        conn.run.assert_any_call("chmod 600 ~/.local/share/opencode/auth.json", timeout=10)

    @pytest.mark.asyncio
    async def test_opencode_api_key_from_project_secrets(self):
        conn = _make_conn()
        conn.run.side_effect = _run_with_home("/root")
        mgr = GitWorkspaceManager(GitAuthConfig())
        secrets = SecretBundle(project={"OPENCODE_ZAI_API_KEY": "proj-key-456"})
        await mgr.inject_secrets(
            conn,
            _workspace(),
            secrets,
            cli_auth=(Cli.OPENCODE, AuthMode.API_KEY),
        )

        upload_calls = conn.upload_content.call_args_list
        auth_uploads = [c for c in upload_calls if "auth.json" in str(c)]
        assert len(auth_uploads) == 1
        assert auth_uploads[0].args[1] == "/root/.local/share/opencode/auth.json"
        content = auth_uploads[0].args[0]
        data = json.loads(content)
        assert data["zai-coding-plan"]["key"] == "proj-key-456"

    @pytest.mark.asyncio
    async def test_opencode_api_key_missing_clears_stale_auth(self):
        conn = _make_conn()
        mgr = GitWorkspaceManager(GitAuthConfig())
        secrets = SecretBundle()  # no OPENCODE_ZAI_API_KEY
        await mgr.inject_secrets(
            conn,
            _workspace(),
            secrets,
            cli_auth=(Cli.OPENCODE, AuthMode.API_KEY),
        )

        # No auth.json should be uploaded
        upload_calls = conn.upload_content.call_args_list
        auth_uploads = [c for c in upload_calls if "auth.json" in str(c)]
        assert len(auth_uploads) == 0

        # Stale auth.json must be removed
        conn.run.assert_any_call("rm -f ~/.local/share/opencode/auth.json", timeout=10)

    @pytest.mark.asyncio
    async def test_fallback_cli_removes_all_stale_auth(self):
        """Unhandled cli/auth combo removes all three auth paths."""
        conn = _make_conn()
        mgr = GitWorkspaceManager(GitAuthConfig())
        secrets = SecretBundle()

        await mgr.inject_cli_auth(conn, secrets, (Cli.BASH, AuthMode.API_KEY))

        conn.run.assert_any_call("rm -f ~/.local/share/opencode/auth.json", timeout=10)
        conn.run.assert_any_call("rm -f ~/.claude/.credentials.json", timeout=10)
        conn.run.assert_any_call("rm -f ~/.codex/auth.json", timeout=10)

    @pytest.mark.asyncio
    async def test_opencode_api_key_does_not_remove_own_auth(self):
        """opencode/api_key writes auth.json, never removes its own."""
        conn = _make_conn()
        conn.run.side_effect = _run_with_home("/root")
        mgr = GitWorkspaceManager(GitAuthConfig())
        secrets = SecretBundle(developer={"OPENCODE_ZAI_API_KEY": "zai-key-123"})

        await mgr.inject_cli_auth(conn, secrets, (Cli.OPENCODE, AuthMode.API_KEY))

        rm_calls = [str(c) for c in conn.run.call_args_list]
        assert not any("rm -f" in c and "opencode" in c and "auth.json" in c for c in rm_calls)

    @pytest.mark.asyncio
    async def test_stateless_across_connections(self):
        """Manager holds no per-connection state between calls."""
        mgr = GitWorkspaceManager(GitAuthConfig())
        secrets = SecretBundle(
            developer={
                "OPENCODE_ZAI_API_KEY": "zai-key-123",
                "CODEX_AUTH_JSON": '{"session": "xyz"}',
            }
        )

        # VM A: inject opencode auth
        conn_a = _make_conn()
        conn_a.run.side_effect = _run_with_home("/root")
        await mgr.inject_cli_auth(conn_a, secrets, (Cli.OPENCODE, AuthMode.API_KEY))

        # VM B: different cli — should still clean up opencode + claude on B
        conn_b = _make_conn()
        conn_b.run.side_effect = _run_with_home("/root")
        await mgr.inject_cli_auth(conn_b, secrets, (Cli.CODEX, AuthMode.SUBSCRIPTION))

        conn_b.run.assert_any_call("rm -f ~/.local/share/opencode/auth.json", timeout=10)
        conn_b.run.assert_any_call("rm -f ~/.claude/.credentials.json", timeout=10)
        # VM A: opencode's own auth path was not rm'd
        conn_a_rm_calls = [str(c) for c in conn_a.run.call_args_list]
        assert not any(
            "rm -f" in c and "opencode" in c and "auth.json" in c for c in conn_a_rm_calls
        )

    @pytest.mark.asyncio
    async def test_claude_oauth_no_auth_file(self):
        conn = _make_conn()
        mgr = GitWorkspaceManager(GitAuthConfig())
        secrets = SecretBundle(developer={"OPENCODE_ZAI_API_KEY": "key"})
        await mgr.inject_secrets(
            conn,
            _workspace(),
            secrets,
            cli_auth=(Cli.CLAUDE, AuthMode.OAUTH),
        )

        # No CLI auth should be uploaded for claude/oauth (fallback branch)
        upload_calls = conn.upload_content.call_args_list
        auth_uploads = [
            c for c in upload_calls if "auth.json" in str(c) or ".credentials.json" in str(c)
        ]
        assert len(auth_uploads) == 0

    @pytest.mark.asyncio
    async def test_no_cli_auth_no_auth_file(self):
        conn = _make_conn()
        mgr = GitWorkspaceManager(GitAuthConfig())
        secrets = SecretBundle(developer={"OPENCODE_ZAI_API_KEY": "key"})
        await mgr.inject_secrets(conn, _workspace(), secrets)

        # No auth.json should be uploaded when cli_auth is None
        upload_calls = conn.upload_content.call_args_list
        auth_uploads = [c for c in upload_calls if "auth.json" in str(c)]
        assert len(auth_uploads) == 0

    @pytest.mark.asyncio
    async def test_claude_subscription_writes_credentials(self):
        conn = _make_conn()
        conn.run.side_effect = _run_with_home("/home/deploy")
        mgr = GitWorkspaceManager(GitAuthConfig())
        creds = '{"token": "claude-tok"}'
        secrets = SecretBundle(developer={"CLAUDE_CREDENTIALS_JSON": creds})

        await mgr.inject_cli_auth(conn, secrets, (Cli.CLAUDE, AuthMode.SUBSCRIPTION))

        upload_calls = conn.upload_content.call_args_list
        cred_uploads = [c for c in upload_calls if ".credentials.json" in str(c)]
        assert len(cred_uploads) == 1
        assert cred_uploads[0].args[1] == "/home/deploy/.claude/.credentials.json"
        assert cred_uploads[0].args[0] == creds
        conn.run.assert_any_call("mkdir -p ~/.claude", timeout=10)
        conn.run.assert_any_call("chmod 600 ~/.claude/.credentials.json", timeout=10)

    @pytest.mark.asyncio
    async def test_codex_subscription_writes_auth(self):
        conn = _make_conn()
        conn.run.side_effect = _run_with_home("/root")
        mgr = GitWorkspaceManager(GitAuthConfig())
        auth = '{"session": "codex-tok"}'
        secrets = SecretBundle(developer={"CODEX_AUTH_JSON": auth})

        await mgr.inject_cli_auth(conn, secrets, (Cli.CODEX, AuthMode.SUBSCRIPTION))

        upload_calls = conn.upload_content.call_args_list
        auth_uploads = [c for c in upload_calls if ".codex/auth.json" in str(c)]
        assert len(auth_uploads) == 1
        assert auth_uploads[0].args[1] == "/root/.codex/auth.json"
        assert auth_uploads[0].args[0] == auth
        conn.run.assert_any_call("mkdir -p ~/.codex", timeout=10)
        conn.run.assert_any_call("chmod 600 ~/.codex/auth.json", timeout=10)

    @pytest.mark.asyncio
    async def test_claude_subscription_missing_key_warns_and_cleans(self):
        conn = _make_conn()
        mgr = GitWorkspaceManager(GitAuthConfig())
        secrets = SecretBundle()

        await mgr.inject_cli_auth(conn, secrets, (Cli.CLAUDE, AuthMode.SUBSCRIPTION))

        conn.upload_content.assert_not_called()
        conn.run.assert_any_call("rm -f ~/.claude/.credentials.json", timeout=10)
        conn.run.assert_any_call("rm -f ~/.local/share/opencode/auth.json", timeout=10)
        conn.run.assert_any_call("rm -f ~/.codex/auth.json", timeout=10)

    @pytest.mark.asyncio
    async def test_codex_subscription_missing_key_warns_and_cleans(self):
        conn = _make_conn()
        mgr = GitWorkspaceManager(GitAuthConfig())
        secrets = SecretBundle()

        await mgr.inject_cli_auth(conn, secrets, (Cli.CODEX, AuthMode.SUBSCRIPTION))

        conn.upload_content.assert_not_called()
        conn.run.assert_any_call("rm -f ~/.codex/auth.json", timeout=10)
        conn.run.assert_any_call("rm -f ~/.local/share/opencode/auth.json", timeout=10)
        conn.run.assert_any_call("rm -f ~/.claude/.credentials.json", timeout=10)

    @pytest.mark.asyncio
    async def test_opencode_removes_claude_and_codex_auth(self):
        conn = _make_conn()
        conn.run.side_effect = _run_with_home("/root")
        mgr = GitWorkspaceManager(GitAuthConfig())
        secrets = SecretBundle(developer={"OPENCODE_ZAI_API_KEY": "zai-key"})

        await mgr.inject_cli_auth(conn, secrets, (Cli.OPENCODE, AuthMode.API_KEY))

        conn.run.assert_any_call("rm -f ~/.claude/.credentials.json", timeout=10)
        conn.run.assert_any_call("rm -f ~/.codex/auth.json", timeout=10)

    @pytest.mark.asyncio
    async def test_claude_removes_opencode_and_codex_auth(self):
        conn = _make_conn()
        conn.run.side_effect = _run_with_home("/root")
        mgr = GitWorkspaceManager(GitAuthConfig())
        secrets = SecretBundle(developer={"CLAUDE_CREDENTIALS_JSON": '{"t": "x"}'})

        await mgr.inject_cli_auth(conn, secrets, (Cli.CLAUDE, AuthMode.SUBSCRIPTION))

        conn.run.assert_any_call("rm -f ~/.local/share/opencode/auth.json", timeout=10)
        conn.run.assert_any_call("rm -f ~/.codex/auth.json", timeout=10)
        # Own path NOT removed
        rm_calls = [str(c) for c in conn.run.call_args_list]
        assert not any("rm -f" in c and ".claude/.credentials.json" in c for c in rm_calls)

    @pytest.mark.asyncio
    async def test_codex_removes_opencode_and_claude_auth(self):
        conn = _make_conn()
        conn.run.side_effect = _run_with_home("/root")
        mgr = GitWorkspaceManager(GitAuthConfig())
        secrets = SecretBundle(developer={"CODEX_AUTH_JSON": '{"s": "y"}'})

        await mgr.inject_cli_auth(conn, secrets, (Cli.CODEX, AuthMode.SUBSCRIPTION))

        conn.run.assert_any_call("rm -f ~/.local/share/opencode/auth.json", timeout=10)
        conn.run.assert_any_call("rm -f ~/.claude/.credentials.json", timeout=10)
        # Own path NOT removed
        rm_calls = [str(c) for c in conn.run.call_args_list]
        assert not any("rm -f" in c and ".codex/auth.json" in c for c in rm_calls)

"""Tests for credential providers."""

import json
from typing import Any
from unittest.mock import AsyncMock

import pytest

from tanren_core.adapters.credentials import (
    ClaudeCredentialProvider,
    CodexCredentialProvider,
    OpencodeCredentialProvider,
    all_credential_cleanup_paths,
    inject_all_cli_credentials,
    providers_for_clis,
)
from tanren_core.adapters.remote_types import RemoteResult, SecretBundle
from tanren_core.schemas import Cli

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _ok(stdout: str = "") -> RemoteResult:
    return RemoteResult(exit_code=0, stdout=stdout, stderr="")


def _make_conn(home: str = "/root") -> AsyncMock:
    conn = AsyncMock()

    def _side_effect(cmd: str, **_kwargs: Any) -> RemoteResult:
        if cmd == "echo $HOME":
            return _ok(f"{home}\n")
        return _ok()

    conn.run = AsyncMock(side_effect=_side_effect)
    conn.upload_content = AsyncMock()
    return conn


# ---------------------------------------------------------------------------
# OpencodeCredentialProvider
# ---------------------------------------------------------------------------


class TestOpencodeCredentialProvider:
    @pytest.mark.asyncio
    async def test_injects_auth_json_from_developer_secrets(self):
        conn = _make_conn("/home/deploy")
        provider = OpencodeCredentialProvider()
        secrets = SecretBundle(developer={"OPENCODE_ZAI_API_KEY": "zai-key-123"})

        result = await provider.inject(conn, secrets)

        assert result is True
        upload_calls = conn.upload_content.call_args_list
        auth_uploads = [c for c in upload_calls if "auth.json" in str(c)]
        assert len(auth_uploads) == 1
        assert auth_uploads[0].args[1] == "/home/deploy/.local/share/opencode/auth.json"
        data = json.loads(auth_uploads[0].args[0])
        assert data["zai-coding-plan"]["type"] == "api"
        assert data["zai-coding-plan"]["key"] == "zai-key-123"
        conn.run.assert_any_call(
            "chmod 600 /home/deploy/.local/share/opencode/auth.json", timeout_secs=10
        )

    @pytest.mark.asyncio
    async def test_injects_auth_json_from_project_secrets(self):
        conn = _make_conn("/root")
        provider = OpencodeCredentialProvider()
        secrets = SecretBundle(project={"OPENCODE_ZAI_API_KEY": "proj-key-456"})

        result = await provider.inject(conn, secrets)

        assert result is True
        upload_calls = conn.upload_content.call_args_list
        auth_uploads = [c for c in upload_calls if "auth.json" in str(c)]
        assert len(auth_uploads) == 1
        assert auth_uploads[0].args[1] == "/root/.local/share/opencode/auth.json"
        data = json.loads(auth_uploads[0].args[0])
        assert data["zai-coding-plan"]["key"] == "proj-key-456"

    @pytest.mark.asyncio
    async def test_returns_false_when_key_missing(self):
        conn = _make_conn()
        provider = OpencodeCredentialProvider()
        secrets = SecretBundle()

        result = await provider.inject(conn, secrets)

        assert result is False
        conn.upload_content.assert_not_called()

    def test_cleanup_paths(self):
        provider = OpencodeCredentialProvider()
        assert provider.cleanup_paths == ("~/.local/share/opencode/auth.json",)

    def test_name(self):
        assert OpencodeCredentialProvider().name == "opencode"


# ---------------------------------------------------------------------------
# ClaudeCredentialProvider
# ---------------------------------------------------------------------------


class TestClaudeCredentialProvider:
    @pytest.mark.asyncio
    async def test_injects_credentials_json(self):
        conn = _make_conn("/home/deploy")
        provider = ClaudeCredentialProvider()
        creds = '{"token": "claude-tok"}'
        secrets = SecretBundle(developer={"CLAUDE_CREDENTIALS_JSON": creds})

        result = await provider.inject(conn, secrets)

        assert result is True
        upload_calls = conn.upload_content.call_args_list
        cred_uploads = [c for c in upload_calls if ".credentials.json" in str(c)]
        assert len(cred_uploads) == 1
        assert cred_uploads[0].args[1] == "/home/deploy/.claude/.credentials.json"
        assert cred_uploads[0].args[0] == creds
        conn.run.assert_any_call("mkdir -p /home/deploy/.claude", timeout_secs=10)
        conn.run.assert_any_call(
            "chmod 600 /home/deploy/.claude/.credentials.json", timeout_secs=10
        )

    @pytest.mark.asyncio
    async def test_returns_false_when_key_missing(self):
        conn = _make_conn()
        provider = ClaudeCredentialProvider()
        secrets = SecretBundle()

        result = await provider.inject(conn, secrets)

        assert result is False
        conn.upload_content.assert_not_called()

    def test_cleanup_paths(self):
        provider = ClaudeCredentialProvider()
        assert provider.cleanup_paths == ("~/.claude/.credentials.json",)

    def test_name(self):
        assert ClaudeCredentialProvider().name == "claude"


# ---------------------------------------------------------------------------
# CodexCredentialProvider
# ---------------------------------------------------------------------------


class TestCodexCredentialProvider:
    @pytest.mark.asyncio
    async def test_injects_auth_json(self):
        conn = _make_conn("/root")
        provider = CodexCredentialProvider()
        auth = '{"session": "codex-tok"}'
        secrets = SecretBundle(developer={"CODEX_AUTH_JSON": auth})

        result = await provider.inject(conn, secrets)

        assert result is True
        upload_calls = conn.upload_content.call_args_list
        auth_uploads = [c for c in upload_calls if ".codex/auth.json" in str(c)]
        assert len(auth_uploads) == 1
        assert auth_uploads[0].args[1] == "/root/.codex/auth.json"
        assert auth_uploads[0].args[0] == auth
        conn.run.assert_any_call("mkdir -p /root/.codex", timeout_secs=10)
        conn.run.assert_any_call("chmod 600 /root/.codex/auth.json", timeout_secs=10)

    @pytest.mark.asyncio
    async def test_returns_false_when_key_missing(self):
        conn = _make_conn()
        provider = CodexCredentialProvider()
        secrets = SecretBundle()

        result = await provider.inject(conn, secrets)

        assert result is False
        conn.upload_content.assert_not_called()

    def test_cleanup_paths(self):
        provider = CodexCredentialProvider()
        assert provider.cleanup_paths == ("~/.codex/auth.json",)

    def test_name(self):
        assert CodexCredentialProvider().name == "codex"


# ---------------------------------------------------------------------------
# home_dir parameter
# ---------------------------------------------------------------------------


class TestHomeDirParameter:
    @pytest.mark.asyncio
    async def test_opencode_uses_home_dir(self):
        conn = _make_conn("/root")
        provider = OpencodeCredentialProvider()
        secrets = SecretBundle(developer={"OPENCODE_ZAI_API_KEY": "key"})

        await provider.inject(conn, secrets, home_dir="/home/tanren")

        upload_calls = conn.upload_content.call_args_list
        auth_uploads = [c for c in upload_calls if "auth.json" in str(c)]
        assert auth_uploads[0].args[1] == "/home/tanren/.local/share/opencode/auth.json"
        conn.run.assert_any_call("mkdir -p /home/tanren/.local/share/opencode", timeout_secs=10)
        conn.run.assert_any_call(
            "chmod 600 /home/tanren/.local/share/opencode/auth.json", timeout_secs=10
        )
        conn.run.assert_any_call(
            "chown -R tanren:tanren /home/tanren/.local/share/opencode", timeout_secs=10
        )

    @pytest.mark.asyncio
    async def test_claude_uses_home_dir(self):
        conn = _make_conn("/root")
        provider = ClaudeCredentialProvider()
        secrets = SecretBundle(developer={"CLAUDE_CREDENTIALS_JSON": '{"t": "c"}'})

        await provider.inject(conn, secrets, home_dir="/home/tanren")

        upload_calls = conn.upload_content.call_args_list
        cred_uploads = [c for c in upload_calls if ".credentials.json" in str(c)]
        assert cred_uploads[0].args[1] == "/home/tanren/.claude/.credentials.json"
        conn.run.assert_any_call("mkdir -p /home/tanren/.claude", timeout_secs=10)
        conn.run.assert_any_call("chown -R tanren:tanren /home/tanren/.claude", timeout_secs=10)

    @pytest.mark.asyncio
    async def test_codex_uses_home_dir(self):
        conn = _make_conn("/root")
        provider = CodexCredentialProvider()
        secrets = SecretBundle(developer={"CODEX_AUTH_JSON": '{"s": "x"}'})

        await provider.inject(conn, secrets, home_dir="/home/tanren")

        upload_calls = conn.upload_content.call_args_list
        auth_uploads = [c for c in upload_calls if ".codex/auth.json" in str(c)]
        assert auth_uploads[0].args[1] == "/home/tanren/.codex/auth.json"
        conn.run.assert_any_call("mkdir -p /home/tanren/.codex", timeout_secs=10)
        conn.run.assert_any_call("chown -R tanren:tanren /home/tanren/.codex", timeout_secs=10)

    @pytest.mark.asyncio
    async def test_no_chown_without_home_dir(self):
        conn = _make_conn("/root")
        provider = ClaudeCredentialProvider()
        secrets = SecretBundle(developer={"CLAUDE_CREDENTIALS_JSON": '{"t": "c"}'})

        await provider.inject(conn, secrets)

        run_cmds = [call.args[0] for call in conn.run.call_args_list]
        assert not any("chown" in c for c in run_cmds)


# ---------------------------------------------------------------------------
# inject_all_cli_credentials
# ---------------------------------------------------------------------------


class TestInjectAllCliCredentials:
    @pytest.mark.asyncio
    async def test_all_present(self):
        conn = _make_conn("/root")
        secrets = SecretBundle(
            developer={
                "OPENCODE_ZAI_API_KEY": "zai-key",
                "CLAUDE_CREDENTIALS_JSON": '{"t": "c"}',
                "CODEX_AUTH_JSON": '{"s": "x"}',
            }
        )

        injected = await inject_all_cli_credentials(conn, secrets)

        assert sorted(injected) == ["claude", "codex", "opencode"]

    @pytest.mark.asyncio
    async def test_some_missing(self):
        conn = _make_conn("/root")
        secrets = SecretBundle(developer={"OPENCODE_ZAI_API_KEY": "zai-key"})

        injected = await inject_all_cli_credentials(conn, secrets)

        assert injected == ["opencode"]

    @pytest.mark.asyncio
    async def test_none_present(self):
        conn = _make_conn()
        secrets = SecretBundle()

        injected = await inject_all_cli_credentials(conn, secrets)

        assert injected == []

    @pytest.mark.asyncio
    async def test_inject_raises_graceful(self):
        """A failing provider is logged but does not stop other providers."""
        conn = _make_conn("/root")
        secrets = SecretBundle(
            developer={
                "OPENCODE_ZAI_API_KEY": "zai-key",
                "CODEX_AUTH_JSON": '{"s": "x"}',
            }
        )

        # Create a provider that raises
        failing = AsyncMock()
        failing.name = "broken"
        failing.inject = AsyncMock(side_effect=RuntimeError("boom"))

        opencode = OpencodeCredentialProvider()
        codex = CodexCredentialProvider()

        injected = await inject_all_cli_credentials(conn, secrets, (opencode, failing, codex))

        assert sorted(injected) == ["codex", "opencode"]

    @pytest.mark.asyncio
    async def test_passes_target_home(self):
        conn = _make_conn("/root")
        secrets = SecretBundle(developer={"CLAUDE_CREDENTIALS_JSON": '{"t": "c"}'})
        providers = (ClaudeCredentialProvider(),)

        await inject_all_cli_credentials(conn, secrets, providers, target_home="/home/tanren")

        upload_calls = conn.upload_content.call_args_list
        cred_uploads = [c for c in upload_calls if ".credentials.json" in str(c)]
        assert cred_uploads[0].args[1] == "/home/tanren/.claude/.credentials.json"


# ---------------------------------------------------------------------------
# all_credential_cleanup_paths
# ---------------------------------------------------------------------------


class TestAllCredentialCleanupPaths:
    def test_default_providers(self):
        paths = all_credential_cleanup_paths()
        assert "~/.local/share/opencode/auth.json" in paths
        assert "~/.claude/.credentials.json" in paths
        assert "~/.codex/auth.json" in paths
        assert len(paths) == 3

    def test_custom_providers(self):
        provider = OpencodeCredentialProvider()
        paths = all_credential_cleanup_paths((provider,))
        assert paths == ["~/.local/share/opencode/auth.json"]


# ---------------------------------------------------------------------------
# providers_for_clis
# ---------------------------------------------------------------------------


class TestProvidersForClis:
    def test_single_cli(self):
        providers = providers_for_clis(frozenset({Cli.CLAUDE}))
        assert len(providers) == 1
        assert providers[0].name == "claude"

    def test_multiple_clis(self):
        providers = providers_for_clis(frozenset({Cli.CLAUDE, Cli.CODEX, Cli.OPENCODE}))
        names = [p.name for p in providers]
        assert sorted(names) == ["claude", "codex", "opencode"]

    def test_empty_set(self):
        providers = providers_for_clis(frozenset())
        assert providers == ()

    def test_bash_only(self):
        providers = providers_for_clis(frozenset({Cli.BASH}))
        assert providers == ()

    def test_bash_ignored(self):
        providers = providers_for_clis(frozenset({Cli.CLAUDE, Cli.BASH}))
        assert len(providers) == 1
        assert providers[0].name == "claude"

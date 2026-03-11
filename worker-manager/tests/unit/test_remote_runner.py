"""Tests for remote agent runner."""

from unittest.mock import AsyncMock

from worker_manager.adapters.remote_runner import RemoteAgentRunner
from worker_manager.adapters.remote_types import RemoteAgentResult, RemoteResult, WorkspacePath


def _make_workspace(path: str = "/workspace/myproj") -> WorkspacePath:
    return WorkspacePath(path=path, project="myproj", branch="main")


def _make_conn(
    *,
    exit_code: int = 0,
    stdout: str = "",
    stderr: str = "",
    timed_out: bool = False,
    signal_content: str | None = "LGTM",
) -> AsyncMock:
    conn = AsyncMock()
    conn.run.return_value = RemoteResult(
        exit_code=exit_code,
        stdout=stdout,
        stderr=stderr,
        timed_out=timed_out,
    )
    conn.download_content.return_value = signal_content
    return conn


class TestRemoteAgentRunnerRun:
    async def test_uploads_prompt_file(self):
        conn = _make_conn()
        ws = _make_workspace()
        runner = RemoteAgentRunner()

        await runner.run(
            conn,
            ws,
            prompt_content="Do the thing",
            cli_command="claude --prompt .tanren-prompt.md",
            signal_path="/workspace/myproj/.signal",
        )

        conn.upload_content.assert_any_call("Do the thing", "/workspace/myproj/.tanren-prompt.md")

    async def test_builds_correct_command_with_secret_sourcing(self):
        conn = _make_conn()
        ws = _make_workspace()
        runner = RemoteAgentRunner()

        await runner.run(
            conn,
            ws,
            prompt_content="prompt",
            cli_command="claude --prompt .tanren-prompt.md",
            signal_path="/workspace/myproj/.signal",
        )

        # First conn.run call is the agent command; second is cleanup
        agent_call = conn.run.call_args_list[0]
        cmd = agent_call.args[0]

        assert "set -a" in cmd
        assert "source /workspace/.developer-secrets" in cmd
        assert f"source {ws.path}/.env" in cmd
        assert f"cd {ws.path}" in cmd
        assert cmd.endswith("claude --prompt .tanren-prompt.md")

    async def test_different_cli_types(self):
        conn = _make_conn()
        ws = _make_workspace()
        runner = RemoteAgentRunner()

        for cli in ["claude --prompt p.md", "bash run.sh", "opencode exec"]:
            conn.reset_mock()
            conn.run.return_value = RemoteResult(exit_code=0, stdout="", stderr="", timed_out=False)
            conn.download_content.return_value = ""

            await runner.run(
                conn,
                ws,
                prompt_content="p",
                cli_command=cli,
                signal_path="/workspace/myproj/.signal",
            )

            agent_cmd = conn.run.call_args_list[0].args[0]
            assert agent_cmd.endswith(cli)

    async def test_extracts_signal_content_via_download(self):
        conn = _make_conn(signal_content="APPROVED: looks good")
        ws = _make_workspace()
        runner = RemoteAgentRunner()

        result = await runner.run(
            conn,
            ws,
            prompt_content="review this",
            cli_command="claude --prompt .tanren-prompt.md",
            signal_path="/workspace/myproj/.signal",
        )

        conn.download_content.assert_called_once_with("/workspace/myproj/.signal")
        assert result.signal_content == "APPROVED: looks good"

    async def test_handles_none_signal_content(self):
        conn = _make_conn(signal_content=None)
        ws = _make_workspace()
        runner = RemoteAgentRunner()

        result = await runner.run(
            conn,
            ws,
            prompt_content="prompt",
            cli_command="claude --prompt .tanren-prompt.md",
            signal_path="/workspace/myproj/.signal",
        )

        assert result.signal_content == ""

    async def test_returns_remote_agent_result_with_correct_fields(self):
        conn = _make_conn(
            exit_code=1,
            stdout="some output",
            stderr="warn: something",
            timed_out=False,
        )
        ws = _make_workspace()
        runner = RemoteAgentRunner()

        result = await runner.run(
            conn,
            ws,
            prompt_content="prompt",
            cli_command="claude --prompt .tanren-prompt.md",
            signal_path="/workspace/myproj/.signal",
        )

        assert isinstance(result, RemoteAgentResult)
        assert result.exit_code == 1
        assert result.stdout == "some output"
        assert result.stderr == "warn: something"
        assert result.timed_out is False
        assert isinstance(result.duration_secs, int)
        assert result.signal_content == "LGTM"

    async def test_timeout_passed_to_conn_run(self):
        conn = _make_conn()
        ws = _make_workspace()
        runner = RemoteAgentRunner()

        await runner.run(
            conn,
            ws,
            prompt_content="prompt",
            cli_command="claude --prompt .tanren-prompt.md",
            signal_path="/workspace/myproj/.signal",
            timeout=600,
        )

        agent_call = conn.run.call_args_list[0]
        assert agent_call.kwargs["timeout"] == 600

    async def test_timed_out_command_sets_timed_out_true(self):
        conn = _make_conn(exit_code=124, timed_out=True)
        ws = _make_workspace()
        runner = RemoteAgentRunner()

        result = await runner.run(
            conn,
            ws,
            prompt_content="prompt",
            cli_command="claude --prompt .tanren-prompt.md",
            signal_path="/workspace/myproj/.signal",
        )

        assert result.timed_out is True
        assert result.exit_code == 124

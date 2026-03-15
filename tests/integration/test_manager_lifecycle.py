"""Integration test: manager lifecycle with real IPC."""

import asyncio
import json
from pathlib import Path

import pytest

from tanren_core.config import Config
from tanren_core.manager import WorkerManager
from tanren_core.schemas import Cli, Dispatch, Phase, Result


class TestManagerLifecycle:
    @pytest.mark.asyncio
    @pytest.mark.timeout(15)
    async def test_gate_dispatch_roundtrip(self, tmp_path: Path):
        """Write a gate dispatch, verify result is produced."""
        ipc_dir = tmp_path / "ipc"
        github_dir = tmp_path / "github"
        data_dir = tmp_path / "data"

        # Create project repo with a branch
        project_dir = github_dir / "test-project"
        project_dir.mkdir(parents=True)

        for cmd in [
            ["git", "init"],
            ["git", "config", "user.email", "test@test.com"],
            ["git", "config", "user.name", "Test"],
            ["git", "checkout", "-b", "main"],
        ]:
            proc = await asyncio.create_subprocess_exec(
                *cmd,
                cwd=str(project_dir),
                stdout=asyncio.subprocess.DEVNULL,
                stderr=asyncio.subprocess.DEVNULL,
            )
            await proc.wait()

        (project_dir / "README.md").write_text("# Test")
        for cmd in [
            ["git", "add", "."],
            ["git", "commit", "-m", "initial"],
            ["git", "branch", "feat-1"],
        ]:
            proc = await asyncio.create_subprocess_exec(
                *cmd,
                cwd=str(project_dir),
                stdout=asyncio.subprocess.DEVNULL,
                stderr=asyncio.subprocess.DEVNULL,
            )
            await proc.wait()

        config = Config(
            ipc_dir=str(ipc_dir),
            github_dir=str(github_dir),
            data_dir=str(data_dir),
            worktree_registry_path=str(data_dir / "worktrees.json"),
            poll_interval=0.5,
            heartbeat_interval=30.0,
            roles_config_path=str(tmp_path / "roles.yml"),
        )

        manager = WorkerManager(config)

        # Ensure input/ exists (manager creates dispatch/results/in-progress but not input/)
        (ipc_dir / "input").mkdir(parents=True, exist_ok=True)

        # Start manager in background
        manager_task = asyncio.create_task(manager.run())

        # Wait for setup
        await asyncio.sleep(1)

        # First: dispatch setup phase
        dispatch_dir = ipc_dir / "dispatch"
        setup_dispatch = Dispatch(
            workflow_id="wf-test-project-1-1000",
            phase=Phase.SETUP,
            project="test-project",
            spec_folder="tanren/specs/test",
            branch="feat-1",
            cli=Cli.BASH,
            model=None,
            gate_cmd=None,
            context=None,
            timeout=30,
        )
        setup_file = dispatch_dir / "1000-aaaaaa.json"
        setup_file.write_text(setup_dispatch.model_dump_json())

        # Wait for processing
        await asyncio.sleep(2)

        # Check setup result
        results_dir = ipc_dir / "results"
        result_files = list(results_dir.glob("*.json"))
        assert len(result_files) >= 1
        setup_result = Result.model_validate_json(result_files[0].read_text())
        assert setup_result.outcome == "success"
        assert setup_result.phase == "setup"

        # Verify worktree was created
        wt_path = github_dir / "test-project-wt-1"
        assert wt_path.exists()

        # Clean up results for next test
        for f in result_files:
            f.unlink()

        # Now dispatch a gate on the worktree
        # Create spec folder in worktree and commit (worktree must be clean)
        spec_dir = wt_path / "tanren" / "specs" / "test"
        spec_dir.mkdir(parents=True)
        (spec_dir / "spec.md").write_text("# Test Spec")
        (spec_dir / "plan.md").write_text("- [ ] Task 1: Do something\n- [x] Task 2: Done\n")

        for cmd in [
            ["git", "add", "."],
            ["git", "commit", "-m", "add spec files"],
        ]:
            proc = await asyncio.create_subprocess_exec(
                *cmd,
                cwd=str(wt_path),
                stdout=asyncio.subprocess.DEVNULL,
                stderr=asyncio.subprocess.DEVNULL,
            )
            await proc.wait()

        gate_dispatch = Dispatch(
            workflow_id="wf-test-project-1-1000",
            phase=Phase.GATE,
            project="test-project",
            spec_folder="tanren/specs/test",
            branch="feat-1",
            cli=Cli.BASH,
            model=None,
            gate_cmd="echo 'all tests passed'",
            context=None,
            timeout=30,
        )
        gate_file = dispatch_dir / "2000-bbbbbb.json"
        gate_file.write_text(gate_dispatch.model_dump_json())

        # Wait for processing
        await asyncio.sleep(2)

        # Check gate result
        result_files = list(results_dir.glob("*.json"))
        assert len(result_files) >= 1
        gate_result = Result.model_validate_json(result_files[0].read_text())
        assert gate_result.outcome == "success"
        assert gate_result.phase == "gate"
        assert gate_result.unchecked_tasks == 1
        assert gate_result.gate_output is not None
        assert "all tests passed" in gate_result.gate_output

        # Check nudge was written in coordinator message envelope format
        input_files = list((ipc_dir / "input").glob("*.json"))
        assert len(input_files) >= 1

        nudge_envelope = json.loads(input_files[0].read_text())
        assert nudge_envelope["type"] == "message"
        assert "text" in nudge_envelope
        nudge_inner = json.loads(nudge_envelope["text"])
        assert nudge_inner["type"] == "workflow_result"
        assert nudge_inner["workflow_id"] == "wf-test-project-1-1000"

        # Verify health file exists
        health_file = ipc_dir / "worker-health.json"
        assert health_file.exists()

        health = json.loads(health_file.read_text())
        assert health["alive"] is True
        assert "last_poll" in health
        assert "pid" in health

        # Verify gate result has pushed=None
        assert gate_result.pushed is None

        # Shutdown
        manager._signal_shutdown()
        await asyncio.wait_for(manager_task, timeout=5)

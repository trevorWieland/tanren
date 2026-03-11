"""Tests for remote_types module."""

import pytest
from pydantic import ValidationError

from worker_manager.adapters.remote_types import (
    BootstrapResult,
    RemoteAgentResult,
    RemoteResult,
    SecretBundle,
    VMAssignment,
    VMHandle,
    VMRequirements,
    WorkspacePath,
    WorkspaceSpec,
)


class TestImmutability:
    def test_vm_requirements_frozen(self):
        req = VMRequirements(profile="small")
        with pytest.raises(ValidationError, match="Instance is frozen"):
            req.cpu = 8  # type: ignore[misc]

    def test_vm_handle_frozen(self):
        handle = VMHandle(vm_id="v1", host="h", provider="manual", created_at="t")
        with pytest.raises(ValidationError, match="Instance is frozen"):
            handle.host = "other"  # type: ignore[misc]

    def test_workspace_spec_frozen(self):
        spec = WorkspaceSpec(project="proj", repo_url="url", branch="main")
        with pytest.raises(ValidationError, match="Instance is frozen"):
            spec.branch = "dev"  # type: ignore[misc]


class TestEquality:
    def test_vm_requirements_equal(self):
        a = VMRequirements(profile="small", cpu=4)
        b = VMRequirements(profile="small", cpu=4)
        assert a == b

    def test_vm_assignment_equal(self):
        kwargs = dict(
            vm_id="v1",
            workflow_id="w1",
            project="proj",
            spec="sp",
            host="h",
            assigned_at="t",
        )
        assert VMAssignment(**kwargs) == VMAssignment(**kwargs)

    def test_workspace_path_equal(self):
        a = WorkspacePath(path="/tmp/ws", project="proj", branch="main")
        b = WorkspacePath(path="/tmp/ws", project="proj", branch="main")
        assert a == b

    def test_remote_result_not_equal_different_exit_code(self):
        a = RemoteResult(exit_code=0, stdout="", stderr="")
        b = RemoteResult(exit_code=1, stdout="", stderr="")
        assert a != b


class TestDefaults:
    def test_vm_requirements_defaults(self):
        req = VMRequirements(profile="small")
        assert req.cpu == 2
        assert req.memory_gb == 4
        assert req.gpu is False
        assert req.labels == {}

    def test_vm_handle_defaults(self):
        handle = VMHandle(vm_id="v1", host="h", provider="manual", created_at="t")
        assert handle.labels == {}
        assert handle.hourly_cost is None

    def test_secret_bundle_defaults(self):
        bundle = SecretBundle()
        assert bundle.developer == {}
        assert bundle.project == {}
        assert bundle.infrastructure == {}

    def test_bootstrap_result_defaults(self):
        result = BootstrapResult()
        assert result.installed == ()
        assert result.skipped == ()
        assert result.duration_secs == 0

    def test_remote_result_timed_out_default(self):
        result = RemoteResult(exit_code=0, stdout="ok", stderr="")
        assert result.timed_out is False

    def test_remote_agent_result_signal_content_default(self):
        result = RemoteAgentResult(exit_code=0, stdout="ok", timed_out=False, duration_secs=10)
        assert result.signal_content == ""

    def test_remote_agent_result_stderr_default(self):
        result = RemoteAgentResult(exit_code=0, stdout="ok", timed_out=False, duration_secs=10)
        assert result.stderr == ""

    def test_remote_agent_result_stderr_preserved(self):
        result = RemoteAgentResult(
            exit_code=1,
            stdout="",
            timed_out=False,
            duration_secs=5,
            stderr="something went wrong",
        )
        assert result.stderr == "something went wrong"


class TestWorkspaceSpecTupleFields:
    def test_setup_commands_default_empty_tuple(self):
        spec = WorkspaceSpec(project="proj", repo_url="url", branch="main")
        assert spec.setup_commands == ()
        assert spec.teardown_commands == ()

    def test_setup_and_teardown_preserved(self):
        spec = WorkspaceSpec(
            project="proj",
            repo_url="url",
            branch="main",
            setup_commands=("pip install .", "make build"),
            teardown_commands=("make clean",),
        )
        assert spec.setup_commands == ("pip install .", "make build")
        assert spec.teardown_commands == ("make clean",)
        assert isinstance(spec.setup_commands, tuple)
        assert isinstance(spec.teardown_commands, tuple)

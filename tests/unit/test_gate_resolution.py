"""Tests for phase-aware gate command resolution."""

from tanren_core.env.environment_schema import EnvironmentProfile
from tanren_core.env.gates import resolve_gate_cmd
from tanren_core.schemas import Phase


def _profile(
    gate_cmd: str = "make check",
    task_gate_cmd: str | None = None,
    spec_gate_cmd: str | None = None,
) -> EnvironmentProfile:
    return EnvironmentProfile(
        name="test",
        gate_cmd=gate_cmd,
        task_gate_cmd=task_gate_cmd,
        spec_gate_cmd=spec_gate_cmd,
    )


class TestResolveGateCmd:
    # -- task-scoped phases use task_gate_cmd --

    def test_task_gate_cmd_used_for_do_task(self):
        p = _profile(task_gate_cmd="make unit")
        assert resolve_gate_cmd(p, Phase.DO_TASK) == "make unit"

    def test_task_gate_cmd_used_for_gate_phase(self):
        p = _profile(task_gate_cmd="make unit")
        assert resolve_gate_cmd(p, Phase.GATE) == "make unit"

    # -- spec-scoped phases use spec_gate_cmd --

    def test_spec_gate_cmd_used_for_run_demo(self):
        p = _profile(spec_gate_cmd="make e2e")
        assert resolve_gate_cmd(p, Phase.RUN_DEMO) == "make e2e"

    def test_spec_gate_cmd_used_for_audit_spec(self):
        p = _profile(spec_gate_cmd="make e2e")
        assert resolve_gate_cmd(p, Phase.AUDIT_SPEC) == "make e2e"

    def test_spec_gate_cmd_used_for_audit_task(self):
        p = _profile(spec_gate_cmd="make e2e")
        assert resolve_gate_cmd(p, Phase.AUDIT_TASK) == "make e2e"

    # -- fallback to gate_cmd when phase-specific not set --

    def test_fallback_to_gate_cmd_when_task_not_set(self):
        p = _profile(gate_cmd="make all")
        assert resolve_gate_cmd(p, Phase.DO_TASK) == "make all"

    def test_fallback_to_gate_cmd_when_spec_not_set(self):
        p = _profile(gate_cmd="make all")
        assert resolve_gate_cmd(p, Phase.RUN_DEMO) == "make all"

    # -- infrastructure phases always use gate_cmd --

    def test_gate_cmd_used_for_setup(self):
        p = _profile(gate_cmd="make all", task_gate_cmd="make unit", spec_gate_cmd="make e2e")
        assert resolve_gate_cmd(p, Phase.SETUP) == "make all"

    def test_gate_cmd_used_for_cleanup(self):
        p = _profile(gate_cmd="make all", task_gate_cmd="make unit", spec_gate_cmd="make e2e")
        assert resolve_gate_cmd(p, Phase.CLEANUP) == "make all"

    def test_gate_cmd_used_for_investigate(self):
        p = _profile(gate_cmd="make all", task_gate_cmd="make unit", spec_gate_cmd="make e2e")
        assert resolve_gate_cmd(p, Phase.INVESTIGATE) == "make all"

    # -- combined scenarios --

    def test_neither_set_uses_gate_cmd_for_all(self):
        p = _profile(gate_cmd="make check")
        for phase in Phase:
            assert resolve_gate_cmd(p, phase) == "make check"

    def test_both_set_each_used_for_respective_phases(self):
        p = _profile(gate_cmd="make all", task_gate_cmd="make unit", spec_gate_cmd="make e2e")
        assert resolve_gate_cmd(p, Phase.DO_TASK) == "make unit"
        assert resolve_gate_cmd(p, Phase.GATE) == "make unit"
        assert resolve_gate_cmd(p, Phase.RUN_DEMO) == "make e2e"
        assert resolve_gate_cmd(p, Phase.AUDIT_SPEC) == "make e2e"
        assert resolve_gate_cmd(p, Phase.AUDIT_TASK) == "make e2e"
        assert resolve_gate_cmd(p, Phase.SETUP) == "make all"
        assert resolve_gate_cmd(p, Phase.CLEANUP) == "make all"
        assert resolve_gate_cmd(p, Phase.INVESTIGATE) == "make all"

    def test_only_task_set_spec_falls_back(self):
        p = _profile(gate_cmd="make all", task_gate_cmd="make unit")
        assert resolve_gate_cmd(p, Phase.DO_TASK) == "make unit"
        assert resolve_gate_cmd(p, Phase.RUN_DEMO) == "make all"

    def test_only_spec_set_task_falls_back(self):
        p = _profile(gate_cmd="make all", spec_gate_cmd="make e2e")
        assert resolve_gate_cmd(p, Phase.DO_TASK) == "make all"
        assert resolve_gate_cmd(p, Phase.AUDIT_SPEC) == "make e2e"

"""Tests for store enum definitions and CLI-to-lane mapping."""

from tanren_core.schemas import Cli
from tanren_core.store.enums import (
    CLI_LANE_MAP,
    DispatchMode,
    DispatchStatus,
    Lane,
    StepStatus,
    StepType,
    cli_to_lane,
)


class TestLaneMapping:
    def test_opencode_maps_to_impl(self) -> None:
        assert cli_to_lane(Cli.OPENCODE) == Lane.IMPL

    def test_claude_maps_to_impl(self) -> None:
        assert cli_to_lane(Cli.CLAUDE) == Lane.IMPL

    def test_codex_maps_to_audit(self) -> None:
        assert cli_to_lane(Cli.CODEX) == Lane.AUDIT

    def test_bash_maps_to_gate(self) -> None:
        assert cli_to_lane(Cli.BASH) == Lane.GATE

    def test_cli_lane_map_string_keys(self) -> None:
        assert CLI_LANE_MAP["opencode"] == Lane.IMPL
        assert CLI_LANE_MAP["claude"] == Lane.IMPL
        assert CLI_LANE_MAP["codex"] == Lane.AUDIT
        assert CLI_LANE_MAP["bash"] == Lane.GATE


class TestEnumValues:
    def test_dispatch_mode_values(self) -> None:
        assert set(DispatchMode) == {"auto", "manual"}

    def test_step_type_values(self) -> None:
        assert set(StepType) == {"provision", "execute", "teardown", "dry_run"}

    def test_lane_values(self) -> None:
        assert set(Lane) == {"impl", "audit", "gate"}

    def test_dispatch_status_values(self) -> None:
        assert set(DispatchStatus) == {"pending", "running", "completed", "failed", "cancelled"}

    def test_step_status_values(self) -> None:
        assert set(StepStatus) == {"pending", "running", "completed", "failed", "cancelled"}

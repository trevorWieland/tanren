"""Phase-aware gate command resolution.

Resolves which gate command to use based on the triggering phase's scope.
See docs/architecture/phase-taxonomy.md for the multi-axis phase model.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

from tanren_core.schemas import Phase

if TYPE_CHECKING:
    from tanren_core.env.environment_schema import EnvironmentProfile

# Phases whose gate runs verify task-level work.
_TASK_PHASES: frozenset[Phase] = frozenset({Phase.DO_TASK, Phase.GATE})

# Phases whose gate runs verify spec-level work.
_SPEC_PHASES: frozenset[Phase] = frozenset({Phase.RUN_DEMO, Phase.AUDIT_SPEC, Phase.AUDIT_TASK})


def resolve_gate_cmd(profile: EnvironmentProfile, phase: Phase) -> str:
    """Resolve the gate command for a given triggering phase.

    Uses phase-specific overrides (task_gate_cmd, spec_gate_cmd) when set,
    falling back to the profile's default gate_cmd.

    Args:
        profile: The environment profile containing gate command configuration.
        phase: The triggering phase that preceded the gate dispatch.

    Returns:
        The resolved gate command string.
    """
    if phase in _TASK_PHASES and profile.task_gate_cmd is not None:
        return profile.task_gate_cmd
    if phase in _SPEC_PHASES and profile.spec_gate_cmd is not None:
        return profile.spec_gate_cmd
    return profile.gate_cmd

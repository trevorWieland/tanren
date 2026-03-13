"""Config endpoint — expose non-secret configuration."""

from typing import Annotated

from fastapi import APIRouter, Depends

from tanren_api.dependencies import get_config
from tanren_api.models import ConfigResponse
from tanren_core.config import Config

router = APIRouter(tags=["config"])


@router.get("/config")
async def get_configuration(
    config: Annotated[Config, Depends(get_config)],
) -> ConfigResponse:
    """Return non-secret config fields."""
    return ConfigResponse(
        ipc_dir=config.ipc_dir,
        github_dir=config.github_dir,
        poll_interval=config.poll_interval,
        heartbeat_interval=config.heartbeat_interval,
        max_opencode=config.max_opencode,
        max_codex=config.max_codex,
        max_gate=config.max_gate,
        events_enabled=config.events_db is not None,
        remote_enabled=config.remote_config_path is not None,
    )

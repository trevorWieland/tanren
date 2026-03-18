"""Config endpoint — expose non-secret configuration."""

from typing import Annotated

from fastapi import APIRouter, Depends

from tanren_api.dependencies import get_config
from tanren_api.models import ConfigResponse
from tanren_api.services import ConfigService
from tanren_core.config import Config

router = APIRouter(tags=["config"])


@router.get("/config")
async def get_configuration(
    config: Annotated[Config, Depends(get_config)],
) -> ConfigResponse:
    """Return non-secret config fields."""
    return await ConfigService(config).get()

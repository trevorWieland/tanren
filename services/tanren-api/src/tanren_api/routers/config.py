"""Config endpoint — expose non-secret configuration."""

from typing import Annotated

from fastapi import APIRouter, Depends

from tanren_api.dependencies import get_settings, get_state_store
from tanren_api.models import ConfigResponse
from tanren_api.services import ConfigService
from tanren_api.settings import APISettings
from tanren_core.store.protocols import StateStore

router = APIRouter(tags=["config"])


@router.get("/config")
async def get_configuration(
    settings: Annotated[APISettings, Depends(get_settings)],
    state_store: Annotated[StateStore, Depends(get_state_store)],
) -> ConfigResponse:
    """Return non-secret config fields."""
    return await ConfigService(settings, state_store).get()

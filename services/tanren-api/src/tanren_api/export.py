"""Generate openapi.json without starting a server."""

import json
from pathlib import Path

from tanren_api.main import create_app
from tanren_api.settings import APISettings


def main() -> None:
    """Export the OpenAPI spec to services/tanren-api/openapi.json."""
    settings = APISettings()
    app = create_app(settings)
    spec = app.openapi()
    output = Path(__file__).resolve().parent.parent.parent / "openapi.json"
    output.write_text(json.dumps(spec, indent=2) + "\n")
    print(f"Wrote {output}")

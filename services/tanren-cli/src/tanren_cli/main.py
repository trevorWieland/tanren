"""Top-level tanren CLI dispatcher."""

import typer

from tanren_cli.env_cli import env_app, secret_app
from tanren_cli.run_cli import run_app
from tanren_cli.vm_cli import vm_app
from tanren_core.config import load_config_env

tanren = typer.Typer(help="tanren - development lifecycle framework CLI.")
tanren.add_typer(env_app, name="env")
tanren.add_typer(secret_app, name="secret")
tanren.add_typer(vm_app, name="vm")
tanren.add_typer(run_app, name="run")


def main() -> None:
    """Load config and run the tanren CLI."""
    load_config_env()
    tanren()

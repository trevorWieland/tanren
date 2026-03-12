"""Top-level tanren CLI dispatcher."""

import typer

from worker_manager.config import load_config_env
from worker_manager.env.cli import env_app, secret_app
from worker_manager.run_cli import run_app
from worker_manager.vm_cli import vm_app

tanren = typer.Typer(help="tanren - development lifecycle framework CLI.")
tanren.add_typer(env_app, name="env")
tanren.add_typer(secret_app, name="secret")
tanren.add_typer(vm_app, name="vm")
tanren.add_typer(run_app, name="run")


def main() -> None:
    load_config_env()
    tanren()

"""Top-level tanren CLI dispatcher."""

import click

from worker_manager.env.cli import env, secret
from worker_manager.vm_cli import vm


@click.group()
def tanren():
    """tanren — development lifecycle framework CLI."""


tanren.add_command(env)
tanren.add_command(secret)
tanren.add_command(vm)


def main():
    tanren()

"""Top-level tanren CLI dispatcher."""

import click

from worker_manager.env.cli import env, secret


@click.group()
def tanren():
    """tanren — development lifecycle framework CLI."""


tanren.add_command(env)
tanren.add_command(secret)


def main():
    tanren()

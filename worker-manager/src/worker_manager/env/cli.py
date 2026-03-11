"""Click subcommands for env and secret management."""

import sys
from pathlib import Path

import click

from worker_manager.env.loader import (
    discover_env_vars_from_dotenv_example,
    load_env_layers,
    parse_tanren_yml,
)
from worker_manager.env.reporter import format_report, format_report_json
from worker_manager.env.schema import EnvBlock
from worker_manager.env.secrets import DEFAULT_SECRETS_DIR, list_secrets, set_secret
from worker_manager.env.validator import validate_env


@click.group()
def env():
    """Validate and manage environment variables."""


@env.command()
@click.option("--all", "check_all", is_flag=True, help="Validate all tanren.yml recursively")
@click.option("--verbose", is_flag=True, help="Include passing vars in output")
@click.option("--json", "json_output", is_flag=True, help="JSON output")
def check(check_all: bool, verbose: bool, json_output: bool):
    """Validate environment variables against tanren.yml."""
    if check_all:
        roots = list(Path.cwd().rglob("tanren.yml"))
        if not roots:
            click.echo("No tanren.yml files found", err=True)
            sys.exit(1)
    else:
        roots = [Path.cwd() / "tanren.yml"]

    any_failed = False

    for yml_path in roots:
        project_root = yml_path.parent
        config = parse_tanren_yml(project_root)

        if config is None:
            click.echo(f"Could not parse {yml_path}", err=True)
            any_failed = True
            continue

        env_block = config.env
        if env_block is None:
            # Fallback to .env.example
            required_vars = discover_env_vars_from_dotenv_example(project_root)
            if required_vars:
                env_block = EnvBlock(required=required_vars)
            else:
                if not json_output:
                    click.echo(f"No env requirements in {yml_path}")
                continue

        merged_env, source_map = load_env_layers(project_root)
        report = validate_env(env_block, merged_env, source_map)

        if json_output:
            click.echo(format_report_json(report))
        else:
            project_name = project_root.name
            click.echo(format_report(report, project_name, str(yml_path), verbose))

        if not report.passed:
            any_failed = True

    if any_failed:
        sys.exit(1)


@env.command()
def init():
    """Scaffold env block in tanren.yml from .env.example."""
    project_root = Path.cwd()
    yml_path = project_root / "tanren.yml"
    example_path = project_root / ".env.example"

    if not yml_path.exists():
        click.echo("No tanren.yml found in current directory", err=True)
        sys.exit(1)

    # Check if env block already exists
    config = parse_tanren_yml(project_root)
    if config and config.env:
        click.echo("tanren.yml already has an env block", err=True)
        sys.exit(1)

    if not example_path.exists():
        click.echo("No .env.example found — nothing to scaffold", err=True)
        sys.exit(1)

    # Parse .env.example keys
    from dotenv import dotenv_values

    values = dotenv_values(example_path)
    if not values:
        click.echo("No variables found in .env.example")
        return

    # Build YAML text via string concatenation (preserves comments in tanren.yml)
    lines = ["\nenv:"]
    lines.append("  on_missing: error")
    lines.append("  required:")

    for key in values:
        lines.append(f"    - key: {key}")
        lines.append('      description: ""')

    yaml_block = "\n".join(lines) + "\n"

    with open(yml_path, "a") as f:
        f.write(yaml_block)

    click.echo(f"Scaffolded env block with {len(values)} variables in {yml_path}")


@click.group()
def secret():
    """Manage secrets in secrets.env (default: $XDG_CONFIG_HOME/tanren/)."""


@secret.command("set")
@click.argument("key")
@click.argument("value")
def secret_set(key: str, value: str):
    """Store a secret in secrets.env (default: $XDG_CONFIG_HOME/tanren/)."""
    path = set_secret(key, value)
    click.echo(f"Secret {key} written to {path}")


@secret.command("list")
def secret_list():
    """List secrets with redacted values."""
    secrets_path = DEFAULT_SECRETS_DIR / "secrets.env"
    secrets = list_secrets()
    if not secrets:
        click.echo(f"No secrets found in {secrets_path}")
        return

    for key, redacted in secrets:
        click.echo(f"  {key} = {redacted}")

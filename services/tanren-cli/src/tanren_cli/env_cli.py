"""Typer subcommands for env and secret management."""

from pathlib import Path

import typer
from dotenv import dotenv_values

from tanren_core.env.loader import (
    discover_env_vars_from_dotenv_example,
    load_env_layers,
    parse_tanren_yml,
)
from tanren_core.env.reporter import format_report, format_report_json
from tanren_core.env.schema import EnvBlock
from tanren_core.env.secrets import DEFAULT_SECRETS_DIR, list_secrets, set_secret
from tanren_core.env.validator import validate_env

env_app = typer.Typer(help="Validate and manage environment variables.")
secret_app = typer.Typer(help="Manage secrets in secrets.env (default: $XDG_CONFIG_HOME/tanren/).")


@env_app.command("check")
def check(
    check_all: bool = typer.Option(False, "--all", help="Validate all tanren.yml recursively"),
    verbose: bool = typer.Option(False, "--verbose", help="Include passing vars in output"),
    json_output: bool = typer.Option(False, "--json", help="JSON output"),
) -> None:
    """Validate environment variables against tanren.yml.

    Raises:
        Exit: If no tanren.yml files are found or validation fails.
    """
    if check_all:
        roots = list(Path.cwd().rglob("tanren.yml"))
        if not roots:
            typer.echo("No tanren.yml files found", err=True)
            raise typer.Exit(code=1)
    else:
        roots = [Path.cwd() / "tanren.yml"]

    any_failed = False

    for yml_path in roots:
        project_root = yml_path.parent
        config = parse_tanren_yml(project_root)

        if config is None:
            typer.echo(f"Could not parse {yml_path}", err=True)
            any_failed = True
            continue

        env_block = config.env
        if env_block is None:
            # Fallback to .env.example.
            required_vars = discover_env_vars_from_dotenv_example(project_root)
            if required_vars:
                env_block = EnvBlock(required=required_vars)
            else:
                if not json_output:
                    typer.echo(f"No env requirements in {yml_path}")
                continue

        merged_env, source_map = load_env_layers(project_root)
        report = validate_env(env_block, merged_env, source_map)

        if json_output:
            typer.echo(format_report_json(report))
        else:
            project_name = project_root.name
            typer.echo(format_report(report, project_name, str(yml_path), verbose))

        if not report.passed:
            any_failed = True

    if any_failed:
        raise typer.Exit(code=1)


@env_app.command("init")
def init() -> None:
    """Scaffold env block in tanren.yml from .env.example.

    Raises:
        Exit: If tanren.yml is missing, already has an env block, or
            no .env.example exists.
    """
    project_root = Path.cwd()
    yml_path = project_root / "tanren.yml"
    example_path = project_root / ".env.example"

    if not yml_path.exists():
        typer.echo("No tanren.yml found in current directory", err=True)
        raise typer.Exit(code=1)

    config = parse_tanren_yml(project_root)
    if config and config.env:
        typer.echo("tanren.yml already has an env block", err=True)
        raise typer.Exit(code=1)

    if not example_path.exists():
        typer.echo("No .env.example found - nothing to scaffold", err=True)
        raise typer.Exit(code=1)

    values = dotenv_values(example_path)
    if not values:
        typer.echo("No variables found in .env.example")
        return

    lines = ["\nenv:"]
    lines.extend(("  on_missing: error", "  required:"))

    for key in values:
        lines.extend((f"    - key: {key}", '      description: ""'))

    yaml_block = "\n".join(lines) + "\n"

    with open(yml_path, "a") as file_obj:
        file_obj.write(yaml_block)

    typer.echo(f"Scaffolded env block with {len(values)} variables in {yml_path}")


@secret_app.command("set")
def secret_set(
    key: str = typer.Argument(...),
    value: str = typer.Argument(...),
) -> None:
    """Store a secret in secrets.env (default: $XDG_CONFIG_HOME/tanren/)."""
    path = set_secret(key, value)
    typer.echo(f"Secret {key} written to {path}")


@secret_app.command("list")
def secret_list() -> None:
    """List secrets with redacted values."""
    secrets_path = DEFAULT_SECRETS_DIR / "secrets.env"
    secrets = list_secrets()
    if not secrets:
        typer.echo(f"No secrets found in {secrets_path}")
        return

    for key, redacted in secrets:
        typer.echo(f"  {key} = {redacted}")


# Backward-compatible names imported by top-level CLI module.
env = env_app
secret = secret_app

"""tanren vm - manage remote VM assignments."""

from __future__ import annotations

import asyncio
import os
from pathlib import Path

import typer

from worker_manager.adapters.sqlite_vm_state import SqliteVMStateStore

vm_app = typer.Typer(help="Manage remote VM assignments.")


def _get_state_store() -> SqliteVMStateStore:
    """Create a VMStateStore from WM_DATA_DIR config."""
    data_dir = str(Path(os.environ.get("WM_DATA_DIR", "~/.local/share/tanren-worker")).expanduser())
    db_path = f"{data_dir}/vm-state.db"
    return SqliteVMStateStore(db_path)


@vm_app.command("list")
def vm_list() -> None:
    """Show active VM assignments."""

    async def _run() -> None:
        store = _get_state_store()
        try:
            assignments = await store.get_active_assignments()
            if not assignments:
                typer.echo("No active VM assignments.")
                return
            typer.echo(
                f"{'VM ID':<20} {'Host':<20} {'Workflow':<30} {'Project':<15} {'Assigned At'}"
            )
            typer.echo("-" * 100)
            for assignment in assignments:
                typer.echo(
                    f"{assignment.vm_id:<20} {assignment.host:<20} "
                    f"{assignment.workflow_id:<30} {assignment.project:<15} "
                    f"{assignment.assigned_at}"
                )
        finally:
            await store.close()

    asyncio.run(_run())


@vm_app.command("release")
def vm_release(vm_id: str = typer.Argument(...)) -> None:
    """Manually release a stuck VM assignment."""

    async def _run() -> None:
        store = _get_state_store()
        try:
            assignment = await store.get_assignment(vm_id)
            if assignment is None:
                typer.echo(f"No active assignment found for VM: {vm_id}", err=True)
                raise typer.Exit(code=1)
            await store.record_release(vm_id)
            typer.echo(f"Released VM {vm_id} (was assigned to {assignment.workflow_id})")
        finally:
            await store.close()

    asyncio.run(_run())


@vm_app.command("recover")
def vm_recover() -> None:
    """Run startup recovery: verify connectivity, release unreachable VMs."""

    async def _run() -> None:
        from worker_manager.adapters.ssh import SSHConfig, SSHConnection
        from worker_manager.remote_config import RemoteSSHConfig

        store = _get_state_store()
        try:
            assignments = await store.get_active_assignments()
            if not assignments:
                typer.echo("No active assignments to recover.")
                return

            typer.echo(f"Checking {len(assignments)} active assignment(s)...")

            remote_config_path = os.environ.get("WM_REMOTE_CONFIG")
            ssh_defaults: RemoteSSHConfig | None = None
            if remote_config_path:
                from worker_manager.remote_config import load_remote_config

                ssh_defaults = load_remote_config(remote_config_path).ssh

            for assignment in assignments:
                if ssh_defaults is not None:
                    ssh_config = SSHConfig(
                        host=assignment.host,
                        user=ssh_defaults.user,
                        key_path=ssh_defaults.key_path,
                        port=ssh_defaults.port,
                        connect_timeout=ssh_defaults.connect_timeout,
                    )
                else:
                    ssh_config = SSHConfig(host=assignment.host)
                conn = SSHConnection(ssh_config)
                try:
                    reachable = await conn.check_connection()
                    if reachable:
                        typer.echo(f"  {assignment.vm_id} ({assignment.host}): reachable")
                    else:
                        await store.record_release(assignment.vm_id)
                        typer.echo(
                            f"  {assignment.vm_id} ({assignment.host}): UNREACHABLE - released"
                        )
                except Exception:
                    await store.record_release(assignment.vm_id)
                    typer.echo(f"  {assignment.vm_id} ({assignment.host}): ERROR - released")
                finally:
                    await conn.close()
        finally:
            await store.close()

    asyncio.run(_run())


# Backward-compatible name imported by top-level CLI module.
vm = vm_app

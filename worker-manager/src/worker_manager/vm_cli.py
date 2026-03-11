"""tanren vm — manage remote VM assignments."""

from __future__ import annotations

import asyncio
import os
import sys
from pathlib import Path

import click

from worker_manager.adapters.sqlite_vm_state import SqliteVMStateStore


def _get_state_store() -> SqliteVMStateStore:
    """Create a VMStateStore from config."""
    data_dir = str(Path(os.environ.get(
        "WM_DATA_DIR", "~/.local/share/tanren-worker"
    )).expanduser())
    db_path = f"{data_dir}/vm-state.db"
    return SqliteVMStateStore(db_path)


@click.group()
def vm():
    """Manage remote VM assignments."""


@vm.command("list")
def vm_list():
    """Show active VM assignments."""

    async def _run():
        store = _get_state_store()
        try:
            assignments = await store.get_active_assignments()
            if not assignments:
                click.echo("No active VM assignments.")
                return
            click.echo(
                f"{'VM ID':<20} {'Host':<20} {'Workflow':<30} "
                f"{'Project':<15} {'Assigned At'}"
            )
            click.echo("-" * 100)
            for a in assignments:
                click.echo(
                    f"{a.vm_id:<20} {a.host:<20} "
                    f"{a.workflow_id:<30} {a.project:<15} "
                    f"{a.assigned_at}"
                )
        finally:
            await store.close()

    asyncio.run(_run())


@vm.command("release")
@click.argument("vm_id")
def vm_release(vm_id: str):
    """Manually release a stuck VM assignment."""

    async def _run():
        store = _get_state_store()
        try:
            assignment = await store.get_assignment(vm_id)
            if assignment is None:
                click.echo(f"No active assignment found for VM: {vm_id}")
                sys.exit(1)
            await store.record_release(vm_id)
            click.echo(
                f"Released VM {vm_id} "
                f"(was assigned to {assignment.workflow_id})"
            )
        finally:
            await store.close()

    asyncio.run(_run())


@vm.command("recover")
def vm_recover():
    """Run startup recovery: verify connectivity, release unreachable VMs."""

    async def _run():
        from worker_manager.adapters.ssh import SSHConfig, SSHConnection

        store = _get_state_store()
        try:
            assignments = await store.get_active_assignments()
            if not assignments:
                click.echo("No active assignments to recover.")
                return

            click.echo(f"Checking {len(assignments)} active assignment(s)...")

            for a in assignments:
                ssh_config = SSHConfig(host=a.host)
                conn = SSHConnection(ssh_config)
                try:
                    reachable = await conn.check_connection()
                    if reachable:
                        click.echo(f"  {a.vm_id} ({a.host}): reachable")
                    else:
                        await store.record_release(a.vm_id)
                        click.echo(
                            f"  {a.vm_id} ({a.host}): "
                            f"UNREACHABLE — released"
                        )
                except Exception:
                    await store.record_release(a.vm_id)
                    click.echo(
                        f"  {a.vm_id} ({a.host}): "
                        f"ERROR — released"
                    )
                finally:
                    await conn.close()
        finally:
            await store.close()

    asyncio.run(_run())

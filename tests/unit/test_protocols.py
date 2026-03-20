"""Verify each adapter class satisfies its protocol via isinstance checks."""

from typing import TYPE_CHECKING

from tanren_core.adapters import (
    DotenvEnvProvisioner,
    DotenvEnvValidator,
    DotenvSecretProvider,
    GitPostflightRunner,
    GitPreflightRunner,
    GitWorktreeManager,
    NullEventEmitter,
    SqliteEventEmitter,
    SubprocessSpawner,
)
from tanren_core.adapters.protocols import (
    EnvProvisioner,
    EnvValidator,
    EventEmitter,
    PostflightRunner,
    PreflightRunner,
    ProcessSpawner,
    SecretProvider,
    WorktreeManager,
)

if TYPE_CHECKING:
    from pathlib import Path


class TestProtocolConformance:
    def test_git_worktree_manager(self):
        assert isinstance(GitWorktreeManager(), WorktreeManager)

    def test_git_preflight_runner(self):
        assert isinstance(GitPreflightRunner(), PreflightRunner)

    def test_git_postflight_runner(self):
        assert isinstance(GitPostflightRunner(), PostflightRunner)

    def test_subprocess_spawner(self):
        assert isinstance(SubprocessSpawner(), ProcessSpawner)

    def test_dotenv_env_validator(self):
        assert isinstance(DotenvEnvValidator(), EnvValidator)

    def test_dotenv_env_provisioner(self):
        assert isinstance(DotenvEnvProvisioner(), EnvProvisioner)

    def test_null_event_emitter(self):
        assert isinstance(NullEventEmitter(), EventEmitter)

    def test_sqlite_event_emitter(self):
        assert isinstance(SqliteEventEmitter("/tmp/test.db"), EventEmitter)

    def test_dotenv_secret_provider(self, tmp_path: Path):
        assert isinstance(DotenvSecretProvider(secrets_dir=tmp_path), SecretProvider)

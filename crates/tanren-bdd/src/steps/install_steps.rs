//! Cucumber step bindings for install-flow fixtures and assertions.

use cucumber::{given, then, when};

use crate::TanrenWorld;
use crate::steps::install::{InstallContext, InstallStepResult};
use crate::steps::install_helpers;
use crate::steps::install_helpers::RepositoryRelativePath;

#[given(expr = "a clean repository fixture")]
#[given(expr = "a clean install repository fixture")]
fn given_clean_repository_fixture(world: &mut TanrenWorld) -> InstallStepResult<()> {
    world.install = Some(InstallContext::new()?);
    Ok(())
}

#[given(expr = "repository file {string} contains {string}")]
fn given_repository_file_contains(
    world: &mut TanrenWorld,
    path: String,
    content: String,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    let relative_path = RepositoryRelativePath::parse(path)?;
    ctx.write_fixture_file(&relative_path, content)
}

#[given(expr = "repository file {string} is seeded from workspace catalog")]
fn given_repository_file_seeded_from_catalog(
    world: &mut TanrenWorld,
    path: String,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    let relative_path = RepositoryRelativePath::parse(path)?;
    let content = install_helpers::read_workspace_catalog_file(relative_path.as_str())?;
    ctx.write_fixture_file(&relative_path, content)
}

#[given(expr = "repository file {string} baseline is recorded")]
fn given_repository_file_baseline(world: &mut TanrenWorld, path: String) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.record_baseline(RepositoryRelativePath::parse(path)?)
}

#[given(expr = "previous install manifest tracks stale generated file {string}")]
fn given_previous_manifest_tracks_stale_generated_file(
    world: &mut TanrenWorld,
    path: String,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    let relative_path = RepositoryRelativePath::parse(path)?;
    ctx.inject_manifest_stale_generated_entry(&relative_path)
}

#[given(expr = "previous install manifest is tampered with raw generated path {string}")]
fn given_previous_manifest_tampered_with_raw_generated_path(
    world: &mut TanrenWorld,
    raw_path: String,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    let raw_path = raw_path.into_boxed_str();
    ctx.inject_manifest_raw_generated_entry_path(raw_path.as_ref())
}

#[given(expr = "repository file {string} is deleted from the repository fixture")]
fn given_repository_file_deleted(world: &mut TanrenWorld, path: String) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    let relative_path = RepositoryRelativePath::parse(path)?;
    ctx.delete_fixture_file(&relative_path)
}

#[given(expr = "repository path {string} is replaced with a symlink to fixture path {string}")]
fn given_repository_path_replaced_with_symlink(
    world: &mut TanrenWorld,
    link_path: String,
    target_path: String,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    let link_path = RepositoryRelativePath::parse(link_path)?;
    let target_path = RepositoryRelativePath::parse(target_path)?;
    ctx.replace_fixture_path_with_directory_symlink(&link_path, &target_path)
}

#[when(expr = "tanren-cli install runs with profile {string}")]
async fn when_install_runs_with_profile(
    world: &mut TanrenWorld,
    profile: String,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.run_install(&profile, None).await
}

#[when(expr = "tanren-cli install runs with profile {string} and integrations {string}")]
async fn when_install_runs_with_profile_and_integrations(
    world: &mut TanrenWorld,
    profile: String,
    integrations: String,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.run_install(&profile, Some(integrations.as_str())).await
}

#[then(expr = "the install command succeeds")]
#[then(expr = "install command succeeds")]
fn then_install_command_succeeds(world: &mut TanrenWorld) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_success()
}

#[then(expr = "the install command exits nonzero")]
#[then(expr = "install command exits nonzero")]
fn then_install_command_exits_nonzero(world: &mut TanrenWorld) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_nonzero()
}

#[then(
    expr = "the install output reports created, updated, removed, restored, and preserved summaries"
)]
fn then_install_output_reports_summaries(world: &mut TanrenWorld) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_summary_output()
}

#[then(expr = "rust-cargo defaults install all methodology command assets and standards files")]
fn then_rust_cargo_defaults_install_all_assets(world: &mut TanrenWorld) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_rust_cargo_default_assets_installed()
}

#[then(expr = "the install manifest records the rust-cargo profile and default integrations")]
fn then_install_manifest_records_rust_cargo_defaults(
    world: &mut TanrenWorld,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_manifest_rust_cargo_defaults()
}

#[then(expr = "the install output reports a validation failure")]
fn then_install_output_reports_validation_failure(
    world: &mut TanrenWorld,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_validation_failure_output()
}

#[then(expr = "no files are written in the repository fixture")]
#[then(expr = "no files are written in repository fixture")]
fn then_no_files_are_written(world: &mut TanrenWorld) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_no_writes_since_last_run()
}

#[then(expr = "repository file {string} exists")]
fn then_repository_file_exists(world: &mut TanrenWorld, path: String) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_file_exists(&RepositoryRelativePath::parse(path)?)
}

#[then(expr = "repository file {string} does not exist")]
fn then_repository_file_absent(world: &mut TanrenWorld, path: String) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_file_absent(&RepositoryRelativePath::parse(path)?)
}

#[then(expr = "repository file {string} contains {string}")]
fn then_repository_file_contains(
    world: &mut TanrenWorld,
    path: String,
    content: String,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_exact_file_content(&RepositoryRelativePath::parse(path)?, content)
}

#[then(expr = "repository file {string} preserves its baseline content")]
fn then_repository_file_preserved(world: &mut TanrenWorld, path: String) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_file_content_preserved(&RepositoryRelativePath::parse(path)?)
}

#[then(expr = "repository file {string} is replaced from its baseline content")]
fn then_repository_file_replaced(world: &mut TanrenWorld, path: String) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_file_content_replaced(&RepositoryRelativePath::parse(path)?)
}

#[then(expr = "stale generated file {string} is removed")]
fn then_stale_generated_file_removed(
    world: &mut TanrenWorld,
    path: String,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_file_absent(&RepositoryRelativePath::parse(path)?)
}

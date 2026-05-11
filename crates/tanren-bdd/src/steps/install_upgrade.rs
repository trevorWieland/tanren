//! Cucumber step bindings for upgrade fixtures and assertions.

use cucumber::{given, then, when};

use crate::TanrenWorld;
use crate::steps::install::{InstallStepResult, RepositoryRelativePath};

#[given(
    expr = "an installed repository fixture snapshot {string} with profile {string} and integrations {string}"
)]
async fn given_installed_repository_fixture_snapshot_with_integrations(
    world: &mut TanrenWorld,
    snapshot_label: String,
    profile: String,
    integrations: String,
) -> InstallStepResult<()> {
    let snapshot_label = snapshot_label.into_boxed_str();
    world
        .seed_upgrade_fixture_from_install(
            &profile,
            Some(integrations.as_str()),
            snapshot_label.as_ref(),
        )
        .await
}

#[given(expr = "an installed repository fixture snapshot {string} with profile {string}")]
async fn given_installed_repository_fixture_snapshot(
    world: &mut TanrenWorld,
    snapshot_label: String,
    profile: String,
) -> InstallStepResult<()> {
    let snapshot_label = snapshot_label.into_boxed_str();
    world
        .seed_upgrade_fixture_from_install(&profile, None, snapshot_label.as_ref())
        .await
}

#[given(expr = "legacy standards path {string} is marked as a migration concern")]
fn given_legacy_standards_path_marked_as_migration_concern(
    world: &mut TanrenWorld,
    path: String,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    let relative_path = RepositoryRelativePath::parse(path)?;
    ctx.inject_legacy_standards_migration_concern(&relative_path)
}

#[given(expr = "repository snapshot {string} is captured")]
fn given_repository_snapshot_captured(
    world: &mut TanrenWorld,
    snapshot_label: String,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    let snapshot_label = snapshot_label.into_boxed_str();
    ctx.capture_labeled_snapshot(snapshot_label.as_ref())
}

#[when(expr = "tanren-cli upgrade preview runs")]
async fn when_upgrade_preview_runs(world: &mut TanrenWorld) -> InstallStepResult<()> {
    world.run_upgrade(false).await?;
    let ctx = world.ensure_install_ctx()?;
    ctx.capture_preview_id_from_stdout();
    Ok(())
}

#[when(expr = "tanren-cli upgrade apply runs with confirmation")]
async fn when_upgrade_apply_runs(world: &mut TanrenWorld) -> InstallStepResult<()> {
    world.run_upgrade(true).await
}

#[then(expr = "the upgrade preview is reported")]
fn then_upgrade_preview_reported(world: &mut TanrenWorld) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_upgrade_preview_output()
}

#[then(expr = "the upgrade preview command succeeds")]
fn then_upgrade_preview_command_succeeds(world: &mut TanrenWorld) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_success()
}

#[then(expr = "the upgrade command requests confirmation")]
fn then_upgrade_confirmation_required(world: &mut TanrenWorld) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_upgrade_confirmation_required_output()
}

#[then(expr = "the upgrade apply command succeeds")]
fn then_upgrade_apply_succeeds(world: &mut TanrenWorld) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_success()?;
    ctx.assert_upgrade_apply_output()
}

#[then(expr = "the upgrade apply command reports noop")]
fn then_upgrade_apply_reports_noop(world: &mut TanrenWorld) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_success()?;
    ctx.assert_upgrade_noop_output()
}

#[then(expr = "the upgrade preview lists compatibility concern {string}")]
fn then_upgrade_preview_lists_compatibility_concern(
    world: &mut TanrenWorld,
    concern: String,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    let concern = concern.into_boxed_str();
    ctx.assert_upgrade_preview_contains_concern(concern.as_ref())
}

#[then(expr = "the upgrade preview lists path {string}")]
fn then_upgrade_preview_lists_path(world: &mut TanrenWorld, path: String) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    let relative_path = RepositoryRelativePath::parse(path)?;
    ctx.assert_upgrade_preview_contains_path(&relative_path)
}

#[then(expr = "the upgrade apply lists path {string}")]
fn then_upgrade_apply_lists_path(world: &mut TanrenWorld, path: String) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    let relative_path = RepositoryRelativePath::parse(path)?;
    ctx.assert_upgrade_apply_contains_path(&relative_path)
}

#[then(expr = "the repository matches snapshot {string}")]
fn then_repository_matches_snapshot(
    world: &mut TanrenWorld,
    snapshot_label: String,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    let snapshot_label = snapshot_label.into_boxed_str();
    ctx.assert_repository_matches_labeled_snapshot(snapshot_label.as_ref())
}

#[then(expr = "the upgrade preview and apply share the same preview id")]
fn then_upgrade_preview_apply_share_preview_id(world: &mut TanrenWorld) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_preview_apply_preview_id_correlation()
}

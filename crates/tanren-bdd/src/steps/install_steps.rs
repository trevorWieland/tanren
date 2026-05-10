//! Cucumber step bindings for install-flow fixtures and assertions.

use cucumber::{given, then, when};

use crate::TanrenWorld;
use crate::steps::install::{
    InstallStepResult, RepositoryRelativePath, read_workspace_catalog_file,
};

const DEFAULT_UNINSTALL_USER_SPEC_PATH: &str = "docs/behaviors/user-uninstall-notes.md";
const DEFAULT_UNINSTALL_USER_SOURCE_PATH: &str =
    "crates/tanren-cli-app/src/user_uninstall_notes.rs";
const DEFAULT_UNINSTALL_STANDARDS_PATH: &str =
    "profiles/rust-cargo/global/dependency-management.md";

#[given(expr = "a clean repository fixture")]
#[given(expr = "a clean install repository fixture")]
fn given_clean_repository_fixture(world: &mut TanrenWorld) -> InstallStepResult<()> {
    world.reset_install_ctx()
}

#[given(expr = "an installed repository fixture")]
async fn given_installed_repository_fixture(world: &mut TanrenWorld) -> InstallStepResult<()> {
    world.reset_install_ctx()?;
    world.run_install("rust-cargo", None).await?;
    world.ensure_install_ctx()?.assert_success()
}

#[given(expr = "a repository with Tanren-managed assets")]
async fn given_repository_with_tanren_managed_assets(
    world: &mut TanrenWorld,
) -> InstallStepResult<()> {
    given_installed_repository_fixture(world).await
}

#[given(expr = "a repository with Tanren-managed assets and user-owned files")]
async fn given_repository_with_tanren_managed_assets_and_user_owned_files(
    world: &mut TanrenWorld,
) -> InstallStepResult<()> {
    given_installed_repository_fixture(world).await?;
    let ctx = world.ensure_install_ctx()?;
    let spec_path = RepositoryRelativePath::parse(DEFAULT_UNINSTALL_USER_SPEC_PATH.to_owned())?;
    let source_path = RepositoryRelativePath::parse(DEFAULT_UNINSTALL_USER_SOURCE_PATH.to_owned())?;
    let standards_path =
        RepositoryRelativePath::parse(DEFAULT_UNINSTALL_STANDARDS_PATH.to_owned())?;

    ctx.write_fixture_file(&spec_path, "team-owned spec evidence".to_owned())?;
    ctx.record_baseline(spec_path)?;
    ctx.write_fixture_file(&source_path, "team-owned source evidence".to_owned())?;
    ctx.record_baseline(source_path)?;
    ctx.write_fixture_file(
        &standards_path,
        "custom standards baseline for uninstall witness".to_owned(),
    )?;
    ctx.record_baseline(standards_path)?;
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

#[given(expr = "user-owned spec file {string} contains {string}")]
fn given_user_owned_spec_file_contains(
    world: &mut TanrenWorld,
    path: String,
    content: String,
) -> InstallStepResult<()> {
    given_repository_file_contains(world, path, content)
}

#[given(expr = "user-owned source file {string} contains {string}")]
fn given_user_owned_source_file_contains(
    world: &mut TanrenWorld,
    path: String,
    content: String,
) -> InstallStepResult<()> {
    given_repository_file_contains(world, path, content)
}

#[given(expr = "standards file {string} contains {string}")]
fn given_standards_file_contains(
    world: &mut TanrenWorld,
    path: String,
    content: String,
) -> InstallStepResult<()> {
    given_repository_file_contains(world, path, content)
}

#[given(expr = "repository file {string} is seeded from workspace catalog")]
fn given_repository_file_seeded_from_catalog(
    world: &mut TanrenWorld,
    path: String,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    let relative_path = RepositoryRelativePath::parse(path)?;
    let content = read_workspace_catalog_file(relative_path.as_str())?;
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

#[given(expr = "previous install manifest is tampered with an invalid content hash entry")]
fn given_previous_manifest_tampered_with_invalid_content_hash(
    world: &mut TanrenWorld,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.inject_manifest_invalid_content_hash_entry()
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

#[given(expr = "repository path {string} is replaced with a file symlink to fixture path {string}")]
fn given_repository_path_replaced_with_file_symlink(
    world: &mut TanrenWorld,
    link_path: String,
    target_path: String,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    let link_path = RepositoryRelativePath::parse(link_path)?;
    let target_path = RepositoryRelativePath::parse(target_path)?;
    ctx.replace_fixture_path_with_file_symlink(&link_path, &target_path)
}

#[when(expr = "tanren-cli install runs with profile {string}")]
async fn when_install_runs_with_profile(
    world: &mut TanrenWorld,
    profile: String,
) -> InstallStepResult<()> {
    world.run_install(&profile, None).await
}

#[when(expr = "tanren-cli install runs with profile {string} and integrations {string}")]
async fn when_install_runs_with_profile_and_integrations(
    world: &mut TanrenWorld,
    profile: String,
    integrations: String,
) -> InstallStepResult<()> {
    world
        .run_install(&profile, Some(integrations.as_str()))
        .await
}

#[when(expr = "tanren-cli uninstall preview runs without confirmation")]
async fn when_uninstall_preview_runs_without_confirmation(
    world: &mut TanrenWorld,
) -> InstallStepResult<()> {
    world.run_uninstall_preview().await
}

#[when(expr = "tanren-cli uninstall runs with confirmation")]
async fn when_uninstall_runs_with_confirmation(world: &mut TanrenWorld) -> InstallStepResult<()> {
    world.run_uninstall_apply().await
}

#[when(expr = "uninstall preview runs through the {word} interface")]
async fn when_uninstall_preview_runs_through_interface(
    world: &mut TanrenWorld,
    interface: String,
) -> InstallStepResult<()> {
    world.assert_active_harness_interface(interface.as_str())?;
    world.run_uninstall_preview().await
}

#[when(expr = "uninstall apply runs through the {word} interface")]
async fn when_uninstall_apply_runs_through_interface(
    world: &mut TanrenWorld,
    interface: String,
) -> InstallStepResult<()> {
    world.assert_active_harness_interface(interface.as_str())?;
    world.run_uninstall_apply().await
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

#[then(expr = "the uninstall command succeeds")]
fn then_uninstall_command_succeeds(world: &mut TanrenWorld) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_success()
}

#[then(
    expr = "the install output reports created, updated, removed, restored, and preserved summaries"
)]
fn then_install_output_reports_summaries(world: &mut TanrenWorld) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_summary_output()
}

#[then(expr = "the uninstall preview output reports remove, preserve, and warning path lists")]
fn then_uninstall_preview_output_reports_summary(world: &mut TanrenWorld) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_uninstall_preview_output()
}

#[then(expr = "the uninstall apply output reports removed generated and metadata summaries")]
fn then_uninstall_apply_output_reports_summary(world: &mut TanrenWorld) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_uninstall_apply_output()
}

#[then(expr = "the uninstall preview lists removable path {string}")]
fn then_uninstall_preview_lists_removable_path(
    world: &mut TanrenWorld,
    path: String,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    let relative_path = RepositoryRelativePath::parse(path)?;
    ctx.assert_uninstall_preview_lists_removal_path(&relative_path)
}

#[then(expr = "the uninstall preview lists preserved path {string}")]
fn then_uninstall_preview_lists_preserved_path(
    world: &mut TanrenWorld,
    path: String,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    let relative_path = RepositoryRelativePath::parse(path)?;
    ctx.assert_uninstall_preview_lists_preserved_path(&relative_path)
}

#[then(expr = "the uninstall apply output lists removed generated path {string}")]
fn then_uninstall_apply_lists_removed_generated_path(
    world: &mut TanrenWorld,
    path: String,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    let relative_path = RepositoryRelativePath::parse(path)?;
    ctx.assert_uninstall_apply_lists_removed_generated_path(&relative_path)
}

#[then(expr = "the uninstall preview includes at least one removable Tanren-managed path")]
fn then_uninstall_preview_includes_removable_path(
    world: &mut TanrenWorld,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_uninstall_preview_has_removals()
}

#[then(expr = "the uninstall output reports nothing to uninstall")]
fn then_uninstall_output_reports_nothing_to_uninstall(
    world: &mut TanrenWorld,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_uninstall_nothing_to_uninstall(true)
}

#[then(expr = "the uninstall output reports nothing reason {word}")]
fn then_uninstall_output_reports_nothing_reason(
    world: &mut TanrenWorld,
    reason: String,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    let reason = reason.into_boxed_str();
    ctx.assert_uninstall_nothing_reason(reason.as_ref())
}

#[then(expr = "the preview includes at least one removable Tanren-managed path")]
fn then_preview_includes_removable_path(world: &mut TanrenWorld) -> InstallStepResult<()> {
    then_uninstall_preview_includes_removable_path(world)
}

#[then(expr = "the uninstall preview preserves user-owned files")]
fn then_uninstall_preview_preserves_user_owned_files(
    world: &mut TanrenWorld,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_uninstall_preview_leaves_repository_snapshot_unchanged()?;
    ctx.assert_uninstall_preserves_spec_baseline_file_content(&RepositoryRelativePath::parse(
        DEFAULT_UNINSTALL_USER_SPEC_PATH.to_owned(),
    )?)?;
    ctx.assert_uninstall_preserves_source_signal_baseline_file_content(
        &RepositoryRelativePath::parse(DEFAULT_UNINSTALL_USER_SOURCE_PATH.to_owned())?,
    )?;
    ctx.assert_uninstall_preserves_standards_baseline_file_content(
        &RepositoryRelativePath::parse(DEFAULT_UNINSTALL_STANDARDS_PATH.to_owned())?,
    )?;
    Ok(())
}

#[then(expr = "the preview preserves user-owned files")]
fn then_preview_preserves_user_owned_files(world: &mut TanrenWorld) -> InstallStepResult<()> {
    then_uninstall_preview_preserves_user_owned_files(world)
}

#[then(expr = "the uninstall apply removes generated assets and install metadata")]
fn then_uninstall_apply_removes_generated_assets_and_manifest(
    world: &mut TanrenWorld,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_uninstall_removed_generated_assets_and_manifest()
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

#[then(expr = "the install output redacts absolute repository paths")]
fn then_install_output_redacts_absolute_repository_paths(
    world: &mut TanrenWorld,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_no_absolute_repository_path_leaked()
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
    ctx.assert_uninstall_preserves_baseline_file_content(&RepositoryRelativePath::parse(path)?)
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

//! BDD steps for standards runtime checks via the real CLI binary.

use cucumber::{given, then, when};

use crate::TanrenWorld;
use crate::steps::install::{InstallStepResult, RepositoryRelativePath};

#[when(expr = "tanren-cli standards inspect runs")]
async fn when_standards_inspect_runs(world: &mut TanrenWorld) -> InstallStepResult<()> {
    world.run_standards_inspect().await
}

#[given(expr = "the configured standards directory is deleted from the repository fixture")]
#[given(
    expr = "the configured standards projection directory is deleted from the repository fixture"
)]
fn given_configured_standards_directory_deleted(world: &mut TanrenWorld) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.delete_configured_standards_directory()
}

#[given(expr = "an installed standards markdown file is corrupted")]
#[given(expr = "an installed standards projection markdown file is corrupted")]
fn given_installed_standards_markdown_file_corrupted(
    world: &mut TanrenWorld,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.corrupt_installed_standards_markdown_file()
}

#[given(expr = "standards assets are moved to repository path {string}")]
#[given(expr = "standards projection assets are moved to repository path {string}")]
fn given_standards_assets_moved_to_path(
    world: &mut TanrenWorld,
    standards_root: String,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    let standards_root = RepositoryRelativePath::parse(standards_root)?;
    ctx.move_standards_assets_to_root(&standards_root)
}

#[then(expr = "standards inspect stdout reports configured standards metadata")]
#[then(expr = "standards inspect stdout reports effective configuration metadata")]
fn then_standards_inspect_stdout_reports_metadata(
    world: &mut TanrenWorld,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_standards_inspect_report_output()
}

#[then(expr = "the standards inspect stderr reports missing standards")]
#[then(expr = "the standards inspect stderr reports missing configured standards projection")]
fn then_standards_inspect_stderr_reports_missing(world: &mut TanrenWorld) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_standards_missing_output()
}

#[then(expr = "the standards inspect stderr reports standards parse failure")]
#[then(expr = "the standards inspect stderr reports malformed standards projection frontmatter")]
fn then_standards_inspect_stderr_reports_parse_failure(
    world: &mut TanrenWorld,
) -> InstallStepResult<()> {
    let ctx = world.ensure_install_ctx()?;
    ctx.assert_standards_parse_failure_output()
}

//! Asset-upgrade step definitions for B-0134.
//!
//! Step bodies dispatch through the per-interface
//! [`UpgradeHarness`](tanren_testkit::UpgradeHarness) trait — never
//! `tanren_app_services::preview_upgrade` / `apply_upgrade` directly.
//! The active harness is selected by the BDD `Before` hook from the
//! scenario's tags (`@api`, `@cli`, `@mcp`, `@tui`, `@web`, or fallback
//! in-process). `xtask check-bdd-wire-coverage` mechanically rejects
//! any future step that bypasses this seam.

use cucumber::{given, then, when};
use tanren_contract::AssetAction;

use crate::TanrenWorld;

#[given(expr = "a repository with Tanren assets installed at version {string}")]
async fn given_installed_repo(world: &mut TanrenWorld, version: String) {
    let _ = version;
    let _ = world.ensure_upgrade_ctx().await;
}

#[when(expr = "the user previews the upgrade")]
async fn when_preview_upgrade(world: &mut TanrenWorld) {
    let ctx = world.ensure_upgrade_ctx().await;
    let root = ctx.fixture.root().to_path_buf();
    let response = ctx
        .harness
        .upgrade_preview(&root)
        .await
        .expect("upgrade preview must succeed");
    ctx.last_preview = Some(response);
}

#[when(expr = "the user confirms and applies the upgrade")]
async fn when_confirm_apply_upgrade(world: &mut TanrenWorld) {
    let ctx = world.ensure_upgrade_ctx().await;
    let root = ctx.fixture.root().to_path_buf();
    let response = ctx
        .harness
        .upgrade_apply(&root)
        .await
        .expect("upgrade apply must succeed");
    ctx.last_preview = Some(response);
}

#[then(expr = "the preview includes actions to create, update, and remove generated assets")]
async fn then_preview_includes_actions(world: &mut TanrenWorld) {
    let ctx = world.ensure_upgrade_ctx().await;
    let preview = ctx.last_preview.as_ref().expect("preview must exist");
    let has_create = preview
        .actions
        .iter()
        .any(|a| matches!(a, AssetAction::Create { .. }));
    let has_update = preview
        .actions
        .iter()
        .any(|a| matches!(a, AssetAction::Update { .. }));
    let has_remove = preview
        .actions
        .iter()
        .any(|a| matches!(a, AssetAction::Remove { .. }));
    assert!(has_create, "preview must include a create action");
    assert!(has_update, "preview must include an update action");
    assert!(has_remove, "preview must include a remove action");
}

#[then(expr = "the preview reports migration concerns")]
async fn then_preview_reports_concerns(world: &mut TanrenWorld) {
    let ctx = world.ensure_upgrade_ctx().await;
    let preview = ctx.last_preview.as_ref().expect("preview must exist");
    assert!(
        !preview.concerns.is_empty(),
        "preview must report at least one migration concern"
    );
}

#[then(expr = "the preview lists user-owned paths as preserved")]
async fn then_preview_preserves_user_paths(world: &mut TanrenWorld) {
    let ctx = world.ensure_upgrade_ctx().await;
    let preview = ctx.last_preview.as_ref().expect("preview must exist");
    assert!(
        !preview.preserved_user_paths.is_empty(),
        "preview must list preserved user paths"
    );
}

#[then(expr = "generated assets are updated to the target version")]
async fn then_assets_updated(world: &mut TanrenWorld) {
    let ctx = world.ensure_upgrade_ctx().await;
    let response = ctx
        .last_preview
        .as_ref()
        .expect("apply response must exist");
    assert_ne!(
        response.source_version, response.target_version,
        "source and target versions must differ"
    );
    let config_path = ctx.fixture.root().join(".tanren/config.toml");
    let content =
        std::fs::read_to_string(&config_path).expect("config.toml must exist after upgrade");
    assert_eq!(
        content, "# Tanren configuration\n",
        "generated asset must reflect target version content"
    );
}

#[then(expr = "user-owned files are unchanged")]
async fn then_user_files_unchanged(world: &mut TanrenWorld) {
    let ctx = world.ensure_upgrade_ctx().await;
    let user_path = ctx.fixture.root().join("standards/team-policy.md");
    let content = std::fs::read_to_string(&user_path).expect("user-owned file must exist");
    assert_eq!(
        content, "# Team policy\n",
        "user-owned file must remain unchanged"
    );
}

#[then(expr = "the repository remains at the installed version")]
async fn then_repo_unchanged(world: &mut TanrenWorld) {
    let ctx = world.ensure_upgrade_ctx().await;
    let config_path = ctx.fixture.root().join(".tanren/config.toml");
    let content = std::fs::read_to_string(&config_path).expect("config.toml must exist");
    assert_eq!(
        content, "# Tanren configuration (old)\n",
        "generated asset must be unchanged after preview-only"
    );
    let retired_path = ctx.fixture.root().join("commands/retired.md");
    assert!(
        retired_path.is_file(),
        "retired asset must still exist after preview-only"
    );
}

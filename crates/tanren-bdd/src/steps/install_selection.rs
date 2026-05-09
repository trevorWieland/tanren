//! Additional install-flow steps focused on integration selection behavior.

use cucumber::then;

use crate::TanrenWorld;

#[then(expr = "rust-cargo profile standards files are installed")]
fn then_rust_cargo_standards_installed(world: &mut TanrenWorld) {
    let ctx = world.ensure_install_ctx();
    ctx.assert_rust_cargo_standards_installed();
}

#[then(expr = "only integrations {string} command assets are installed")]
fn then_only_selected_integration_assets_installed(world: &mut TanrenWorld, integrations: String) {
    let ctx = world.ensure_install_ctx();
    let integrations = integrations.into_boxed_str();
    ctx.assert_selected_integration_command_assets(integrations.as_ref());
}

#[then(expr = "the install stderr contains {string}")]
fn then_install_stderr_contains(world: &mut TanrenWorld, expected: String) {
    let ctx = world.ensure_install_ctx();
    let expected = expected.into_boxed_str();
    ctx.assert_stderr_contains(expected.as_ref());
}

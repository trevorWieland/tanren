//! Install-flow step definitions for B-0068 / B-0070.
//!
//! Step bodies dispatch through the [`InstallHarness`] in
//! `tanren-testkit`, which shells out to the compiled `tanren-cli`
//! binary — never in-process installer functions.

use cucumber::{given, then, when};

use crate::TanrenWorld;

#[given(expr = "a temporary repository")]
fn given_temp_repo(world: &mut TanrenWorld) {
    let _ = world.ensure_install_ctx();
}

#[when(expr = "the installer runs with profile {string}")]
async fn when_install_profile(world: &mut TanrenWorld, profile: String) {
    let ctx = world.ensure_install_ctx();
    ctx.harness
        .run_install(&profile, None)
        .await
        .expect("install invocation must not fail at transport level");
}

#[when(expr = "the installer runs with profile {string} and integrations {string}")]
async fn when_install_profile_integrations(
    world: &mut TanrenWorld,
    profile: String,
    integrations: String,
) {
    let ctx = world.ensure_install_ctx();
    let list: Vec<&str> = integrations.split(',').map(str::trim).collect();
    ctx.harness
        .run_install(&profile, Some(&list))
        .await
        .expect("install invocation must not fail at transport level");
}

#[then(expr = "the installer succeeds")]
fn then_install_succeeds(world: &mut TanrenWorld) {
    let ctx = world.ensure_install_ctx();
    let inv = ctx
        .harness
        .last_invocation()
        .expect("install must have been invoked");
    assert!(
        inv.exit_success,
        "expected install to succeed, but it failed with: {}",
        inv.stderr.trim()
    );
}

#[then(expr = "the installer fails")]
fn then_install_fails(world: &mut TanrenWorld) {
    let ctx = world.ensure_install_ctx();
    let inv = ctx
        .harness
        .last_invocation()
        .expect("install must have been invoked");
    assert!(
        !inv.exit_success,
        "expected install to fail, but it succeeded with: {}",
        inv.stdout.trim()
    );
}

#[then(expr = "the installer fails with message containing {string}")]
fn then_install_fails_with_message(world: &mut TanrenWorld, fragment: String) {
    let ctx = world.ensure_install_ctx();
    let inv = ctx
        .harness
        .last_invocation()
        .expect("install must have been invoked");
    assert!(
        !inv.exit_success,
        "expected install to fail, but it succeeded with: {}",
        inv.stdout.trim()
    );
    let combined = format!("{}{}", inv.stdout, inv.stderr);
    assert!(
        combined.contains(&fragment),
        "expected stderr/stdout to contain '{fragment}', got: {combined}"
    );
    drop(fragment);
}

#[given(expr = "a file {string} exists in the repository with content")]
fn given_file_with_content(world: &mut TanrenWorld, step: &cucumber::gherkin::Step, path: String) {
    let ctx = world.ensure_install_ctx();
    let content = step
        .docstring
        .as_deref()
        .expect("step must have a docstring");
    ctx.harness
        .write_file(&path, content)
        .expect("write file must succeed");
    drop(path);
}

#[when(expr = "the file {string} is modified to contain")]
fn when_file_modified(world: &mut TanrenWorld, step: &cucumber::gherkin::Step, path: String) {
    let ctx = world.ensure_install_ctx();
    let content = step
        .docstring
        .as_deref()
        .expect("step must have a docstring");
    ctx.harness
        .write_file(&path, content)
        .expect("modify file must succeed");
    drop(path);
}

#[when(expr = "the file {string} is deleted")]
fn when_file_deleted(world: &mut TanrenWorld, path: String) {
    let ctx = world.ensure_install_ctx();
    let full_path = ctx.harness.repo_dir().join(&path);
    std::fs::remove_file(&full_path).expect("delete file must succeed");
    drop(path);
}

#[then(expr = "the file {string} exists in the repository")]
fn then_file_exists(world: &mut TanrenWorld, path: String) {
    let ctx = world.ensure_install_ctx();
    assert!(
        ctx.harness.file_exists(&path),
        "expected file '{path}' to exist in the repository"
    );
    drop(path);
}

#[then(expr = "the file {string} does not exist in the repository")]
fn then_file_not_exists(world: &mut TanrenWorld, path: String) {
    let ctx = world.ensure_install_ctx();
    assert!(
        !ctx.harness.file_exists(&path),
        "expected file '{path}' to NOT exist in the repository"
    );
    drop(path);
}

#[then(expr = "the file {string} in the repository contains")]
fn then_file_contains(world: &mut TanrenWorld, step: &cucumber::gherkin::Step, path: String) {
    let ctx = world.ensure_install_ctx();
    let content = ctx.harness.read_file(&path).expect("file must be readable");
    let expected = step
        .docstring
        .as_deref()
        .expect("step must have a docstring");
    assert!(
        content.contains(expected),
        "expected file '{path}' to contain '{expected}', got: {content}"
    );
    drop(path);
}

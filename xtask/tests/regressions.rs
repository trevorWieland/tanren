//! Regression-fixture suite for the xtask enforcement guards.
//!
//! Each `#[test]` below points one xtask subcommand at the matching
//! synthetic source tree under `xtask/tests/fixtures/<guard>/` and
//! asserts the guard exits non-zero with the expected error message
//! substring. The fixtures are intentionally minimal Rust trees that
//! violate exactly one rule — they do not have to compile or be valid
//! Cargo packages, only valid input for the AST/text walkers each
//! guard runs.
//!
//! If a fixture stops failing its guard, the guard has been weakened —
//! investigate before merging the change. This file is the only
//! `#[test]`-bearing module the workspace permits outside `tanren-bdd`;
//! `check-rust-test-surface` skips `xtask/tests/` for that reason.

use std::path::PathBuf;
use std::process::Command;

fn xtask_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_tanren-xtask"))
}

fn fixture_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[track_caller]
fn assert_check_fails(subcommand: &str, fixture: &str, expected_substring: &str) {
    let dir = fixture_dir(fixture);
    assert!(
        dir.exists(),
        "fixture `{fixture}` does not exist at {}",
        dir.display()
    );
    let output = Command::new(xtask_bin())
        .arg(subcommand)
        .arg("--root")
        .arg(&dir)
        .output()
        .expect("xtask binary spawns and produces output");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "expected `xtask {subcommand} --root {}` to fail but it succeeded.\n\
         stdout: {stdout}\n\
         stderr: {stderr}",
        dir.display()
    );
    let combined = format!("{stdout}\n{stderr}");
    assert!(
        combined.contains(expected_substring),
        "expected output of `xtask {subcommand} --root {}` to contain {expected_substring:?}.\n\
         stdout: {stdout}\n\
         stderr: {stderr}",
        dir.display()
    );
}

#[test]
fn check_secrets_rejects_string_password() {
    assert_check_fails("check-secrets", "regression-string-password", "password");
}

#[test]
fn check_bdd_wire_coverage_rejects_direct_handler_dispatch() {
    // `quote::ToTokens` lowercases the receiver (the local `handlers`
    // binding) and inserts spaces around `.`; assert on the rendered
    // form the guard actually emits.
    assert_check_fails(
        "check-bdd-wire-coverage",
        "regression-bdd-direct-handler",
        "handlers . sign_in",
    );
}

#[test]
fn check_bdd_tags_rejects_unknown_surface_tag() {
    assert_check_fails(
        "check-bdd-tags",
        "regression-bdd-unknown-surface",
        "scenario tag @gameplay",
    );
}

#[test]
fn check_test_hooks_rejects_ungated_pub_seed_fn() {
    assert_check_fails(
        "check-test-hooks",
        "regression-pub-test-fn",
        "seed_test_data",
    );
}

#[test]
fn check_newtype_ids_rejects_bare_uuid_field() {
    assert_check_fails("check-newtype-ids", "regression-bare-uuid", "bare uuid");
}

#[test]
fn check_tracing_init_rejects_main_without_init() {
    assert_check_fails(
        "check-tracing-init",
        "regression-no-tracing-init",
        "tanren_observability::init",
    );
}

#[test]
fn check_orphan_traits_rejects_unimplemented_trait() {
    assert_check_fails(
        "check-orphan-traits",
        "regression-orphan-trait",
        "DanglingTrait",
    );
}

#[test]
fn check_openapi_handcraft_rejects_json_literal_document() {
    assert_check_fails(
        "check-openapi-handcraft",
        "regression-openapi-handcraft",
        "hand-rolled",
    );
}

use std::collections::HashMap;

use cucumber::{given, then, when};
use tanren_contract::InstallDriftState;
use tanren_testkit::{DriftReport, InstallDriftFixture};

use crate::TanrenWorld;

#[given(expr = "a freshly installed Tanren repository")]
fn given_fresh_repo(world: &mut TanrenWorld) {
    let fixture = InstallDriftFixture::new().expect("install drift fixture must initialize");
    let snapshots = fixture.snapshot_files().expect("snapshot fixture files");
    world.install_drift = Some(InstallDriftContext {
        fixture,
        report: None,
        snapshots_before: snapshots,
        snapshots_after: None,
    });
}

#[given(expr = "a generated asset is modified")]
fn given_generated_modified(world: &mut TanrenWorld) {
    let ctx = world.install_drift.as_mut().expect("fixture must exist");
    ctx.fixture
        .modify_generated_asset()
        .expect("modify generated asset");
}

#[given(expr = "a generated asset is deleted")]
fn given_generated_deleted(world: &mut TanrenWorld) {
    let ctx = world.install_drift.as_mut().expect("fixture must exist");
    ctx.fixture
        .delete_generated_asset()
        .expect("delete generated asset");
}

#[given(expr = "a preserved standard is deleted")]
fn given_preserved_deleted(world: &mut TanrenWorld) {
    let ctx = world.install_drift.as_mut().expect("fixture must exist");
    ctx.fixture
        .delete_preserved_standard()
        .expect("delete preserved standard");
}

#[given(expr = "a preserved standard is edited by the user")]
fn given_preserved_edited(world: &mut TanrenWorld) {
    let ctx = world.install_drift.as_mut().expect("fixture must exist");
    ctx.fixture
        .edit_preserved_standard()
        .expect("edit preserved standard");
}

#[when(expr = "the drift check runs against the repository")]
async fn when_drift_check(world: &mut TanrenWorld) {
    let pre = {
        let ctx = world.install_drift.as_ref().expect("fixture must exist");
        ctx.fixture
            .snapshot_files()
            .expect("snapshot before drift check")
    };
    let report = {
        let ctx = world.install_drift.as_ref().expect("fixture must exist");
        ctx.fixture
            .run_drift_check()
            .await
            .expect("drift check must succeed")
    };
    let post = {
        let ctx = world.install_drift.as_ref().expect("fixture must exist");
        ctx.fixture
            .snapshot_files()
            .expect("snapshot after drift check")
    };
    let ctx = world.install_drift.as_mut().expect("fixture must exist");
    ctx.report = Some(report);
    ctx.snapshots_before = pre;
    ctx.snapshots_after = Some(post);
}

#[then(expr = "the drift report shows no drift")]
fn then_no_drift(world: &mut TanrenWorld) {
    let ctx = world.install_drift.as_ref().expect("fixture must exist");
    let report = ctx.report.as_ref().expect("drift report must exist");
    assert!(
        !report.has_drift,
        "expected no drift, but drift was reported: {:?}",
        report.entries
    );
    assert!(
        report.entries.iter().all(
            |e| e.state == InstallDriftState::Matches || e.state == InstallDriftState::Accepted
        ),
        "expected all entries to be Matches or Accepted, got: {:?}",
        report.entries
    );
}

#[then(expr = "the drift report shows drift")]
fn then_has_drift(world: &mut TanrenWorld) {
    let ctx = world.install_drift.as_ref().expect("fixture must exist");
    let report = ctx.report.as_ref().expect("drift report must exist");
    assert!(
        report.has_drift,
        "expected drift, but none was reported: {:?}",
        report.entries
    );
}

#[then(expr = "the drift report shows the standard as missing")]
fn then_standard_missing(world: &mut TanrenWorld) {
    let ctx = world.install_drift.as_ref().expect("fixture must exist");
    let report = ctx.report.as_ref().expect("drift report must exist");
    let expected_path = InstallDriftFixture::first_preserved_rel_path();
    let found = report
        .entries
        .iter()
        .find(|e| e.relative_path == expected_path);
    assert!(
        found.is_some(),
        "expected entry for path {expected_path}, got: {:?}",
        report.entries
    );
    let entry = found.expect("checked above");
    assert_eq!(
        entry.state,
        InstallDriftState::Missing,
        "expected {} to be Missing, got {:?}",
        expected_path,
        entry.state
    );
}

#[then(expr = "the drift report shows the standard as accepted")]
fn then_standard_accepted(world: &mut TanrenWorld) {
    let ctx = world.install_drift.as_ref().expect("fixture must exist");
    let report = ctx.report.as_ref().expect("drift report must exist");
    let expected_path = InstallDriftFixture::first_preserved_rel_path();
    let found = report
        .entries
        .iter()
        .find(|e| e.relative_path == expected_path);
    assert!(
        found.is_some(),
        "expected entry for path {expected_path}, got: {:?}",
        report.entries
    );
    let entry = found.expect("checked above");
    assert_eq!(
        entry.state,
        InstallDriftState::Accepted,
        "expected {} to be Accepted, got {:?}",
        expected_path,
        entry.state
    );
}

#[then(expr = "the repository is unchanged by the drift check")]
fn then_unchanged(world: &mut TanrenWorld) {
    let ctx = world.install_drift.as_ref().expect("fixture must exist");
    let before = &ctx.snapshots_before;
    let after = ctx
        .snapshots_after
        .as_ref()
        .expect("post-check snapshots must exist");
    let all_paths = InstallDriftFixture::all_manifest_rel_paths();
    for rel_path in &all_paths {
        let before_bytes = before.get(*rel_path);
        let after_bytes = after.get(*rel_path);
        let before_present = before_bytes.is_some();
        let after_present = after_bytes.is_some();
        assert_eq!(
            before_present, after_present,
            "file {rel_path} presence changed after drift check (before={before_present}, after={after_present})"
        );
        if let (Some(b), Some(a)) = (before_bytes, after_bytes) {
            assert_eq!(b, a, "file {rel_path} was modified by the drift check");
        }
    }
}

#[then(expr = "the preserved standard is reported as accepted")]
fn then_preserved_accepted(world: &mut TanrenWorld) {
    let ctx = world.install_drift.as_ref().expect("fixture must exist");
    let report = ctx.report.as_ref().expect("drift report must exist");
    let expected_path = InstallDriftFixture::first_preserved_rel_path();
    let found = report
        .entries
        .iter()
        .find(|e| e.relative_path == expected_path);
    assert!(
        found.is_some(),
        "expected entry for preserved path {expected_path}, got: {:?}",
        report.entries
    );
    let entry = found.expect("checked above");
    assert_eq!(
        entry.state,
        InstallDriftState::Accepted,
        "expected {expected_path} to be Accepted, got {:?}",
        entry.state
    );
}

#[then(expr = "the generated asset is reported as drifted")]
fn then_generated_drifted(world: &mut TanrenWorld) {
    let ctx = world.install_drift.as_ref().expect("fixture must exist");
    let report = ctx.report.as_ref().expect("drift report must exist");
    let expected_path = InstallDriftFixture::first_generated_rel_path();
    let found = report
        .entries
        .iter()
        .find(|e| e.relative_path == expected_path);
    assert!(
        found.is_some(),
        "expected entry for generated path {expected_path}, got: {:?}",
        report.entries
    );
    let entry = found.expect("checked above");
    assert_eq!(
        entry.state,
        InstallDriftState::Drifted,
        "expected {expected_path} to be Drifted, got {:?}",
        entry.state
    );
}

#[then(expr = "the generated asset is reported as missing")]
fn then_generated_missing(world: &mut TanrenWorld) {
    let ctx = world.install_drift.as_ref().expect("fixture must exist");
    let report = ctx.report.as_ref().expect("drift report must exist");
    let expected_path = InstallDriftFixture::first_generated_rel_path();
    let found = report
        .entries
        .iter()
        .find(|e| e.relative_path == expected_path);
    assert!(
        found.is_some(),
        "expected entry for generated path {expected_path}, got: {:?}",
        report.entries
    );
    let entry = found.expect("checked above");
    assert_eq!(
        entry.state,
        InstallDriftState::Missing,
        "expected {expected_path} to be Missing, got {:?}",
        entry.state
    );
}

pub struct InstallDriftContext {
    pub fixture: InstallDriftFixture,
    pub report: Option<DriftReport>,
    pub snapshots_before: HashMap<String, Vec<u8>>,
    pub snapshots_after: Option<HashMap<String, Vec<u8>>>,
}

impl std::fmt::Debug for InstallDriftContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstallDriftContext")
            .field("report", &self.report)
            .finish_non_exhaustive()
    }
}

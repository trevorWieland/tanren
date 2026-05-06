use std::collections::HashMap;

use cucumber::{given, then, when};
use tanren_testkit::{DriftEntry, DriftReport, InstallDriftFixture};

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
    let found = report.entries.iter().any(|e| e.state == "missing");
    assert!(
        found,
        "expected at least one missing entry, got {:?}",
        report.entries
    );
}

#[then(expr = "the drift report shows the standard as accepted")]
fn then_standard_accepted(world: &mut TanrenWorld) {
    let ctx = world.install_drift.as_ref().expect("fixture must exist");
    let report = ctx.report.as_ref().expect("drift report must exist");
    let found = report.entries.iter().any(|e| e.state == "accepted");
    assert!(
        found,
        "expected at least one accepted entry, got {:?}",
        report.entries
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
    for (path, before_bytes) in before {
        match after.get(path) {
            Some(after_bytes) => assert_eq!(
                before_bytes, after_bytes,
                "file {path} was modified by the drift check"
            ),
            None => {
                unreachable!("file {path} existed before drift check but is missing after")
            }
        }
    }
}

#[then(expr = "the preserved standard is reported as accepted")]
fn then_preserved_accepted(world: &mut TanrenWorld) {
    let ctx = world.install_drift.as_ref().expect("fixture must exist");
    let report = ctx.report.as_ref().expect("drift report must exist");
    let accepted: Vec<&DriftEntry> = report
        .entries
        .iter()
        .filter(|e| e.state == "accepted")
        .collect();
    assert!(
        !accepted.is_empty(),
        "expected at least one accepted preserved standard, got {:?}",
        report.entries
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

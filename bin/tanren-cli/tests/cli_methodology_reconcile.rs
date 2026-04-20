//! Integration tests for methodology projection-reconcile CLI paths.

#[path = "support/methodology_test_support.rs"]
mod methodology_test_support;

use methodology_test_support::{
    cli, mk_spec_folder, mkdb, parse_stderr, parse_stdout, write_legacy_phase_events_file,
    write_phase_events_file,
};
use tanren_domain::SpecId;

#[test]
fn reconcile_projections_requires_spec_id() {
    let (_d, url) = mkdb();
    let temp = tempfile::tempdir().expect("tempdir");
    let spec = SpecId::new();
    let _ = write_phase_events_file(temp.path(), spec);
    let _ = write_legacy_phase_events_file(temp.path(), spec);
    let out = cli(&url)
        .args(["methodology", "reconcile-projections"])
        .output()
        .expect("cli");
    assert_eq!(out.status.code(), Some(4));
    let err = parse_stderr(&out);
    assert_eq!(err["kind"].as_str(), Some("validation_failed"));
    assert_eq!(err["field_path"].as_str(), Some("/spec_id"));
}

#[test]
fn reconcile_projections_rebuilds_report() {
    let (d, url) = mkdb();
    let spec = "00000000-0000-0000-0000-000000000414";
    let spec_folder = mk_spec_folder(&d, spec);
    let create = cli(&url)
        .args([
            "methodology",
            "--spec-id",
            spec,
            "--spec-folder",
            spec_folder.to_str().expect("utf8"),
            "task",
            "create",
            "--json",
            &format!(
                "{{\"schema_version\":\"1.0.0\",\"spec_id\":\"{spec}\",\"title\":\"T\",\"description\":\"\",\"origin\":{{\"kind\":\"user\"}},\"acceptance_criteria\":[]}}"
            ),
        ])
        .output()
        .expect("create task");
    assert!(create.status.success(), "create task should succeed");

    let out = cli(&url)
        .args(["methodology", "--spec-id", spec, "reconcile-projections"])
        .output()
        .expect("reconcile projections");
    assert!(out.status.success(), "reconcile-projections should succeed");
    let v = parse_stdout(&out);
    assert!(
        v["tasks_rebuilt"].as_u64().unwrap_or(0) >= 1,
        "expected at least one rebuilt task projection row"
    );
    assert!(v["task_spec_rows_repaired"].is_u64());
    assert!(v["signpost_spec_rows_repaired"].is_u64());
}

#[test]
fn reconcile_phase_events_does_not_require_spec_id() {
    let (d, url) = mkdb();
    let spec = "00000000-0000-0000-0000-000000000415";
    let spec_folder = mk_spec_folder(&d, spec);

    let out = cli(&url)
        .args([
            "methodology",
            "--spec-folder",
            spec_folder.to_str().expect("utf8"),
            "reconcile-phase-events",
        ])
        .output()
        .expect("reconcile phase-events");
    assert!(
        out.status.success(),
        "reconcile-phase-events should succeed without --spec-id: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_stdout(&out);
    assert!(v["projected"].is_u64());
}

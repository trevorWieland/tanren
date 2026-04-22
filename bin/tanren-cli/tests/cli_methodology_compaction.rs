#[path = "support/methodology_test_support.rs"]
mod methodology_test_support;

use methodology_test_support::{
    cli, mk_spec_folder, mkdb, parse_stderr, parse_stdout, write_legacy_phase_events_file,
    write_phase_events_file,
};

#[test]
fn compact_phase_events_requires_spec_folder() {
    let (_d, url) = mkdb();
    let out = cli(&url)
        .args(["methodology", "compact-phase-events"])
        .output()
        .expect("cli");
    assert_eq!(out.status.code(), Some(4));
    let err = parse_stderr(&out);
    assert_eq!(err["kind"].as_str(), Some("validation_failed"));
    assert_eq!(err["field_path"].as_str(), Some("/spec_folder"));
}

#[test]
fn compact_phase_events_dedupes_duplicate_event_ids() {
    let (d, url) = mkdb();
    let spec = "00000000-0000-0000-0000-000000000501";
    let spec_folder = mk_spec_folder(&d, spec);
    let _ = write_phase_events_file(&spec_folder, tanren_domain::SpecId::new());
    let _ = write_legacy_phase_events_file(&spec_folder, tanren_domain::SpecId::new());
    let event_id = "019da764-e015-7af1-9385-8a7b98995ec1";
    let payload = format!(
        "\n{{\"schema_version\":\"1.0.0\",\"event_id\":\"{event_id}\",\"spec_id\":\"{spec}\",\"phase\":\"do-task\",\"agent_session_id\":\"s\",\"timestamp\":\"2026-04-22T00:00:00Z\",\"origin_kind\":\"tool_primary\",\"tool\":\"start_task\",\"payload\":{{\"kind\":\"task_started\",\"task_id\":\"00000000-0000-0000-0000-000000000001\",\"spec_id\":\"{spec}\"}}}}\n{{\"schema_version\":\"1.0.0\",\"event_id\":\"{event_id}\",\"spec_id\":\"{spec}\",\"phase\":\"do-task\",\"agent_session_id\":\"s\",\"timestamp\":\"2026-04-22T00:00:01Z\",\"origin_kind\":\"tool_primary\",\"tool\":\"start_task\",\"payload\":{{\"kind\":\"task_started\",\"task_id\":\"00000000-0000-0000-0000-000000000001\",\"spec_id\":\"{spec}\"}}}}\n"
    );
    std::fs::write(spec_folder.join("phase-events.jsonl"), payload).expect("seed phase events");

    let out = cli(&url)
        .args([
            "methodology",
            "--spec-folder",
            spec_folder.to_str().expect("utf8"),
            "compact-phase-events",
        ])
        .output()
        .expect("cli");
    assert!(
        out.status.success(),
        "compact-phase-events should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let body = parse_stdout(&out);
    assert_eq!(body["duplicates_removed"].as_u64(), Some(1));
    let on_disk =
        std::fs::read_to_string(spec_folder.join("phase-events.jsonl")).expect("phase events");
    let lines: Vec<&str> = on_disk
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    assert_eq!(lines.len(), 1);
}

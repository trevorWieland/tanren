use std::path::PathBuf;

use tanren_domain::methodology::default_phase_capability_bindings;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn agent_tool_surface_phase_scope_table_matches_domain_contract() {
    let docs =
        std::fs::read_to_string(workspace_root().join("docs/architecture/agent-tool-surface.md"))
            .expect("read agent-tool-surface.md");
    let start = docs
        .find("## 4. Per-phase capability scopes")
        .expect("phase scope section");
    let tail = &docs[start..];
    let end = tail.find("\n---").expect("section terminator");
    let section = &tail[..end];

    for binding in default_phase_capability_bindings() {
        let tags = binding
            .capabilities
            .iter()
            .map(|cap| cap.tag())
            .collect::<Vec<_>>()
            .join(", ");
        let expected_line = format!("| `{}` | {tags} |", binding.phase.tag());
        assert!(
            section.contains(&expected_line),
            "missing or drifted phase-scope row: {expected_line}"
        );
    }

    let row_count = section
        .lines()
        .filter(|line| line.starts_with("| `") && line.contains(" | "))
        .count();
    assert_eq!(
        row_count,
        default_phase_capability_bindings().len(),
        "phase-scope docs row count must match canonical binding count"
    );
}

use std::path::PathBuf;

use tanren_app_services::methodology::{
    append_only_protected_artifacts, readonly_protected_artifacts,
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn orchestration_flow_docs_list_canonical_protected_artifacts() {
    let docs =
        std::fs::read_to_string(workspace_root().join("docs/architecture/orchestration-flow.md"))
            .expect("read orchestration-flow.md");

    for file in readonly_protected_artifacts() {
        assert!(
            docs.contains(file),
            "docs/architecture/orchestration-flow.md must include readonly artifact `{file}`"
        );
    }
    for file in append_only_protected_artifacts() {
        assert!(
            docs.contains(file),
            "docs/architecture/orchestration-flow.md must include append-only artifact `{file}`"
        );
    }
}

#[test]
fn evidence_schema_docs_mark_audit_and_signposts_as_generated() {
    let docs =
        std::fs::read_to_string(workspace_root().join("docs/architecture/evidence-schemas.md"))
            .expect("read evidence-schemas.md");
    assert!(
        docs.contains("| `audit.md` | `AuditFrontmatter` | generated |"),
        "audit.md must be documented as generated"
    );
    assert!(
        docs.contains("| `signposts.md` | `SignpostsFrontmatter` | generated |"),
        "signposts.md must be documented as generated"
    );
    assert!(
        !docs.contains("| `audit.md` | `AuditFrontmatter` | agent (narrative) |"),
        "legacy narrative ownership text must be removed for audit.md"
    );
    assert!(
        !docs.contains("| `signposts.md` | `SignpostsFrontmatter` | agent (narrative) |"),
        "legacy narrative ownership text must be removed for signposts.md"
    );
}

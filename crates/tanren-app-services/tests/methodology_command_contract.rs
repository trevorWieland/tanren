use std::fs;
use std::path::Path;

use tanren_app_services::methodology::source::load_catalog;

fn write_command(root: &Path, rel: &str, frontmatter: &str, body: &str) {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("mkdir");
    }
    fs::write(path, format!("---\n{frontmatter}\n---\n\n{body}\n")).expect("write");
}

#[test]
fn load_catalog_rejects_unknown_declared_tool() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_command(
        dir.path(),
        "spec/demo.md",
        r#"name: demo
role: audit
declared_variables: []
declared_tools:
  - not_a_real_tool
required_capabilities:
  - rubric.record
"#,
        "body",
    );

    let err = load_catalog(dir.path()).expect_err("unknown tool must fail");
    assert!(
        err.to_string()
            .contains("unknown declared tool `not_a_real_tool`"),
        "unexpected error: {err}"
    );
}

#[test]
fn load_catalog_rejects_unknown_required_capability() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_command(
        dir.path(),
        "spec/demo.md",
        r#"name: demo
role: audit
declared_variables: []
declared_tools:
  - record_rubric_score
required_capabilities:
  - rubric.record
  - unknown.capability
"#,
        "body",
    );

    let err = load_catalog(dir.path()).expect_err("unknown capability must fail");
    assert!(
        err.to_string()
            .contains("unknown required capability `unknown.capability`"),
        "unexpected error: {err}"
    );
}

#[test]
fn load_catalog_rejects_declared_tool_without_implied_capability() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_command(
        dir.path(),
        "spec/demo.md",
        r#"name: demo
role: audit
declared_variables: []
declared_tools:
  - record_non_negotiable_compliance
required_capabilities:
  - rubric.record
"#,
        "body",
    );

    let err = load_catalog(dir.path()).expect_err("missing implied capability must fail");
    assert!(
        err.to_string()
            .contains("requires capability `compliance.record`"),
        "unexpected error: {err}"
    );
}

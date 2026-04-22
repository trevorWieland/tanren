//! CLI metadata contract tests for tanren-mcp.

#[test]
fn version_flag_reports_binary_identity() {
    use assert_cmd::Command;

    let output = Command::cargo_bin("tanren-mcp")
        .expect("cargo bin")
        .arg("--version")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).expect("utf8");
    assert!(text.starts_with("tanren-mcp "));
}

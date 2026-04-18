//! CLI-level tests for the new `tanren install` flags: `--profile`,
//! `--source`, `--target`. Audit finding #6.

use std::io::Write as _;

use assert_cmd::Command;
use tempfile::TempDir;

fn write_config(dir: &TempDir, yaml: &str) -> std::path::PathBuf {
    let p = dir.path().join("tanren.yml");
    let mut f = std::fs::File::create(&p).expect("create");
    f.write_all(yaml.as_bytes()).expect("write");
    p
}

fn write_command(dir: &TempDir, subdir: &str) {
    let cmds = dir.path().join(subdir);
    std::fs::create_dir_all(&cmds).expect("mkdir commands");
    let body = "---\n\
name: do-task\n\
role: implementation\n\
orchestration_loop: true\n\
autonomy: autonomous\n\
declared_variables: []\n\
declared_tools: []\n\
required_capabilities: []\n\
produces_evidence: []\n\
---\n\
body text\n";
    std::fs::write(cmds.join("do-task.md"), body).expect("write cmd");
}

#[test]
fn install_help_lists_profile_source_target_flags() {
    let out = Command::cargo_bin("tanren-cli")
        .expect("bin")
        .args(["install", "--help"])
        .output()
        .expect("help");
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(help.contains("--profile"), "missing --profile: {help}");
    assert!(help.contains("--source"), "missing --source: {help}");
    assert!(help.contains("--target"), "missing --target: {help}");
}

#[test]
fn install_rejects_unknown_profile_with_exit_4() {
    let dir = TempDir::new().expect("tempdir");
    write_command(&dir, "commands/spec");
    let cfg_yaml = r"methodology:
  source:
    path: commands
  install_targets: []
";
    let cfg = write_config(&dir, cfg_yaml);
    let out = Command::cargo_bin("tanren-cli")
        .expect("bin")
        .current_dir(dir.path())
        .args([
            "install",
            "--config",
            cfg.to_str().expect("cfg utf8"),
            "--profile",
            "nonexistent",
            "--dry-run",
        ])
        .output()
        .expect("run");
    assert_eq!(
        out.status.code(),
        Some(4),
        "expected exit 4; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("unknown --profile"), "stderr: {stderr}");
}

#[test]
fn install_rejects_unknown_target_with_exit_4() {
    let dir = TempDir::new().expect("tempdir");
    write_command(&dir, "commands/spec");
    let cfg_yaml = r"methodology:
  source:
    path: commands
  install_targets: []
";
    let cfg = write_config(&dir, cfg_yaml);
    let out = Command::cargo_bin("tanren-cli")
        .expect("bin")
        .current_dir(dir.path())
        .args([
            "install",
            "--config",
            cfg.to_str().expect("cfg utf8"),
            "--target",
            "bogus-format",
            "--dry-run",
        ])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(4));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("unknown --target"), "stderr: {stderr}");
}

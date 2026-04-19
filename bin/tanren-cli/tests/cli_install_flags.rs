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

fn write_command_with_body(dir: &TempDir, subdir: &str, body: &str) {
    let cmds = dir.path().join(subdir);
    std::fs::create_dir_all(&cmds).expect("mkdir commands");
    std::fs::write(cmds.join("do-task.md"), body).expect("write cmd");
}

fn write_command_with_vars(dir: &TempDir, subdir: &str, body: &str, vars: &[&str]) {
    let declared = if vars.is_empty() {
        "[]".to_owned()
    } else {
        let quoted = vars
            .iter()
            .map(|v| format!("\"{v}\""))
            .collect::<Vec<_>>()
            .join(", ");
        format!("[{quoted}]")
    };
    let command = format!(
        "---\n\
name: do-task\n\
role: implementation\n\
orchestration_loop: true\n\
autonomy: autonomous\n\
declared_variables: {declared}\n\
declared_tools: []\n\
required_capabilities: []\n\
produces_evidence: []\n\
---\n\
{body}\n"
    );
    write_command_with_body(dir, subdir, &command);
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

#[test]
fn install_render_failure_exits_1() {
    let dir = TempDir::new().expect("tempdir");
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
body {{MISSING}}\n";
    write_command_with_body(&dir, "commands/spec", body);
    let cfg_yaml = r"methodology:
  source:
    path: commands
  install_targets:
    - path: .claude/commands
      format: claude-code
      binding: mcp
      merge_policy: destructive
";
    let cfg = write_config(&dir, cfg_yaml);
    let out = Command::cargo_bin("tanren-cli")
        .expect("bin")
        .current_dir(dir.path())
        .args([
            "install",
            "--config",
            cfg.to_str().expect("cfg utf8"),
            "--dry-run",
        ])
        .output()
        .expect("run");
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn install_malformed_mcp_config_exits_1() {
    let dir = TempDir::new().expect("tempdir");
    write_command(&dir, "commands/spec");
    std::fs::write(dir.path().join(".mcp.json"), "{ broken").expect("seed bad mcp json");
    let cfg_yaml = r"methodology:
  source:
    path: commands
  install_targets: []
  mcp:
    enabled: true
    transport: stdio
    also_write_configs:
      - path: .mcp.json
        format: claude-mcp-json
        merge_policy: preserve_other_keys
";
    let cfg = write_config(&dir, cfg_yaml);
    let out = Command::cargo_bin("tanren-cli")
        .expect("bin")
        .current_dir(dir.path())
        .args([
            "install",
            "--config",
            cfg.to_str().expect("cfg utf8"),
            "--dry-run",
        ])
        .output()
        .expect("run");
    assert_eq!(
        out.status.code(),
        Some(1),
        "expected exit 1; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn install_profile_overrides_required_guards_variable() {
    let dir = TempDir::new().expect("tempdir");
    write_command_with_vars(
        &dir,
        "commands/spec",
        "guards={{REQUIRED_GUARDS}}",
        &["REQUIRED_GUARDS"],
    );
    let cfg_yaml = r"methodology:
  task_complete_requires: [gate_checked, audited, adherent]
  source:
    path: commands
  install_targets:
    - path: .claude/commands
      format: claude-code
      binding: mcp
      merge_policy: destructive
  profiles:
    lean:
      task_complete_requires: [gate_checked]
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
            "lean",
        ])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "install failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let rendered = std::fs::read_to_string(dir.path().join(".claude/commands/do-task.md"))
        .expect("read rendered command");
    assert!(
        rendered.contains("guards=gate_checked"),
        "expected profile guard override in rendered output, got:\n{rendered}"
    );
}

#[test]
fn install_prefers_rubric_file_for_pillar_list() {
    let dir = TempDir::new().expect("tempdir");
    write_command_with_vars(
        &dir,
        "commands/spec",
        "pillars={{PILLAR_LIST}}",
        &["PILLAR_LIST"],
    );
    std::fs::create_dir_all(dir.path().join("tanren")).expect("mkdir tanren");
    std::fs::write(
        dir.path().join("tanren/rubric.yml"),
        r"pillars:
  - id: compile_time_verification_strictness
    name: Compile-Time Verification Strictness
    task_description: Enforce type-level invariants aggressively.
    spec_description: Preserve compile-time safety across the whole spec.
    target_score: 10
    passing_score: 7
    applicable_at: both
",
    )
    .expect("write rubric.yml");
    let cfg_yaml = r"methodology:
  source:
    path: commands
  install_targets:
    - path: .claude/commands
      format: claude-code
      binding: mcp
      merge_policy: destructive
  rubric:
    pillars:
      - id: from_methodology_config
        name: From Methodology Config
        task_description: Should lose to rubric file.
        spec_description: Should lose to rubric file.
        target_score: 10
        passing_score: 7
        applicable_at: both
";
    let cfg = write_config(&dir, cfg_yaml);
    let out = Command::cargo_bin("tanren-cli")
        .expect("bin")
        .current_dir(dir.path())
        .args(["install", "--config", cfg.to_str().expect("cfg utf8")])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "install failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let rendered = std::fs::read_to_string(dir.path().join(".claude/commands/do-task.md"))
        .expect("read rendered command");
    assert!(
        rendered.contains("compile_time_verification_strictness"),
        "expected tanren/rubric.yml pillar ids to win, got:\n{rendered}"
    );
    assert!(
        !rendered.contains("from_methodology_config"),
        "methodology.rubric must not override tanren/rubric.yml when both exist"
    );
}

#[test]
fn install_uses_tanren_yml_rubric_when_rubric_file_missing() {
    let dir = TempDir::new().expect("tempdir");
    write_command_with_vars(
        &dir,
        "commands/spec",
        "pillars={{PILLAR_LIST}}",
        &["PILLAR_LIST"],
    );
    let cfg_yaml = r"methodology:
  source:
    path: commands
  install_targets:
    - path: .claude/commands
      format: claude-code
      binding: mcp
      merge_policy: destructive
  rubric:
    pillars:
      - id: custom_runtime
        name: Custom Runtime
        task_description: Custom task lens.
        spec_description: Custom spec lens.
        target_score: 10
        passing_score: 7
        applicable_at: both
";
    let cfg = write_config(&dir, cfg_yaml);
    let out = Command::cargo_bin("tanren-cli")
        .expect("bin")
        .current_dir(dir.path())
        .args(["install", "--config", cfg.to_str().expect("cfg utf8")])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "install failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let rendered = std::fs::read_to_string(dir.path().join(".claude/commands/do-task.md"))
        .expect("read rendered command");
    assert!(
        rendered.contains("custom_runtime"),
        "expected methodology.rubric pillar ids in rendered output, got:\n{rendered}"
    );
}

#[test]
fn install_legacy_top_level_rubric_alias_is_still_supported() {
    let dir = TempDir::new().expect("tempdir");
    write_command_with_vars(
        &dir,
        "commands/spec",
        "pillars={{PILLAR_LIST}}",
        &["PILLAR_LIST"],
    );
    let cfg_yaml = r"methodology:
  source:
    path: commands
  install_targets:
    - path: .claude/commands
      format: claude-code
      binding: mcp
      merge_policy: destructive
rubric:
  pillars:
    - id: legacy_top_level
      name: Legacy Top-Level
      task_description: Legacy config alias.
      spec_description: Legacy config alias.
      target_score: 10
      passing_score: 7
      applicable_at: both
";
    let cfg = write_config(&dir, cfg_yaml);
    let out = Command::cargo_bin("tanren-cli")
        .expect("bin")
        .current_dir(dir.path())
        .args(["install", "--config", cfg.to_str().expect("cfg utf8")])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "install failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let rendered = std::fs::read_to_string(dir.path().join(".claude/commands/do-task.md"))
        .expect("read rendered command");
    assert!(
        rendered.contains("legacy_top_level"),
        "expected deprecated top-level rubric alias to be honored, got:\n{rendered}"
    );
}

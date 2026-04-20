//! Hook-resolution and strict dry-run tests split from `cli_install_flags.rs`
//! to keep file-length checks under the repo guardrail.

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
fn install_hook_resolution_prefers_phase_keys_then_base_fallbacks() {
    let dir = TempDir::new().expect("tempdir");
    write_command_with_vars(
        &dir,
        "commands/spec",
        "task={{TASK_VERIFICATION_HOOK}}\n\
spec={{SPEC_VERIFICATION_HOOK}}\n\
audit_task={{AUDIT_TASK_HOOK}}\n\
adhere_task={{ADHERE_TASK_HOOK}}\n\
run_demo={{RUN_DEMO_HOOK}}\n\
audit_spec={{AUDIT_SPEC_HOOK}}\n\
adhere_spec={{ADHERE_SPEC_HOOK}}",
        &[
            "TASK_VERIFICATION_HOOK",
            "SPEC_VERIFICATION_HOOK",
            "AUDIT_TASK_HOOK",
            "ADHERE_TASK_HOOK",
            "RUN_DEMO_HOOK",
            "AUDIT_SPEC_HOOK",
            "ADHERE_SPEC_HOOK",
        ],
    );
    let cfg_yaml = r"methodology:
  source:
    path: commands
  install_targets:
    - path: .claude/commands
      format: claude-code
      binding: mcp
      merge_policy: destructive
environment:
  default:
    gate_cmd: gate-default
    task_gate_cmd: gate-task
    spec_gate_cmd: gate-spec
    verification_hooks:
      do-task: hook-do
      audit-task: hook-audit-task
      run-demo: hook-demo
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
    assert!(rendered.contains("task=hook-do"), "rendered:\n{rendered}");
    assert!(rendered.contains("spec=hook-demo"), "rendered:\n{rendered}");
    assert!(
        rendered.contains("audit_task=hook-audit-task"),
        "rendered:\n{rendered}"
    );
    assert!(
        rendered.contains("adhere_task=hook-do"),
        "rendered:\n{rendered}"
    );
    assert!(
        rendered.contains("run_demo=hook-demo"),
        "rendered:\n{rendered}"
    );
    assert!(
        rendered.contains("audit_spec=hook-demo"),
        "rendered:\n{rendered}"
    );
    assert!(
        rendered.contains("adhere_spec=hook-demo"),
        "rendered:\n{rendered}"
    );
}

#[test]
fn install_hook_resolution_uses_scoped_legacy_gate_fallbacks() {
    let dir = TempDir::new().expect("tempdir");
    write_command_with_vars(
        &dir,
        "commands/spec",
        "task={{TASK_VERIFICATION_HOOK}}\n\
spec={{SPEC_VERIFICATION_HOOK}}\n\
audit_task={{AUDIT_TASK_HOOK}}\n\
run_demo={{RUN_DEMO_HOOK}}",
        &[
            "TASK_VERIFICATION_HOOK",
            "SPEC_VERIFICATION_HOOK",
            "AUDIT_TASK_HOOK",
            "RUN_DEMO_HOOK",
        ],
    );
    let cfg_yaml = r"methodology:
  source:
    path: commands
  install_targets:
    - path: .claude/commands
      format: claude-code
      binding: mcp
      merge_policy: destructive
environment:
  default:
    gate_cmd: gate-default
    task_gate_cmd: gate-task
    spec_gate_cmd: gate-spec
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
    assert!(rendered.contains("task=gate-task"), "rendered:\n{rendered}");
    assert!(
        rendered.contains("audit_task=gate-task"),
        "rendered:\n{rendered}"
    );
    assert!(rendered.contains("spec=gate-spec"), "rendered:\n{rendered}");
    assert!(
        rendered.contains("run_demo=gate-spec"),
        "rendered:\n{rendered}"
    );
}

#[test]
fn install_hook_resolution_prefers_methodology_variables_over_environment() {
    let dir = TempDir::new().expect("tempdir");
    write_command_with_vars(
        &dir,
        "commands/spec",
        "task={{TASK_VERIFICATION_HOOK}}\n\
spec={{SPEC_VERIFICATION_HOOK}}\n\
audit_task={{AUDIT_TASK_HOOK}}",
        &[
            "TASK_VERIFICATION_HOOK",
            "SPEC_VERIFICATION_HOOK",
            "AUDIT_TASK_HOOK",
        ],
    );
    let cfg_yaml = r"methodology:
  source:
    path: commands
  install_targets:
    - path: .claude/commands
      format: claude-code
      binding: mcp
      merge_policy: destructive
  variables:
    task_verification_hook: var-task
    spec_verification_hook: var-spec
    audit_task_hook: var-audit-task
environment:
  default:
    gate_cmd: gate-default
    task_gate_cmd: gate-task
    spec_gate_cmd: gate-spec
    verification_hooks:
      do-task: hook-do
      audit-task: hook-audit-task
      run-demo: hook-demo
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
    assert!(rendered.contains("task=var-task"), "rendered:\n{rendered}");
    assert!(rendered.contains("spec=var-spec"), "rendered:\n{rendered}");
    assert!(
        rendered.contains("audit_task=var-audit-task"),
        "rendered:\n{rendered}"
    );
}

#[test]
fn install_strict_dry_run_reports_exact_diff_payload() {
    let dir = TempDir::new().expect("tempdir");
    write_command(&dir, "commands/spec");
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
    let first = Command::cargo_bin("tanren-cli")
        .expect("bin")
        .current_dir(dir.path())
        .args(["install", "--config", cfg.to_str().expect("cfg utf8")])
        .output()
        .expect("install");
    assert!(
        first.status.success(),
        "initial install failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );

    let rendered = dir.path().join(".claude/commands/do-task.md");
    std::fs::write(&rendered, "manually drifted\n").expect("drift");

    let out = Command::cargo_bin("tanren-cli")
        .expect("bin")
        .current_dir(dir.path())
        .args([
            "install",
            "--config",
            cfg.to_str().expect("cfg utf8"),
            "--dry-run",
            "--strict",
        ])
        .output()
        .expect("strict dry-run");
    assert_eq!(
        out.status.code(),
        Some(3),
        "strict dry-run should exit 3 on drift"
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let drifts = json["drift"].as_array().expect("drift array");
    let differs = drifts
        .iter()
        .find(|entry| entry["kind"] == "differs")
        .expect("differs entry");
    assert!(
        differs["expected_sha256"].as_str().is_some(),
        "expected hash payload in differs entry"
    );
    assert!(
        differs["actual_sha256"].as_str().is_some(),
        "actual hash payload in differs entry"
    );
    assert!(
        differs["unified_diff"]
            .as_str()
            .is_some_and(|patch| patch.contains("--- expected:")),
        "unified diff payload missing expected header"
    );
}

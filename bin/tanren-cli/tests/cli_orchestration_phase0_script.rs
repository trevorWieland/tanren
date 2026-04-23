use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::prelude::*;

const ALL_CAPS: &str = "task.create,task.start,task.complete,task.revise,task.abandon,task.read,finding.add,rubric.record,compliance.record,spec.frontmatter,demo.frontmatter,demo.results,signpost.add,signpost.update,phase.outcome,phase.escalate,issue.create,standard.read,adherence.record,feedback.reply";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("bin/")
        .parent()
        .expect("repo root")
        .to_path_buf()
}

fn write_executable(path: &Path, body: &str) {
    fs::write(path, body).expect("write executable");
    let mut perms = fs::metadata(path).expect("metadata").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("chmod");
}

fn mkdb() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("tanren.db");
    let url = format!("sqlite:{}?mode=rwc", db.display());
    Command::cargo_bin("tanren-cli")
        .expect("bin")
        .args(["--database-url", &url, "db", "migrate"])
        .assert()
        .success();
    (dir, url)
}

fn cli(url: &str) -> Command {
    let mut cmd = Command::cargo_bin("tanren-cli").expect("tanren-cli");
    cmd.args(["--database-url", url]);
    cmd.env("TANREN_PHASE_CAPABILITIES", ALL_CAPS);
    cmd
}

fn create_task(url: &str, spec_id: &str, spec_folder: &Path, title: &str) -> String {
    let out = cli(url)
        .args([
            "methodology",
            "--phase",
            "shape-spec",
            "--spec-id",
            spec_id,
            "--spec-folder",
            &spec_folder.to_string_lossy(),
            "task",
            "create",
            "--json",
            &format!(
                "{{\"schema_version\":\"1.0.0\",\"spec_id\":\"{spec_id}\",\"title\":\"{title}\",\"description\":\"Ship {title}\",\"origin\":{{\"kind\":\"user\"}},\"acceptance_criteria\":[{{\"id\":\"ac-1\",\"description\":\"done\",\"measurable\":\"artifact shipped\"}}]}}"
            ),
        ])
        .output()
        .expect("create task");
    assert!(
        out.status.success(),
        "create task failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let body: serde_json::Value = serde_json::from_slice(&out.stdout).expect("stdout json");
    body["task_id"].as_str().expect("task_id").to_owned()
}

fn run_task_mutation(
    url: &str,
    spec_id: &str,
    spec_folder: &Path,
    phase: &str,
    verb: &str,
    payload: &str,
) {
    let out = cli(url)
        .args([
            "methodology",
            "--phase",
            phase,
            "--spec-id",
            spec_id,
            "--spec-folder",
            &spec_folder.to_string_lossy(),
            "task",
            verb,
            "--json",
            payload,
        ])
        .output()
        .expect("task mutation");
    assert!(
        out.status.success(),
        "{phase} task {verb} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn mk_shim_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("shim tempdir");
    let tanren_cli_bin = Command::cargo_bin("tanren-cli")
        .expect("tanren-cli")
        .get_program()
        .to_string_lossy()
        .to_string();
    write_executable(
        &dir.path().join("tanren-cli"),
        &format!("#!/usr/bin/env bash\nexec '{tanren_cli_bin}' \"$@\"\n"),
    );
    write_executable(
        &dir.path().join("tanren-mcp"),
        "#!/usr/bin/env bash\nexit 0\n",
    );
    write_executable(&dir.path().join("codex"), "#!/usr/bin/env bash\nexit 0\n");
    dir
}

fn run_phase0(
    path_env: &str,
    config: &Path,
    db_url: &str,
    spec_id: &str,
    spec_folder: &Path,
    output_mode: &str,
) -> std::process::Output {
    let script = repo_root().join("scripts/orchestration/phase0.sh");
    Command::new("bash")
        .current_dir(repo_root())
        .env("PATH", path_env)
        .env("TANREN_PHASE_CAPABILITIES", ALL_CAPS)
        .arg(script)
        .arg("--spec-id")
        .arg(spec_id)
        .arg("--spec-folder")
        .arg(spec_folder)
        .arg("--config")
        .arg(config)
        .arg("--database-url")
        .arg(db_url)
        .arg("--output-mode")
        .arg(output_mode)
        .arg("--dry-run")
        .output()
        .expect("run phase0.sh")
}

#[test]
fn phase0_script_routes_implemented_task_to_gate_step() {
    let (_tmp, db_url) = mkdb();
    let spec_id = "00000000-0000-0000-0000-000000000ca1";
    let root = tempfile::tempdir().expect("root dir");
    let spec_folder = root.path().join(spec_id);
    fs::create_dir_all(&spec_folder).expect("mkdir spec");
    let config = repo_root().join("tanren.yml");

    let task_id = create_task(&db_url, spec_id, &spec_folder, "T01");
    run_task_mutation(
        &db_url,
        spec_id,
        &spec_folder,
        "do-task",
        "start",
        &format!("{{\"schema_version\":\"1.0.0\",\"task_id\":\"{task_id}\"}}"),
    );
    run_task_mutation(
        &db_url,
        spec_id,
        &spec_folder,
        "do-task",
        "complete",
        &format!("{{\"schema_version\":\"1.0.0\",\"task_id\":\"{task_id}\",\"evidence_refs\":[]}}"),
    );
    run_task_mutation(
        &db_url,
        spec_id,
        &spec_folder,
        "audit-task",
        "guard",
        &format!(
            "{{\"schema_version\":\"1.0.0\",\"task_id\":\"{task_id}\",\"guard\":\"audited\"}}"
        ),
    );

    let shim = mk_shim_dir();
    let path_env = format!(
        "{}:{}",
        shim.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = run_phase0(
        &path_env,
        &config,
        &db_url,
        spec_id,
        &spec_folder,
        "verbose",
    );
    assert!(
        out.status.success(),
        "phase0.sh failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("step=task_gate"),
        "expected gate step in stdout: {stdout}"
    );
    assert!(
        stdout.contains("[dry-run] task_verification_hook"),
        "expected gate hook dry-run in stdout: {stdout}"
    );
    assert!(
        stdout.contains("[dry-run] task_guard_gate_checked"),
        "expected guard mutation dry-run in stdout: {stdout}"
    );
    assert!(
        !stdout.contains("harness phase do-task"),
        "did not expect do-task harness for implemented task: {stdout}"
    );
}

#[test]
fn phase0_script_output_modes_render_expected_verbosity() {
    let (_tmp, db_url) = mkdb();
    let spec_id = "00000000-0000-0000-0000-000000000ca2";
    let root = tempfile::tempdir().expect("root dir");
    let spec_folder = root.path().join(spec_id);
    fs::create_dir_all(&spec_folder).expect("mkdir spec");
    let config = repo_root().join("tanren.yml");
    let _task_id = create_task(&db_url, spec_id, &spec_folder, "T01");

    let shim = mk_shim_dir();
    let path_env = format!(
        "{}:{}",
        shim.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );

    let out_silent = run_phase0(&path_env, &config, &db_url, spec_id, &spec_folder, "silent");
    assert!(
        out_silent.status.success(),
        "silent mode failed: {}",
        String::from_utf8_lossy(&out_silent.stderr)
    );
    let silent = String::from_utf8_lossy(&out_silent.stdout);
    assert!(
        silent.contains("task 1/1 - task_do_task (implementing)"),
        "silent output: {silent}"
    );
    assert!(
        !silent.contains("definition:"),
        "silent mode should stay minimal: {silent}"
    );

    let out_quiet = run_phase0(&path_env, &config, &db_url, spec_id, &spec_folder, "quiet");
    assert!(
        out_quiet.status.success(),
        "quiet mode failed: {}",
        String::from_utf8_lossy(&out_quiet.stderr)
    );
    let quiet = String::from_utf8_lossy(&out_quiet.stdout);
    assert!(quiet.contains("definition:"), "quiet output: {quiet}");
    assert!(quiet.contains("deliverable:"), "quiet output: {quiet}");
    assert!(
        quiet.contains("[dry-run] harness phase do-task"),
        "quiet output: {quiet}"
    );

    let out_verbose = run_phase0(
        &path_env,
        &config,
        &db_url,
        spec_id,
        &spec_folder,
        "verbose",
    );
    assert!(
        out_verbose.status.success(),
        "verbose mode failed: {}",
        String::from_utf8_lossy(&out_verbose.stderr)
    );
    let verbose = String::from_utf8_lossy(&out_verbose.stdout);
    assert!(
        verbose.contains("cycle 1: task 1/1"),
        "verbose output: {verbose}"
    );
    assert!(
        verbose.contains("[dry-run] harness phase do-task"),
        "verbose output: {verbose}"
    );
}

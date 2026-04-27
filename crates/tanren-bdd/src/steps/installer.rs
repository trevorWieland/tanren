use std::fs;
use std::path::Path;

use cucumber::{given, then, when};
use tanren_testkit::fs_assertions::{collect_file_snapshot, read_file, remove_file, write_file};
use tanren_testkit::temp_repo::TempRepo;

use crate::world::BehaviorWorld;

fn assert_success(world: &BehaviorWorld) {
    let output = world.installer_output.as_ref();
    assert_eq!(
        output.and_then(|item| item.status),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        output.map_or("", |item| item.stdout.as_str()),
        output.map_or("", |item| item.stderr.as_str())
    );
}

fn assert_validation_failure(world: &BehaviorWorld) {
    let output = world.installer_output.as_ref();
    assert_eq!(
        output.and_then(|item| item.status),
        Some(4),
        "stdout:\n{}\nstderr:\n{}",
        output.map_or("", |item| item.stdout.as_str()),
        output.map_or("", |item| item.stderr.as_str())
    );
}

fn assert_exists(path: &Path) {
    assert!(path.exists(), "expected {} to exist", path.display());
}

fn assert_missing(path: &Path) {
    assert!(!path.exists(), "expected {} to be missing", path.display());
}

#[given("an empty target repository")]
fn given_empty_target_repository(world: &mut BehaviorWorld) {
    world.installer_repo =
        Some(TempRepo::create("tanren-bdd-install").expect("create temporary repo"));
    world.installer_output = None;
    world.installer_snapshot.clear();
}

#[given("a bootstrapped rust-cargo repository")]
fn given_bootstrapped_rust_cargo_repository(world: &mut BehaviorWorld) {
    given_empty_target_repository(world);
    world.run_cli(
        vec!["install".into(), "--profile".into(), "rust-cargo".into()],
        true,
    );
    assert_success(world);
}

#[given("a rendered command, unmanaged command file, edited standard, and missing standard")]
fn given_reinstall_mutations(world: &mut BehaviorWorld) {
    let repo = world.repo_path();
    write_file(
        &repo.join(".claude/commands/do-task.md"),
        "local command edit\n",
    )
    .expect("write rendered command mutation");
    write_file(&repo.join(".claude/commands/stale.md"), "stale command\n")
        .expect("write stale command");
    write_file(
        &repo.join("tanren/standards/rust/error-handling.md"),
        "local standard edit\n",
    )
    .expect("write edited standard");
    remove_file(&repo.join("tanren/standards/rust/workspace-lints.md"))
        .expect("remove installed standard");
}

#[given("a rendered command has local drift")]
fn given_rendered_command_has_local_drift(world: &mut BehaviorWorld) {
    let repo = world.repo_path();
    write_file(
        &repo.join(".claude/commands/do-task.md"),
        "local command drift\n",
    )
    .expect("write command drift");
    world.installer_snapshot = collect_file_snapshot(repo).expect("snapshot repo");
}

#[given("an installed standard is missing")]
fn given_installed_standard_is_missing(world: &mut BehaviorWorld) {
    let repo = world.repo_path();
    remove_file(&repo.join("tanren/standards/rust/workspace-lints.md"))
        .expect("remove installed standard");
    world.installer_snapshot = collect_file_snapshot(repo).expect("snapshot repo");
}

#[given("an installed standard has local edits")]
fn given_installed_standard_has_local_edits(world: &mut BehaviorWorld) {
    let repo = world.repo_path();
    write_file(
        &repo.join("tanren/standards/rust/error-handling.md"),
        "local standard edit\n",
    )
    .expect("write edited standard");
    world.installer_snapshot = collect_file_snapshot(repo).expect("snapshot repo");
}

#[given("a target repository with legacy methodology profiles")]
fn given_target_repository_with_legacy_profiles(world: &mut BehaviorWorld) {
    given_empty_target_repository(world);
    write_file(
        &world.repo_path().join("tanren.yml"),
        r"methodology:
  source:
    path: commands
  install_targets:
    - path: tanren/standards
      format: standards-baseline
      binding: none
      merge_policy: preserve_existing
  profiles:
    rust-cargo:
      variables:
        project_language: rust
",
    )
    .expect("write legacy config");
}

#[given("an existing MCP config is malformed")]
fn given_existing_mcp_config_is_malformed(world: &mut BehaviorWorld) {
    let repo = world.repo_path();
    write_file(
        &repo.join(".claude/commands/do-task.md"),
        "local command before mcp failure\n",
    )
    .expect("write command before failure");
    write_file(&repo.join(".mcp.json"), "{not-json\n").expect("write malformed MCP config");
}

#[given("the runtime standards root is missing")]
fn given_runtime_standards_root_missing(world: &mut BehaviorWorld) {
    let root = world.repo_path().join("tanren/standards");
    fs::remove_dir_all(&root).expect("remove standards root");
}

#[when(expr = "install is run with profile {string}")]
fn when_install_run_with_profile(world: &mut BehaviorWorld, profile: String) {
    world.run_cli(vec!["install".into(), "--profile".into(), profile], true);
}

#[when(expr = "install is run with profile {string} and agents {string}")]
fn when_install_run_with_profile_and_agents(
    world: &mut BehaviorWorld,
    profile: String,
    agents: String,
) {
    world.run_cli(
        vec![
            "install".into(),
            "--profile".into(),
            profile,
            "--agents".into(),
            agents,
        ],
        true,
    );
}

#[when(expr = "install is run with invalid profile {string}")]
fn when_install_run_with_invalid_profile(world: &mut BehaviorWorld, profile: String) {
    world.run_cli(vec!["install".into(), "--profile".into(), profile], true);
}

#[when("install is run without bootstrap flags")]
fn when_install_run_without_bootstrap_flags(world: &mut BehaviorWorld) {
    world.run_cli(vec!["install".into()], true);
}

#[when("install is run again")]
fn when_install_run_again(world: &mut BehaviorWorld) {
    world.run_cli(vec!["install".into()], true);
}

#[when("strict dry-run install is executed")]
fn when_strict_dry_run_install_executed(world: &mut BehaviorWorld) {
    world.run_cli(
        vec!["install".into(), "--dry-run".into(), "--strict".into()],
        true,
    );
}

#[when("a methodology command loads runtime standards")]
fn when_methodology_command_loads_runtime_standards(world: &mut BehaviorWorld) {
    let config = world
        .repo_path()
        .join("tanren.yml")
        .to_string_lossy()
        .to_string();
    let database_url = format!(
        "sqlite:{}?mode=rwc",
        world.repo_path().join("tanren.db").to_string_lossy()
    );
    world.run_cli(
        vec![
            "--database-url".into(),
            database_url.clone(),
            "db".into(),
            "migrate".into(),
        ],
        false,
    );
    assert_success(world);
    world.run_cli(
        vec![
            "--database-url".into(),
            database_url,
            "methodology".into(),
            "--methodology-config".into(),
            config,
            "--phase".into(),
            "discover-standards".into(),
            "standard".into(),
            "list".into(),
            "--json".into(),
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000001"}"#.into(),
        ],
        false,
    );
}

#[then("bootstrap writes tanren config, commands, MCP configs, and rust standards")]
fn then_bootstrap_writes_full_default_targets(world: &mut BehaviorWorld) {
    assert_success(world);
    let repo = world.repo_path();
    for rel in [
        "tanren.yml",
        ".claude/commands/do-task.md",
        ".codex/skills/do-task/SKILL.md",
        ".opencode/commands/do-task.md",
        ".mcp.json",
        ".codex/config.toml",
        "opencode.json",
        "tanren/standards/rust/error-handling.md",
    ] {
        assert_exists(&repo.join(rel));
    }
}

#[then(expr = "generated config records profile {string} and all default agents")]
fn then_generated_config_records_profile_and_agents(world: &mut BehaviorWorld, profile: String) {
    let config = read_file(&world.repo_path().join("tanren.yml")).expect("read generated config");
    let expected = format!("profile: {profile}");
    drop(profile);
    assert!(config.contains(&expected));
    assert!(config.contains("format: claude-code"));
    assert!(config.contains("format: codex-skills"));
    assert!(config.contains("format: opencode"));
    assert!(config.contains("format: standards-profile"));
}

#[then("bootstrap writes only the codex command and MCP config targets")]
fn then_bootstrap_writes_only_codex_targets(world: &mut BehaviorWorld) {
    assert_success(world);
    let repo = world.repo_path();
    assert_exists(&repo.join("tanren.yml"));
    assert_exists(&repo.join(".codex/skills/do-task/SKILL.md"));
    assert_exists(&repo.join(".codex/config.toml"));
    assert_exists(&repo.join("tanren/standards/rust/error-handling.md"));
    assert_missing(&repo.join(".claude/commands/do-task.md"));
    assert_missing(&repo.join(".mcp.json"));
    assert_missing(&repo.join(".opencode/commands/do-task.md"));
    assert_missing(&repo.join("opencode.json"));
}

#[then("rendered commands are exhaustively replaced")]
fn then_rendered_commands_are_exhaustively_replaced(world: &mut BehaviorWorld) {
    assert_success(world);
    let repo = world.repo_path();
    let command = read_file(&repo.join(".claude/commands/do-task.md")).expect("read command");
    assert!(command.contains("# do-task"));
    assert!(!command.contains("local command edit"));
    assert_missing(&repo.join(".claude/commands/stale.md"));
}

#[then("edited standards are preserved while missing standards are restored")]
fn then_edited_standards_preserved_missing_restored(world: &mut BehaviorWorld) {
    let repo = world.repo_path();
    assert_eq!(
        read_file(&repo.join("tanren/standards/rust/error-handling.md"))
            .expect("read edited standard"),
        "local standard edit\n"
    );
    assert_exists(&repo.join("tanren/standards/rust/workspace-lints.md"));
}

#[then("install exits with drift status")]
fn then_install_exits_with_drift_status(world: &mut BehaviorWorld) {
    let output = world.installer_output.as_ref();
    assert_eq!(
        output.and_then(|item| item.status),
        Some(3),
        "stdout:\n{}\nstderr:\n{}",
        output.map_or("", |item| item.stdout.as_str()),
        output.map_or("", |item| item.stderr.as_str())
    );
}

#[then("install exits successfully")]
fn then_install_exits_successfully(world: &mut BehaviorWorld) {
    assert_success(world);
}

#[then("strict dry-run performs no mutation")]
fn then_strict_dry_run_performs_no_mutation(world: &mut BehaviorWorld) {
    assert_eq!(
        world.installer_snapshot,
        collect_file_snapshot(world.repo_path()).expect("snapshot repo")
    );
}

#[then("install fails validation")]
fn then_install_fails_validation(world: &mut BehaviorWorld) {
    assert_validation_failure(world);
}

#[then("no bootstrap files are written")]
fn then_no_bootstrap_files_written(world: &mut BehaviorWorld) {
    let repo = world.repo_path();
    assert_missing(&repo.join("tanren.yml"));
    assert_missing(&repo.join(".codex"));
    assert_missing(&repo.join("tanren"));
}

#[then("rendered commands are not rewritten after the MCP failure")]
fn then_rendered_commands_not_rewritten_after_mcp_failure(world: &mut BehaviorWorld) {
    assert_validation_failure(world);
    let command =
        read_file(&world.repo_path().join(".claude/commands/do-task.md")).expect("read command");
    assert_eq!(command, "local command before mcp failure\n");
}

#[then("runtime standards loading fails explicitly")]
fn then_runtime_standards_loading_fails_explicitly(world: &mut BehaviorWorld) {
    let output = world
        .installer_output
        .as_ref()
        .expect("methodology command output");
    assert_eq!(output.status, Some(1));
    assert!(
        output.stderr.contains("standards root") && output.stderr.contains("does not exist"),
        "stderr:\n{}",
        output.stderr
    );
}

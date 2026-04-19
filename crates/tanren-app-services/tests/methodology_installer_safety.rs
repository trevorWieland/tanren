use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use tanren_app_services::methodology::MethodologyError;
use tanren_app_services::methodology::config::{
    InstallBinding, InstallFormat, InstallTarget, MergePolicy, MethodologyConfig, SourceConfig,
};
use tanren_app_services::methodology::installer::{
    InstallPlan, PlannedWrite, apply_install, drift, plan_install,
};
use tanren_app_services::methodology::source::{CommandFamily, CommandFrontmatter, CommandSource};
use tempfile::TempDir;

fn workspace_tempdir() -> TempDir {
    let base = std::env::current_dir()
        .expect("cwd")
        .join("target")
        .join("installer-tests");
    std::fs::create_dir_all(&base).expect("mkdir base");
    tempfile::Builder::new()
        .prefix("installer-")
        .tempdir_in(base)
        .expect("tempdir")
}

#[test]
fn empty_plan_has_no_drift() {
    let plan = InstallPlan { writes: vec![] };
    assert!(drift(&plan).is_empty());
}

#[test]
fn plan_install_applies_task_tool_binding_per_target() {
    let cfg = MethodologyConfig {
        task_complete_requires: vec![],
        source: SourceConfig {
            path: PathBuf::from("commands"),
        },
        install_targets: vec![
            InstallTarget {
                path: PathBuf::from(".claude/commands"),
                format: InstallFormat::ClaudeCode,
                binding: InstallBinding::Mcp,
                merge_policy: MergePolicy::Destructive,
            },
            InstallTarget {
                path: PathBuf::from(".opencode/commands"),
                format: InstallFormat::Opencode,
                binding: InstallBinding::Cli,
                merge_policy: MergePolicy::Destructive,
            },
        ],
        mcp: Default::default(),
        variables: BTreeMap::new(),
        profiles: BTreeMap::new(),
    };
    let command = CommandSource {
        name: "do-task".into(),
        family: CommandFamily::SpecLoop,
        frontmatter: CommandFrontmatter {
            name: "do-task".into(),
            role: "impl".into(),
            orchestration_loop: true,
            autonomy: "autonomous".into(),
            declared_variables: vec!["TASK_TOOL_BINDING".into()],
            declared_tools: vec![],
            required_capabilities: vec![],
            produces_evidence: vec![],
            extras: BTreeMap::new(),
        },
        body: "binding={{TASK_TOOL_BINDING}}\n".into(),
        source_path: PathBuf::from("commands/spec/do-task.md"),
    };
    let mut ctx = HashMap::new();
    ctx.insert("TASK_TOOL_BINDING".into(), "mcp".into());
    let plan = plan_install(&cfg, &[command], &ctx).expect("plan");
    let mut claude = None;
    let mut opencode = None;
    for w in plan.writes {
        let body = String::from_utf8(w.bytes).expect("utf8");
        let is_task_file = w.dest.ends_with("do-task.md");
        let has_mcp = body.contains("binding=mcp");
        let has_cli = body.contains("binding=cli");
        if is_task_file && has_mcp {
            claude = Some(body.clone());
        }
        if is_task_file && has_cli {
            opencode = Some(body);
        }
    }
    assert!(claude.is_some(), "mcp binding expected in claude target");
    assert!(
        opencode.is_some(),
        "cli binding expected in opencode target"
    );
}

#[test]
fn apply_install_prunes_extras_for_destructive_targets() {
    let dir = workspace_tempdir();
    let root = dir.path().join(".claude/commands");
    std::fs::create_dir_all(&root).expect("mkdir");
    let stale = root.join("stale.md");
    std::fs::write(&stale, "stale").expect("seed stale");
    let dest = root.join("do-task.md");
    let plan = InstallPlan {
        writes: vec![PlannedWrite {
            dest: dest.clone(),
            bytes: b"fresh\n".to_vec(),
            merge_policy: MergePolicy::Destructive,
            format: InstallFormat::ClaudeCode,
        }],
    };

    let written = apply_install(&plan).expect("apply");
    assert_eq!(written, vec![dest.clone()]);
    assert!(
        !stale.exists(),
        "destructive apply must prune unmanaged files"
    );
    assert_eq!(std::fs::read_to_string(dest).expect("read"), "fresh\n");
}

#[test]
fn apply_install_does_not_prune_extras_for_preserve_existing() {
    let dir = workspace_tempdir();
    let root = dir.path().join("tanren/standards/security");
    std::fs::create_dir_all(&root).expect("mkdir");
    let extra = root.join("local-note.md");
    std::fs::write(&extra, "keep me").expect("seed extra");
    let managed = root.join("input-validation.md");
    let plan = InstallPlan {
        writes: vec![PlannedWrite {
            dest: managed.clone(),
            bytes: b"managed\n".to_vec(),
            merge_policy: MergePolicy::PreserveExisting,
            format: InstallFormat::StandardsBaseline,
        }],
    };

    let _ = apply_install(&plan).expect("apply");
    assert!(
        extra.exists(),
        "preserve_existing targets must not prune unmanaged files"
    );
}

#[test]
fn apply_install_rejects_unsafe_destructive_root() {
    let plan = InstallPlan {
        writes: vec![PlannedWrite {
            dest: PathBuf::from("do-task.md"),
            bytes: b"x".to_vec(),
            merge_policy: MergePolicy::Destructive,
            format: InstallFormat::ClaudeCode,
        }],
    };

    let err = apply_install(&plan).expect_err("unsafe root must fail");
    assert!(
        matches!(err, MethodologyError::Validation(ref msg) if msg.contains("unsafe root")),
        "unexpected error: {err:?}"
    );
}

#[test]
fn apply_install_rejects_parent_traversal_destructive_root() {
    let plan = InstallPlan {
        writes: vec![PlannedWrite {
            dest: PathBuf::from("../escape/do-task.md"),
            bytes: b"x".to_vec(),
            merge_policy: MergePolicy::Destructive,
            format: InstallFormat::ClaudeCode,
        }],
    };

    let err = apply_install(&plan).expect_err("traversal root must fail");
    assert!(
        matches!(err, MethodologyError::Validation(ref msg) if msg.contains("path traversal")),
        "unexpected error: {err:?}"
    );
}

#[cfg(unix)]
#[test]
fn apply_install_rejects_symlink_escape_root() {
    let inside = workspace_tempdir();
    let outside = tempfile::tempdir().expect("outside tempdir");
    let link = inside.path().join("escape-link");
    std::os::unix::fs::symlink(outside.path(), &link).expect("symlink");
    let plan = InstallPlan {
        writes: vec![PlannedWrite {
            dest: link.join("do-task.md"),
            bytes: b"x".to_vec(),
            merge_policy: MergePolicy::Destructive,
            format: InstallFormat::ClaudeCode,
        }],
    };

    let err = apply_install(&plan).expect_err("symlink escape must fail");
    assert!(
        matches!(err, MethodologyError::Validation(ref msg) if msg.contains("escapes workspace")),
        "unexpected error: {err:?}"
    );
}

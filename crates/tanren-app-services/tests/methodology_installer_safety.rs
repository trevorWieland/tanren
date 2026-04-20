use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tanren_app_services::methodology::MethodologyError;
use tanren_app_services::methodology::config::{
    InstallBinding, InstallFormat, InstallTarget, MergePolicy, MethodologyConfig, SourceConfig,
};
use tanren_app_services::methodology::formats::render_commands;
use tanren_app_services::methodology::installer::{
    DriftReason, InstallPlan, PlannedWrite, apply_install, drift, plan_install,
};
use tanren_app_services::methodology::renderer::{CanonicalBytes, render_catalog};
use tanren_app_services::methodology::source::load_catalog;
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

fn default_render_context() -> HashMap<String, String> {
    HashMap::from([
        ("TASK_VERIFICATION_HOOK".into(), "just check".into()),
        ("SPEC_VERIFICATION_HOOK".into(), "just ci".into()),
        ("ISSUE_PROVIDER".into(), "GitHub".into()),
        ("PROJECT_LANGUAGE".into(), "rust".into()),
        ("SPEC_ROOT".into(), "tanren/specs".into()),
        ("PRODUCT_ROOT".into(), "tanren/product".into()),
        ("STANDARDS_ROOT".into(), "tanren/standards".into()),
        ("AGENT_CLI_NOUN".into(), "the agent CLI".into()),
        ("TASK_TOOL_BINDING".into(), "mcp".into()),
        ("PHASE_EVENTS_FILE".into(), "phase-events.jsonl".into()),
        ("ADHERE_SPEC_HOOK".into(), "just check".into()),
        ("ADHERE_TASK_HOOK".into(), "just check".into()),
        ("AUDIT_SPEC_HOOK".into(), "just check".into()),
        ("AUDIT_TASK_HOOK".into(), "just check".into()),
        ("DEMO_HOOK".into(), "just check".into()),
        ("RUN_DEMO_HOOK".into(), "just check".into()),
        (
            "PILLAR_LIST".into(),
            "completeness, performance, security".into(),
        ),
        ("ISSUE_REF_NOUN".into(), "GitHub issue".into()),
        ("PR_NOUN".into(), "pull request".into()),
        (
            "READONLY_ARTIFACT_BANNER".into(),
            "ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT".into(),
        ),
    ])
}

fn split_frontmatter(doc: &str) -> (&str, &str) {
    assert!(doc.starts_with("---\n"), "expected YAML frontmatter");
    let after = &doc[4..];
    let end = after
        .find("\n---\n")
        .expect("expected frontmatter closing fence");
    let fm = &after[..end];
    let body = &after[end + 5..];
    (fm, body)
}

fn command_name_from_dest(format: InstallFormat, dest: &Path) -> String {
    if format == InstallFormat::CodexSkills {
        dest.parent()
            .and_then(Path::file_name)
            .and_then(std::ffi::OsStr::to_str)
            .expect("codex command dir")
            .to_owned()
    } else {
        dest.file_stem()
            .and_then(std::ffi::OsStr::to_str)
            .expect("markdown command stem")
            .to_owned()
    }
}

fn semantic_body_hashes(
    rendered: &[tanren_app_services::methodology::renderer::RenderedCommand],
    format: InstallFormat,
    dest_root: &Path,
) -> BTreeMap<String, String> {
    let artifacts = render_commands(rendered, format, dest_root).expect("render commands");
    let mut out = BTreeMap::new();
    for artifact in artifacts {
        let text = String::from_utf8(artifact.bytes).expect("artifact utf8");
        let body = if format == InstallFormat::Opencode {
            let (fm, _body) = split_frontmatter(&text);
            let fm_yaml: serde_yaml::Value = serde_yaml::from_str(fm).expect("parse yaml");
            fm_yaml
                .get("template")
                .and_then(serde_yaml::Value::as_str)
                .expect("template in opencode frontmatter")
                .to_owned()
        } else {
            let (_fm, body) = split_frontmatter(&text);
            body.to_owned()
        };
        assert!(
            !body.contains("report_phase_outcome(\"fail\""),
            "phase-outcome vocabulary must not drift back to `fail`"
        );
        let canonical = CanonicalBytes::canonicalize(&body);
        let hash = format!("{:x}", Sha256::digest(&canonical.0));
        out.insert(command_name_from_dest(format, &artifact.dest), hash);
    }
    out
}

#[test]
fn empty_plan_has_no_drift() {
    let plan = InstallPlan { writes: vec![] };
    assert!(drift(&plan).expect("drift").is_empty());
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
        rubric: Default::default(),
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
            description: None,
            agent: None,
            model: None,
            subtask: None,
            extensions: BTreeMap::new(),
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

#[cfg(unix)]
#[test]
fn apply_install_prunes_nested_symlink_without_following_target() {
    let inside = workspace_tempdir();
    let root = inside.path().join(".claude/commands");
    std::fs::create_dir_all(&root).expect("mkdir");
    let outside = tempfile::tempdir().expect("outside tempdir");
    let outside_file = outside.path().join("outside.md");
    std::fs::write(&outside_file, "outside").expect("seed outside");
    let nested = root.join("nested");
    std::os::unix::fs::symlink(outside.path(), &nested).expect("nested symlink");

    let dest = root.join("do-task.md");
    let plan = InstallPlan {
        writes: vec![PlannedWrite {
            dest,
            bytes: b"fresh\n".to_vec(),
            merge_policy: MergePolicy::Destructive,
            format: InstallFormat::ClaudeCode,
        }],
    };

    let written = apply_install(&plan).expect("apply should succeed");
    assert!(
        written.contains(&root.join("do-task.md")),
        "managed write should be applied"
    );
    assert!(
        !nested.exists(),
        "prune should remove nested symlink entry without traversing target"
    );
    assert!(
        outside_file.exists(),
        "prune must not follow symlink target outside root"
    );
}

#[cfg(unix)]
#[test]
fn drift_reports_nested_symlink_as_extra_file_without_following_target() {
    let inside = workspace_tempdir();
    let root = inside.path().join(".claude/commands");
    std::fs::create_dir_all(&root).expect("mkdir");
    let outside = tempfile::tempdir().expect("outside tempdir");
    let outside_file = outside.path().join("outside.md");
    std::fs::write(&outside_file, "outside").expect("seed outside");
    let nested = root.join("nested");
    std::os::unix::fs::symlink(outside.path(), &nested).expect("nested symlink");

    let plan = InstallPlan {
        writes: vec![PlannedWrite {
            dest: root.join("do-task.md"),
            bytes: b"fresh\n".to_vec(),
            merge_policy: MergePolicy::Destructive,
            format: InstallFormat::ClaudeCode,
        }],
    };

    let drifts = drift(&plan).expect("drift should succeed");
    assert!(
        drifts
            .iter()
            .any(|entry| entry.dest == nested && matches!(entry.reason, DriftReason::ExtraFile)),
        "nested symlink should be treated as an extra file entry"
    );
    assert!(
        outside_file.exists(),
        "drift scan must not follow symlink target outside root"
    );
}

#[cfg(unix)]
#[test]
fn apply_install_prunes_dangling_symlink_without_error() {
    let inside = workspace_tempdir();
    let root = inside.path().join(".claude/commands");
    std::fs::create_dir_all(&root).expect("mkdir");
    let dangling = root.join("dangling");
    std::os::unix::fs::symlink(root.join("missing-target"), &dangling).expect("dangling symlink");

    let plan = InstallPlan {
        writes: vec![PlannedWrite {
            dest: root.join("do-task.md"),
            bytes: b"fresh\n".to_vec(),
            merge_policy: MergePolicy::Destructive,
            format: InstallFormat::ClaudeCode,
        }],
    };

    apply_install(&plan).expect("apply should prune dangling symlink");
    assert!(
        !dangling.exists(),
        "dangling symlink extra file should be pruned safely"
    );
}

#[test]
fn command_matrix_semantic_hashes_match_across_all_targets() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let commands_root = repo_root.join("commands");
    let catalog = load_catalog(&commands_root).expect("load command catalog");
    assert_eq!(catalog.len(), 17, "lane 0.5 command matrix is 17 commands");
    for command in &catalog {
        assert!(
            !command.body.contains("report_phase_outcome(\"fail\""),
            "source command `{}` drifted to invalid phase-outcome vocabulary",
            command.name
        );
    }

    let (rendered, _refs) = render_catalog(&catalog, &default_render_context()).expect("render");
    assert_eq!(
        rendered.len(),
        17,
        "rendered matrix must include all 17 commands"
    );
    let claude = semantic_body_hashes(
        &rendered,
        InstallFormat::ClaudeCode,
        Path::new(".claude/commands"),
    );
    let codex = semantic_body_hashes(
        &rendered,
        InstallFormat::CodexSkills,
        Path::new(".codex/skills"),
    );
    let opencode = semantic_body_hashes(
        &rendered,
        InstallFormat::Opencode,
        Path::new(".opencode/commands"),
    );
    assert_eq!(
        claude.len(),
        17,
        "claude hash matrix must cover all commands"
    );
    assert_eq!(codex.len(), 17, "codex hash matrix must cover all commands");
    assert_eq!(
        opencode.len(),
        17,
        "opencode hash matrix must cover all commands"
    );
    assert_eq!(
        claude, codex,
        "claude and codex semantic body hashes must match"
    );
    assert_eq!(
        claude, opencode,
        "claude and opencode semantic body hashes must match"
    );
}

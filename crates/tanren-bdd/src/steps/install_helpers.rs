//! Shared install-step assertions that do not need scenario-local state.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path};

pub(crate) fn assert_rust_cargo_default_assets_installed(repository_root: &Path) {
    let command_files = list_relative_files_under_workspace("commands/project");
    assert!(
        !command_files.is_empty(),
        "expected workspace command catalog to have at least one file"
    );
    for command_file in &command_files {
        for integration_destination in [
            format!(".claude/commands/{command_file}"),
            format!(".codex/skills/{command_file}"),
            format!(".opencode/commands/{command_file}"),
        ] {
            assert_file_exists(repository_root, &integration_destination);
        }
    }

    let standards_files = list_relative_files_under_workspace("profiles/rust-cargo");
    assert!(
        !standards_files.is_empty(),
        "expected rust-cargo profile catalog to have at least one file"
    );
    for standard_file in standards_files {
        assert_file_exists(
            repository_root,
            &format!("profiles/rust-cargo/{standard_file}"),
        );
    }
}

pub(crate) fn assert_manifest_rust_cargo_defaults(repository_root: &Path) {
    let manifest_path = repository_root.join(".tanren/install-manifest.toml");
    let manifest = fs::read_to_string(&manifest_path).expect("read install manifest from fixture");
    for expected in [
        "manifest_version = 1",
        "profile = \"rust-cargo\"",
        "integrations = [\"claude\", \"codex\", \"open-code\"]",
        "asset_class = \"methodology-command\"",
        "asset_class = \"standards-profile\"",
    ] {
        assert!(
            manifest.contains(expected),
            "expected install manifest to contain `{expected}`; got:\n{manifest}",
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct RepositoryRelativePath(String);

impl RepositoryRelativePath {
    pub(crate) fn parse(raw: String) -> Self {
        validate_relative_path(raw.as_str());
        Self(raw)
    }

    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

pub(crate) fn validate_relative_path(path: &str) {
    assert!(!path.is_empty(), "repository-relative path cannot be empty");
    let candidate = Path::new(path);
    assert!(
        !candidate.is_absolute(),
        "repository-relative path must not be absolute: {path}"
    );
    let is_valid = candidate
        .components()
        .all(|component| matches!(component, Component::Normal(_) | Component::CurDir));
    assert!(
        is_valid,
        "repository-relative path must not contain traversal components: {path}"
    );
}

pub(crate) fn append_stale_generated_manifest_entry(
    manifest: &mut String,
    relative_path: &RepositoryRelativePath,
) {
    let entry = GeneratedManifestEntry::stale_methodology_command(relative_path.clone());
    manifest.push_str(&entry.to_manifest_block());
}

fn assert_file_exists(repository_root: &Path, relative_path: &str) {
    let absolute = repository_root.join(relative_path);
    assert!(
        absolute.exists(),
        "expected repository file to exist: {}",
        absolute.display()
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GeneratedManifestEntry {
    path: RepositoryRelativePath,
    content_hash: &'static str,
    asset_class: &'static str,
    integration: &'static str,
    preservation: &'static str,
}

impl GeneratedManifestEntry {
    fn stale_methodology_command(path: RepositoryRelativePath) -> Self {
        Self {
            path,
            content_hash: "stale-generated-entry",
            asset_class: "methodology-command",
            integration: "codex",
            preservation: "replace-generated",
        }
    }

    fn to_manifest_block(&self) -> String {
        format!(
            "\n[[entries]]\npath = \"{}\"\ncontent_hash = \"{}\"\nasset_class = \"{}\"\nintegration = \"{}\"\npreservation = \"{}\"\n",
            self.path.as_str(),
            self.content_hash,
            self.asset_class,
            self.integration,
            self.preservation
        )
    }
}

fn list_relative_files_under_workspace(relative_root: &str) -> Vec<String> {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonicalize workspace root for BDD install steps");
    let root = workspace_root.join(relative_root);
    assert!(
        root.exists() && root.is_dir(),
        "expected workspace catalog root to exist: {}",
        root.display()
    );

    let mut files = BTreeMap::new();
    collect_files(&root, &root, &mut files);
    files.into_keys().collect()
}

fn collect_files(root: &Path, cursor: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
    let entries = fs::read_dir(cursor).expect("read workspace catalog directory");
    for entry in entries {
        let entry = entry.expect("inspect workspace catalog directory entry");
        let path = entry.path();
        let file_type = entry.file_type().expect("inspect workspace file type");
        if file_type.is_dir() {
            collect_files(root, &path, out);
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .expect("workspace catalog path must stay under its root");
        let relative = relative.to_string_lossy().replace('\\', "/");
        let bytes = fs::read(&path).expect("read workspace catalog file");
        out.insert(relative, bytes);
    }
}

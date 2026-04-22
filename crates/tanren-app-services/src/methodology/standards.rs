//! Baseline standards bundled with `tanren-cli install`.
//!
//! `tanren-cli install` bootstraps a new repo's
//! `tanren/standards/<category>/<name>.md`
//! tree from this baseline, under the `preserve_existing` merge policy
//! — so adopters can extend or replace them without the installer
//! stomping their edits on reinstall.
//!
//! The set here is intentionally small (high-signal defaults covering
//! the Rust rewrite's core disciplines). Follow-up lanes will extend
//! and/or externalize this catalog.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use tanren_domain::methodology::evidence::frontmatter::split as split_frontmatter;
use tanren_domain::methodology::standard::{Standard, StandardImportance};
use tanren_domain::validated::NonEmptyString;

use super::errors::{MethodologyError, MethodologyResult};

/// Return the built-in baseline standards as typed records. Rendered
/// into `<root>/<category>/<name>.md` with `preserve_existing`.
#[must_use]
pub fn baseline_standards() -> Vec<Standard> {
    let raw: &[(
        &str,
        &str,
        &str,
        &str,
        &[&str],
        &[&str],
        &[&str],
        StandardImportance,
        &str,
    )] = &[
        (
            "thiserror-for-libraries",
            "rust-error-handling",
            "Libraries return `thiserror` enums; only binaries use `anyhow`.",
            "Rationale: library consumers need typed errors to match and recover. Binary top-levels get ergonomics via `anyhow`. Mixing produces unmatchable error trees at the boundary.",
            &["**/*.rs"],
            &["rust"],
            &["error-handling"],
            StandardImportance::High,
            "Libraries must return `thiserror`-derived enums; `anyhow` is permitted only in `bin/` crates.",
        ),
        (
            "no-unwrap-in-production",
            "rust-error-handling",
            "No `unwrap()` / `expect()` / `panic!()` / `todo!()` / `unimplemented!()` in library code.",
            "These force a runtime abort on a path the compiler would otherwise prove unreachable. Use match/Result or encode the invariant in the type.",
            &["**/*.rs"],
            &["rust"],
            &["error-handling"],
            StandardImportance::Critical,
            "Library code must not panic. Workspace-level clippy denies `unwrap_used`, `panic`, `todo`, `unimplemented`, `dbg_macro`. Test code may use `expect` for invariants that are themselves tested.",
        ),
        (
            "tracing-over-println",
            "rust-observability",
            "`println!` / `eprintln!` / `dbg!` are forbidden; use `tracing::info!` etc.",
            "Printing bypasses the observability pipeline (correlation ids, log levels, JSON output) and corrupts MCP stdio transports.",
            &["**/*.rs"],
            &["rust"],
            &["observability"],
            StandardImportance::Critical,
            "Emit via `tracing`; configure a stderr subscriber in binaries. Workspace-level clippy denies `print_stdout`, `print_stderr`, `dbg_macro`.",
        ),
        (
            "file-size-budget",
            "rust-style",
            "≤ 500 lines per `.rs` file; ≤ 100 lines per function.",
            "Keeps modules single-purpose and code-reviewable. Enforced by `just check-lines` and clippy's `too_many_lines`.",
            &["**/*.rs"],
            &["rust"],
            &["style"],
            StandardImportance::Medium,
            "Split files that grow past 500 lines by responsibility; extract helpers when functions exceed 100 lines.",
        ),
        (
            "secrecy-for-secrets",
            "rust-security",
            "Wrap secrets with `secrecy::Secret<T>`; never log or serialize raw values.",
            "Raw `String` passwords / tokens flow freely through Debug / Display / serde, leaking into logs and event payloads.",
            &["**/*.rs"],
            &["rust"],
            &["security"],
            StandardImportance::Critical,
            "Any field carrying an API key, token, or password must be `Secret<String>` (or a `Secret<CustomType>`); downstream code uses `expose_secret()` at point-of-use and never in a log call.",
        ),
    ];
    raw.iter()
        .filter_map(
            |(name, cat, _short, _why, applies, langs, domains, imp, body)| {
                let name = NonEmptyString::try_new(*name).ok()?;
                let category = NonEmptyString::try_new(*cat).ok()?;
                Some(Standard {
                    name,
                    category,
                    applies_to: applies.iter().map(|s| (*s).to_owned()).collect(),
                    applies_to_languages: langs.iter().map(|s| (*s).to_owned()).collect(),
                    applies_to_domains: domains.iter().map(|s| (*s).to_owned()).collect(),
                    importance: *imp,
                    body: (*body).to_owned(),
                })
            },
        )
        .collect()
}

/// Load runtime standards from the configured standards root.
///
/// Behavior:
/// - If `root` does not exist: fall back to bundled baseline standards.
/// - If `root` exists: load every `*.md` recursively and validate strict
///   metadata/frontmatter schema for each file.
/// - If `root` exists but is empty: hard error (misconfigured root).
///
/// # Errors
/// Returns [`MethodologyError::Validation`] when file schema/metadata is
/// invalid, or [`MethodologyError::Io`] on filesystem errors.
pub fn load_runtime_standards(root: &Path) -> MethodologyResult<Vec<Standard>> {
    if !root.exists() {
        return Ok(baseline_standards());
    }
    let files = collect_markdown_files(root)?;
    if files.is_empty() {
        return Err(MethodologyError::Validation(format!(
            "standards root `{}` exists but contains no .md standards",
            root.display()
        )));
    }
    let mut out = Vec::with_capacity(files.len());
    for file in files {
        out.push(parse_standard_file(root, &file)?);
    }
    out.sort_by(|a, b| {
        a.category
            .as_str()
            .cmp(b.category.as_str())
            .then(a.name.as_str().cmp(b.name.as_str()))
    });
    Ok(out)
}

fn collect_markdown_files(root: &Path) -> MethodologyResult<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|source| MethodologyError::Io {
            path: dir.clone(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| MethodologyError::Io {
                path: dir.clone(),
                source,
            })?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path
                .extension()
                .and_then(std::ffi::OsStr::to_str)
                .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
            {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

fn parse_standard_file(root: &Path, path: &Path) -> MethodologyResult<Standard> {
    let raw = std::fs::read_to_string(path).map_err(|source| MethodologyError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let (yaml, body) = split_frontmatter(&raw).map_err(|e| {
        MethodologyError::Validation(format!("invalid frontmatter in {}: {e}", path.display()))
    })?;
    let fm: StandardFrontmatterIn = serde_yaml::from_value(yaml).map_err(|e| {
        MethodologyError::Validation(format!(
            "standard frontmatter schema error in {}: {e}",
            path.display()
        ))
    })?;
    if fm.kind.as_str() != "standard" {
        return Err(MethodologyError::Validation(format!(
            "standard `{}` has invalid kind `{}`; expected `standard`",
            path.display(),
            fm.kind
        )));
    }
    let rel = path.strip_prefix(root).map_err(|_| {
        MethodologyError::Validation(format!(
            "standard file `{}` is outside configured root `{}`",
            path.display(),
            root.display()
        ))
    })?;
    let stem = path
        .file_stem()
        .and_then(std::ffi::OsStr::to_str)
        .ok_or_else(|| {
            MethodologyError::Validation(format!(
                "standard file `{}` has no valid UTF-8 stem",
                path.display()
            ))
        })?;
    let dir_category = rel
        .parent()
        .and_then(Path::file_name)
        .and_then(std::ffi::OsStr::to_str)
        .ok_or_else(|| {
            MethodologyError::Validation(format!(
                "standard file `{}` must be under `<root>/<category>/<name>.md`",
                path.display()
            ))
        })?;
    if dir_category != fm.category {
        return Err(MethodologyError::Validation(format!(
            "standard `{}` category mismatch: path has `{dir_category}`, frontmatter has `{}`",
            path.display(),
            fm.category
        )));
    }
    if stem != fm.name {
        return Err(MethodologyError::Validation(format!(
            "standard `{}` name mismatch: file stem `{stem}`, frontmatter name `{}`",
            path.display(),
            fm.name
        )));
    }
    let body = body.trim();
    if body.is_empty() {
        return Err(MethodologyError::Validation(format!(
            "standard `{}` body is empty",
            path.display()
        )));
    }
    Ok(Standard {
        name: NonEmptyString::try_new(fm.name).map_err(MethodologyError::Domain)?,
        category: NonEmptyString::try_new(fm.category).map_err(MethodologyError::Domain)?,
        applies_to: fm.applies_to,
        applies_to_languages: fm.applies_to_languages,
        applies_to_domains: fm.applies_to_domains,
        importance: parse_importance(&fm.importance).ok_or_else(|| {
            MethodologyError::Validation(format!(
                "standard `{}` has invalid importance `{}`",
                path.display(),
                fm.importance
            ))
        })?,
        body: body.to_owned(),
    })
}

/// Compute the install path for one standard under `root`:
/// `<root>/<category>/<name>.md`.
#[must_use]
pub fn standard_path(root: &Path, std: &Standard) -> PathBuf {
    root.join(std.category.as_str())
        .join(format!("{}.md", std.name.as_str()))
}

/// Render one standard to its canonical Markdown form. The body is a
/// full document with typed frontmatter so adopters can edit prose
/// safely; the installer uses `preserve_existing` to never overwrite
/// downstream edits.
#[must_use]
pub fn render_standard(std: &Standard) -> Vec<u8> {
    let fm = StandardFrontmatter {
        kind: "standard".into(),
        name: std.name.as_str().into(),
        category: std.category.as_str().into(),
        importance: importance_tag(std.importance).into(),
        applies_to: std.applies_to.clone(),
        applies_to_languages: std.applies_to_languages.clone(),
        applies_to_domains: std.applies_to_domains.clone(),
    };
    let yaml = serde_yaml::to_string(&fm).unwrap_or_default();
    let mut out = String::with_capacity(yaml.len() + std.body.len() + 32);
    out.push_str("---\n");
    out.push_str(&yaml);
    if !yaml.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("---\n\n");
    out.push_str(&std.body);
    if !std.body.ends_with('\n') {
        out.push('\n');
    }
    out.into_bytes()
}

const fn importance_tag(i: StandardImportance) -> &'static str {
    match i {
        StandardImportance::Low => "low",
        StandardImportance::Medium => "medium",
        StandardImportance::High => "high",
        StandardImportance::Critical => "critical",
    }
}

fn parse_importance(raw: &str) -> Option<StandardImportance> {
    match raw {
        "low" => Some(StandardImportance::Low),
        "medium" => Some(StandardImportance::Medium),
        "high" => Some(StandardImportance::High),
        "critical" => Some(StandardImportance::Critical),
        _ => None,
    }
}

#[derive(serde::Serialize)]
struct StandardFrontmatter {
    kind: String,
    name: String,
    category: String,
    importance: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    applies_to: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    applies_to_languages: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    applies_to_domains: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StandardFrontmatterIn {
    kind: String,
    name: String,
    category: String,
    importance: String,
    #[serde(default)]
    applies_to: Vec<String>,
    #[serde(default)]
    applies_to_languages: Vec<String>,
    #[serde(default)]
    applies_to_domains: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_is_non_empty_and_unique() {
        let stds = baseline_standards();
        assert!(stds.len() >= 3);
        let mut names: Vec<_> = stds
            .iter()
            .map(|s| format!("{}/{}", s.category.as_str(), s.name.as_str()))
            .collect();
        names.sort();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before, "standard names must be unique");
    }

    #[test]
    fn render_has_frontmatter() {
        let std = &baseline_standards()[0];
        let bytes = render_standard(std);
        let s = String::from_utf8(bytes).expect("utf8");
        assert!(s.starts_with("---\n"));
        assert!(s.contains("kind: standard"));
        assert!(s.ends_with('\n'));
    }
}

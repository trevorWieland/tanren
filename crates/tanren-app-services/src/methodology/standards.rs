//! Baseline standards bundled with the tanren install.
//!
//! `tanren install` bootstraps a new repo's `tanren/standards/<category>/<name>.md`
//! tree from this baseline, under the `preserve_existing` merge policy
//! — so adopters can extend or replace them without the installer
//! stomping their edits on reinstall.
//!
//! The set here is intentionally small (high-signal defaults covering
//! the Rust rewrite's core disciplines). Follow-up lanes will extend
//! and/or externalize this catalog.

use std::path::{Path, PathBuf};

use tanren_domain::methodology::standard::{Standard, StandardImportance};
use tanren_domain::validated::NonEmptyString;

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
        .map(
            |(name, cat, _short, _why, applies, langs, domains, imp, body)| Standard {
                name: NonEmptyString::try_new(*name).expect("static name"),
                category: NonEmptyString::try_new(*cat).expect("static category"),
                applies_to: applies.iter().map(|s| (*s).to_owned()).collect(),
                applies_to_languages: langs.iter().map(|s| (*s).to_owned()).collect(),
                applies_to_domains: domains.iter().map(|s| (*s).to_owned()).collect(),
                importance: *imp,
                body: (*body).to_owned(),
            },
        )
        .collect()
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

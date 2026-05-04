//! `xtask check-profiles` — profile/architecture markdown stays valid.
//!
//! Walks `profiles/**/*.md` and `docs/architecture/**/*.md` and validates:
//!
//! 1. Every relative markdown link `[text](path/to.md[#anchor])` resolves
//!    to a file that exists (anchors themselves are not validated).
//! 2. Every reference to `just <recipe>` resolves to a recipe defined in
//!    the workspace `justfile`, or to an entry in
//!    `xtask/check-profiles-pending.toml`'s `pending_recipes`.
//! 3. Every reference to `xtask <subcommand>` resolves to a subcommand
//!    registered in `xtask/src/main.rs`, or to an entry in the same
//!    file's `pending_subcommands`.

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

#[derive(Debug, Deserialize, Default)]
struct PendingFile {
    #[serde(default)]
    pending_recipes: Vec<String>,
    #[serde(default)]
    pending_subcommands: Vec<String>,
}

pub(crate) fn run(root: &Path) -> Result<()> {
    let pending_path = root.join("xtask").join("check-profiles-pending.toml");
    let pending = load_pending(&pending_path)?;
    let recipes = parse_justfile_recipes(&root.join("justfile"))?;
    let subcommands = parse_xtask_subcommands(&root.join("xtask").join("src").join("main.rs"))?;

    let link_re =
        Regex::new(r"\[(?P<text>[^\]]*)\]\((?P<href>[^)]+)\)").context("compile link re")?;
    let just_re = Regex::new(r"`just\s+(?P<r>[a-z0-9][a-z0-9_-]*)`").context("compile just re")?;
    let xtask_re =
        Regex::new(r"`xtask\s+(?P<r>[a-z0-9][a-z0-9_-]*)`").context("compile xtask re")?;

    let mut targets: Vec<PathBuf> = Vec::new();
    for sub in ["profiles", "docs/architecture"] {
        let dir = root.join(sub);
        if !dir.exists() {
            continue;
        }
        for entry in WalkDir::new(&dir).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            if entry.path().extension().is_some_and(|e| e == "md") {
                targets.push(entry.path().to_path_buf());
            }
        }
    }

    let mut violations: Vec<String> = Vec::new();
    for path in &targets {
        let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        check_links(root, path, &text, &link_re, &mut violations);
        check_recipes(
            root,
            path,
            &text,
            &just_re,
            &recipes,
            &pending.pending_recipes_set(),
            &mut violations,
        );
        check_subcommands(
            root,
            path,
            &text,
            &xtask_re,
            &subcommands,
            &pending.pending_subcommands_set(),
            &mut violations,
        );
    }

    if violations.is_empty() {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        let _ = writeln!(
            handle,
            "check-profiles: 0 violations ({} markdown file(s) validated)",
            targets.len()
        );
        return Ok(());
    }
    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    for v in &violations {
        let _ = writeln!(handle, "{v}");
    }
    bail!(
        "check-profiles: {} violation(s); fix the link/recipe/subcommand reference",
        violations.len()
    );
}

impl PendingFile {
    fn pending_recipes_set(&self) -> BTreeSet<String> {
        self.pending_recipes.iter().cloned().collect()
    }
    fn pending_subcommands_set(&self) -> BTreeSet<String> {
        self.pending_subcommands.iter().cloned().collect()
    }
}

fn load_pending(path: &Path) -> Result<PendingFile> {
    if !path.exists() {
        return Ok(PendingFile::default());
    }
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let parsed: PendingFile =
        toml::from_str(&raw).with_context(|| format!("parse {} as TOML", path.display()))?;
    Ok(parsed)
}

fn parse_justfile_recipes(path: &Path) -> Result<BTreeSet<String>> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    if !path.exists() {
        return Ok(out);
    }
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let re = Regex::new(r"^(?P<name>[a-z0-9][a-z0-9_-]*)\s*[: ]").context("compile justfile re")?;
    for line in raw.lines() {
        if let Some(c) = re.captures(line) {
            if let Some(m) = c.name("name") {
                out.insert(m.as_str().to_string());
            }
        }
    }
    Ok(out)
}

fn parse_xtask_subcommands(path: &Path) -> Result<BTreeSet<String>> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    if !path.exists() {
        return Ok(out);
    }
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let re = Regex::new(r#"#\[command\(name\s*=\s*"(?P<n>[a-z0-9][a-z0-9_-]*)""#)
        .context("compile xtask re")?;
    for c in re.captures_iter(&raw) {
        if let Some(m) = c.name("n") {
            out.insert(m.as_str().to_string());
        }
    }
    Ok(out)
}

fn check_links(root: &Path, path: &Path, text: &str, re: &Regex, violations: &mut Vec<String>) {
    let parent = path.parent().unwrap_or(root);
    for c in re.captures_iter(text) {
        let href = match c.name("href") {
            Some(m) => m.as_str().trim(),
            None => continue,
        };
        // Skip absolute URLs and pure anchors.
        if href.starts_with("http://")
            || href.starts_with("https://")
            || href.starts_with("mailto:")
            || href.starts_with('#')
        {
            continue;
        }
        // Strip in-page anchor fragments.
        let path_part = href.split('#').next().unwrap_or(href);
        if path_part.is_empty() {
            continue;
        }
        let target = if path_part.starts_with('/') {
            root.join(path_part.trim_start_matches('/'))
        } else {
            parent.join(path_part)
        };
        if !target.exists() {
            let line = locate_substring_line(text, href);
            violations.push(format!(
                "{}:{}: broken link `{href}` (resolved to {})",
                path.strip_prefix(root).unwrap_or(path).display(),
                line,
                target.display()
            ));
        }
    }
}

fn check_recipes(
    root: &Path,
    path: &Path,
    text: &str,
    re: &Regex,
    recipes: &BTreeSet<String>,
    pending: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    for c in re.captures_iter(text) {
        let Some(m) = c.name("r") else { continue };
        let name = m.as_str();
        if recipes.contains(name) || pending.contains(name) {
            continue;
        }
        let line = locate_substring_line(text, &format!("just {name}"));
        violations.push(format!(
            "{}:{}: reference to `just {name}` but no such recipe exists in justfile (and not in xtask/check-profiles-pending.toml)",
            path.strip_prefix(root).unwrap_or(path).display(),
            line
        ));
    }
}

fn check_subcommands(
    root: &Path,
    path: &Path,
    text: &str,
    re: &Regex,
    subcommands: &BTreeSet<String>,
    pending: &BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    for c in re.captures_iter(text) {
        let Some(m) = c.name("r") else { continue };
        let name = m.as_str();
        if subcommands.contains(name) || pending.contains(name) {
            continue;
        }
        let line = locate_substring_line(text, &format!("xtask {name}"));
        violations.push(format!(
            "{}:{}: reference to `xtask {name}` but no such subcommand registered in xtask/src/main.rs (and not in xtask/check-profiles-pending.toml)",
            path.strip_prefix(root).unwrap_or(path).display(),
            line
        ));
    }
}

fn locate_substring_line(src: &str, needle: &str) -> usize {
    if let Some(idx) = src.find(needle) {
        return src[..idx].lines().count().max(1);
    }
    1
}

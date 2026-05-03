//! `xtask check-orphan-traits` — every workspace `pub trait` must have
//! at least one `impl` somewhere in the workspace.
//!
//! AST-walks `crates/**/src/**/*.rs`. First pass: collect every `pub trait`
//! definition (name → path:line). Second pass: collect every `impl <Trait>
//! for <Type>` declaration in the workspace and record which trait names
//! are implemented (matched by the trailing path segment, e.g.
//! `tanren_identity_policy::CredentialVerifier` matches `CredentialVerifier`).
//! Any trait with zero impls is reported.

use anyhow::{Context, Result, bail};
use quote::ToTokens;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use syn::{Item, Visibility};

pub(crate) fn run(root: &Path) -> Result<()> {
    let crates_dir = root.join("crates");
    if !crates_dir.exists() {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        let _ = writeln!(
            handle,
            "check-orphan-traits: 0 violations (no crates/ tree present)"
        );
        return Ok(());
    }

    let mut traits: BTreeMap<String, (PathBuf, usize)> = BTreeMap::new();
    let mut impls: BTreeSet<String> = BTreeSet::new();

    for entry in walkdir::WalkDir::new(&crates_dir)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        if !path.components().any(|c| c.as_os_str() == "src") {
            continue;
        }
        scan_file(path, &mut traits, &mut impls)?;
    }

    let mut violations: Vec<String> = Vec::new();
    for (name, (path, line)) in &traits {
        if !impls.contains(name) {
            violations.push(format!(
                "{}:{}: trait `{name}` has no impl in the workspace",
                path.strip_prefix(root).unwrap_or(path).display(),
                line
            ));
        }
    }

    if violations.is_empty() {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        let _ = writeln!(
            handle,
            "check-orphan-traits: 0 violations ({} pub trait(s) implemented)",
            traits.len()
        );
        return Ok(());
    }
    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    for v in &violations {
        let _ = writeln!(handle, "{v}");
    }
    bail!(
        "check-orphan-traits: {} trait(s) lack an implementor",
        violations.len()
    );
}

fn scan_file(
    path: &Path,
    traits: &mut BTreeMap<String, (PathBuf, usize)>,
    impls: &mut BTreeSet<String>,
) -> Result<()> {
    let src = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let Ok(file) = syn::parse_file(&src) else {
        return Ok(());
    };
    walk_items(path, &src, &file.items, traits, impls);
    Ok(())
}

fn walk_items(
    path: &Path,
    src: &str,
    items: &[Item],
    traits: &mut BTreeMap<String, (PathBuf, usize)>,
    impls: &mut BTreeSet<String>,
) {
    for item in items {
        match item {
            Item::Trait(t) => {
                if matches!(t.vis, Visibility::Public(_)) {
                    let name = t.ident.to_string();
                    let line = locate_marker(src, &format!("trait {name}"));
                    traits
                        .entry(name)
                        .or_insert_with(|| (path.to_path_buf(), line));
                }
            }
            Item::Impl(i) => {
                if let Some((_, p, _)) = &i.trait_ {
                    let path_text = collapse_ws(&p.to_token_stream().to_string());
                    if let Some(seg) = path_text.rsplit("::").next() {
                        // `Trait < Generic >` — strip generics.
                        let base = seg.split('<').next().unwrap_or(seg).trim().to_string();
                        if !base.is_empty() {
                            impls.insert(base);
                        }
                    }
                }
            }
            Item::Mod(m) => {
                if let Some((_, items)) = &m.content {
                    walk_items(path, src, items, traits, impls);
                }
            }
            _ => {}
        }
    }
}

fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

fn locate_marker(src: &str, marker: &str) -> usize {
    if let Some(idx) = src.find(marker) {
        return src[..idx].lines().count().max(1);
    }
    1
}

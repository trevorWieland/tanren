//! `xtask check-test-hooks` — pub APIs that look like test scaffolding
//! must be `#[cfg]`-gated.
//!
//! AST-walks `crates/**/src/**/*.rs`. For every `pub fn` (sync or async)
//! whose accumulated doc-comments mention `test`, `fixture`, or `seed`
//! (case-insensitive), the item must also carry one of:
//!
//! - `#[cfg(test)]`
//! - `#[cfg(feature = "test-hooks")]`
//! - `#[cfg(any(test, feature = "test-hooks"))]`
//!
//! See `docs/architecture/subsystems/state.md`.

use anyhow::{Context, Result, bail};
use quote::ToTokens;
use regex::Regex;
use std::fs;
use std::io::Write;
use std::path::Path;

use syn::{Attribute, Expr, ImplItem, Item, ItemFn, Lit, Meta, Visibility};

pub(crate) fn run(root: &Path) -> Result<()> {
    let crates_dir = root.join("crates");
    if !crates_dir.exists() {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        let _ = writeln!(
            handle,
            "check-test-hooks: 0 violations (no crates/ tree present)"
        );
        return Ok(());
    }
    let doc_re = Regex::new(r"(?i)test|fixture|seed").context("compile doc regex")?;

    let mut violations: Vec<String> = Vec::new();
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
        // Only scan src/** trees.
        if !path.components().any(|c| c.as_os_str() == "src") {
            continue;
        }
        scan_file(root, path, &doc_re, &mut violations)?;
    }

    if violations.is_empty() {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        let _ = writeln!(
            handle,
            "check-test-hooks: 0 violations (test-flavored pub fns are cfg-gated)"
        );
        return Ok(());
    }
    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    for v in &violations {
        let _ = writeln!(handle, "{v}");
    }
    bail!(
        "check-test-hooks: {} violation(s); gate test-flavored pub fns behind #[cfg(test)] or #[cfg(feature = \"test-hooks\")]",
        violations.len()
    );
}

fn scan_file(root: &Path, path: &Path, doc_re: &Regex, violations: &mut Vec<String>) -> Result<()> {
    let src = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let Ok(file) = syn::parse_file(&src) else {
        return Ok(());
    };
    // A `#![cfg(feature = "test-hooks")]` (or equivalent) inner attribute
    // at the top of the file gates every public item it contains.
    if is_cfg_gated(&file.attrs) {
        return Ok(());
    }
    scan_items(root, path, &src, &file.items, doc_re, violations);
    Ok(())
}

fn scan_items(
    root: &Path,
    path: &Path,
    src: &str,
    items: &[Item],
    doc_re: &Regex,
    violations: &mut Vec<String>,
) {
    for item in items {
        match item {
            Item::Fn(f) => check_fn(root, path, src, f, doc_re, violations),
            Item::Mod(m) => {
                if let Some((_, items)) = &m.content {
                    scan_items(root, path, src, items, doc_re, violations);
                }
            }
            Item::Impl(i) => {
                // The cfg-gate may live on the enclosing `impl` block —
                // that is the canonical pattern for a `#[cfg(feature =
                // "test-hooks")] impl Foo { ... }` block of fixture
                // seeders. Treat any cfg on the impl as covering every
                // method inside it.
                let impl_gated = is_cfg_gated(&i.attrs);
                for ii in &i.items {
                    if let ImplItem::Fn(m) = ii {
                        let is_pub = matches!(m.vis, Visibility::Public(_));
                        if !is_pub {
                            continue;
                        }
                        let attrs = &m.attrs;
                        let doc = collect_doc(attrs);
                        if !doc_re.is_match(&doc) {
                            continue;
                        }
                        if impl_gated || is_cfg_gated(attrs) {
                            continue;
                        }
                        let line = locate_fn_line(src, &m.sig.ident.to_string());
                        violations.push(format_violation(
                            root,
                            path,
                            line,
                            &m.sig.ident.to_string(),
                        ));
                    }
                }
            }
            _ => {}
        }
    }
}

fn check_fn(
    root: &Path,
    path: &Path,
    src: &str,
    f: &ItemFn,
    doc_re: &Regex,
    violations: &mut Vec<String>,
) {
    if !matches!(f.vis, Visibility::Public(_)) {
        return;
    }
    let doc = collect_doc(&f.attrs);
    if !doc_re.is_match(&doc) {
        return;
    }
    if is_cfg_gated(&f.attrs) {
        return;
    }
    let name = f.sig.ident.to_string();
    let line = locate_fn_line(src, &name);
    violations.push(format_violation(root, path, line, &name));
}

fn format_violation(root: &Path, path: &Path, line: usize, name: &str) -> String {
    format!(
        "{}:{}: pub fn `{name}` looks like test scaffolding but is not gated by #[cfg(test)] or #[cfg(feature = \"test-hooks\")]",
        path.strip_prefix(root).unwrap_or(path).display(),
        line
    )
}

fn collect_doc(attrs: &[Attribute]) -> String {
    let mut out = String::new();
    for a in attrs {
        if !a.path().is_ident("doc") {
            continue;
        }
        if let Meta::NameValue(nv) = &a.meta {
            if let Expr::Lit(lit) = &nv.value {
                if let Lit::Str(s) = &lit.lit {
                    out.push_str(&s.value());
                    out.push('\n');
                }
            }
        }
    }
    out
}

fn is_cfg_gated(attrs: &[Attribute]) -> bool {
    for a in attrs {
        if !a.path().is_ident("cfg") {
            continue;
        }
        let text = a.to_token_stream().to_string();
        if text.contains("test") || text.contains("test-hooks") || text.contains("test_hooks") {
            return true;
        }
    }
    false
}

fn locate_fn_line(src: &str, fn_name: &str) -> usize {
    let needles = [
        format!("pub fn {fn_name}"),
        format!("pub async fn {fn_name}"),
        format!("fn {fn_name}"),
    ];
    for n in &needles {
        if let Some(idx) = src.find(n.as_str()) {
            return src[..idx].lines().count().max(1);
        }
    }
    1
}

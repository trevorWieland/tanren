//! `xtask check-newtype-ids` — bare `uuid::Uuid` field types are forbidden
//! outside the allowlisted newtype declaration sites.
//!
//! AST-walks
//! `crates/tanren-{contract,store,identity-policy,app-services}/src/**/*.rs`.
//! For every struct/enum field, the field's type rendered text is checked;
//! if it equals one of:
//!
//! - `Uuid`
//! - `uuid::Uuid`
//! - `Option<Uuid>`
//! - `Option<uuid::Uuid>`
//! - `Vec<Uuid>`
//! - `Vec<uuid::Uuid>`
//!
//! and the file is not listed in `xtask/uuid-allowlist.toml`, the field is
//! rejected.
//!
//! See `profiles/rust-cargo/architecture/id-formats.md`.

use anyhow::{Context, Result, bail};
use quote::ToTokens;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::Path;

use syn::{Fields, Item};

const TARGET_CRATES: &[&str] = &[
    "tanren-contract",
    "tanren-store",
    "tanren-identity-policy",
    "tanren-app-services",
];

const FORBIDDEN_TYPES: &[&str] = &[
    "Uuid",
    "uuid :: Uuid",
    "Option < Uuid >",
    "Option < uuid :: Uuid >",
    "Vec < Uuid >",
    "Vec < uuid :: Uuid >",
];

#[derive(Debug, Deserialize)]
struct AllowlistFile {
    #[serde(default)]
    allowed_files: Vec<String>,
}

pub(crate) fn run(root: &Path) -> Result<()> {
    let allowlist = load_allowlist(&root.join("xtask").join("uuid-allowlist.toml"))?;

    let mut violations: Vec<String> = Vec::new();
    for c in TARGET_CRATES {
        let crate_src = root.join("crates").join(c).join("src");
        if !crate_src.exists() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&crate_src)
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
            let rel = path.strip_prefix(root).unwrap_or(path);
            let rel_str = rel.display().to_string().replace('\\', "/");
            if allowlist.contains(&rel_str) {
                continue;
            }
            scan_file(root, path, &mut violations)?;
        }
    }

    if violations.is_empty() {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        let _ = writeln!(
            handle,
            "check-newtype-ids: 0 violations (no bare uuid::Uuid field types)"
        );
        return Ok(());
    }
    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    for v in &violations {
        let _ = writeln!(handle, "{v}");
    }
    bail!(
        "check-newtype-ids: {} violation(s); use a workspace newtype or add the file to xtask/uuid-allowlist.toml",
        violations.len()
    );
}

fn load_allowlist(path: &Path) -> Result<BTreeSet<String>> {
    if !path.exists() {
        return Ok(BTreeSet::new());
    }
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let parsed: AllowlistFile =
        toml::from_str(&raw).with_context(|| format!("parse {} as TOML", path.display()))?;
    Ok(parsed.allowed_files.into_iter().collect())
}

fn scan_file(root: &Path, path: &Path, violations: &mut Vec<String>) -> Result<()> {
    let src = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let Ok(file) = syn::parse_file(&src) else {
        return Ok(());
    };
    for item in &file.items {
        match item {
            Item::Struct(s) => check_fields(root, path, &s.fields, &src, violations),
            Item::Enum(e) => {
                for v in &e.variants {
                    check_fields(root, path, &v.fields, &src, violations);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn check_fields(
    root: &Path,
    path: &Path,
    fields: &Fields,
    src: &str,
    violations: &mut Vec<String>,
) {
    let iter: Box<dyn Iterator<Item = &syn::Field>> = match fields {
        Fields::Named(n) => Box::new(n.named.iter()),
        Fields::Unnamed(u) => Box::new(u.unnamed.iter()),
        Fields::Unit => return,
    };
    for f in iter {
        let ty = collapse_ws(&f.ty.to_token_stream().to_string());
        if FORBIDDEN_TYPES.iter().any(|t| t == &ty) {
            let label = f
                .ident
                .as_ref()
                .map_or_else(|| "<tuple field>".to_string(), ToString::to_string);
            let line = locate_line(src, &label);
            violations.push(format!(
                "{}:{}: field `{label}: {ty}` uses bare uuid; introduce a workspace newtype",
                path.strip_prefix(root).unwrap_or(path).display(),
                line
            ));
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

fn locate_line(src: &str, name: &str) -> usize {
    let probe = format!("{name}:");
    if let Some(idx) = src.find(&probe) {
        return src[..idx].lines().count().max(1);
    }
    1
}

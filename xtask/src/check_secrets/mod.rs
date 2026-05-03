//! `xtask check-secrets` — secret-typed fields must wrap a `secrecy` /
//! workspace newtype.
//!
//! AST-walks `crates/tanren-{contract,store,app-services,identity-policy}`.
//! For every struct field whose name matches one of the workspace's
//! secret-shaped names (`password`, `secret`, `api_key`, `credential`,
//! `session_token`, `bearer`, `private_key`, `csrf`, `auth_token`), the
//! field's type must render to a string containing one of:
//!
//! - `secrecy::SecretString` / `SecretString`
//! - `secrecy::SecretBox` / `SecretBox`
//! - any allowlist entry from `xtask/secret-newtypes.toml`.
//!
//! In addition, fields whose names end in `_token`, `_key`, or `_secret`
//! are rejected if their type is bare `String` or `Vec<u8>` and the type
//! is not on the allowlist. This keeps "look-alikes" honest.
//!
//! See `profiles/rust-cargo/architecture/secrets-handling.md`.

use anyhow::{Context, Result, bail};
use quote::ToTokens;
use regex::Regex;
use serde::Deserialize;
use std::fs;
use std::io::Write;
use std::path::Path;

use syn::{Fields, Item};

struct ScanCtx<'a> {
    root: &'a Path,
    secret_name_re: &'a Regex,
    suffix_name_re: &'a Regex,
    allowlist: &'a [String],
}

const TARGET_CRATES: &[&str] = &[
    "tanren-contract",
    "tanren-store",
    "tanren-app-services",
    "tanren-identity-policy",
];

#[derive(Debug, Deserialize)]
struct AllowlistFile {
    #[serde(default)]
    allowed: Vec<String>,
}

pub(crate) fn run(root: &Path) -> Result<()> {
    let allowlist = load_allowlist(&root.join("xtask").join("secret-newtypes.toml"))?;
    let secret_name_re = Regex::new(
        r"(?i)password|secret|api_key|credential|session_token|bearer|private_key|csrf|auth_token",
    )
    .context("compile secret-name regex")?;
    let suffix_name_re = Regex::new(r"(?i)(_token|_key|_secret)$").context("compile suffix re")?;

    let ctx = ScanCtx {
        root,
        secret_name_re: &secret_name_re,
        suffix_name_re: &suffix_name_re,
        allowlist: &allowlist,
    };
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
            scan_file(&ctx, path, &mut violations)?;
        }
    }

    report(&violations)
}

fn load_allowlist(path: &Path) -> Result<Vec<String>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let parsed: AllowlistFile =
        toml::from_str(&raw).with_context(|| format!("parse {} as TOML", path.display()))?;
    Ok(parsed.allowed)
}

fn scan_file(ctx: &ScanCtx<'_>, path: &Path, violations: &mut Vec<String>) -> Result<()> {
    let src = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let Ok(file) = syn::parse_file(&src) else {
        return Ok(());
    };
    for item in &file.items {
        match item {
            Item::Struct(s) => check_fields(ctx, path, &s.fields, &src, violations),
            Item::Enum(e) => {
                for v in &e.variants {
                    check_fields(ctx, path, &v.fields, &src, violations);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn check_fields(
    ctx: &ScanCtx<'_>,
    path: &Path,
    fields: &Fields,
    src: &str,
    violations: &mut Vec<String>,
) {
    let named = match fields {
        Fields::Named(n) => &n.named,
        _ => return,
    };
    for f in named {
        let Some(ident) = f.ident.as_ref() else {
            continue;
        };
        let name = ident.to_string();
        let ty_string = normalize(&f.ty.to_token_stream().to_string());
        let line = locate_line(src, &name);
        let rel = path
            .strip_prefix(ctx.root)
            .unwrap_or(path)
            .display()
            .to_string();

        let name_hits_secret = ctx.secret_name_re.is_match(&name);
        let name_hits_suffix = ctx.suffix_name_re.is_match(&name);
        if !name_hits_secret && !name_hits_suffix {
            continue;
        }

        let has_secrecy = ty_string.contains("secrecy :: SecretString")
            || ty_string.contains("secrecy :: SecretBox")
            || ty_string.contains("SecretString")
            || ty_string.contains("SecretBox");
        let has_allowlist = ctx
            .allowlist
            .iter()
            .any(|t| ty_string.contains(&normalize(t)));

        if name_hits_secret && !has_secrecy && !has_allowlist {
            violations.push(format!(
                "{rel}:{line}: field `{name}: {ty_string}` looks like a secret but is not wrapped in `secrecy::SecretString`/`SecretBox` or a workspace newtype (xtask/secret-newtypes.toml)"
            ));
            continue;
        }

        if name_hits_suffix && !has_secrecy && !has_allowlist {
            let bare_string = ty_string == "String" || ty_string == "std :: string :: String";
            let bare_bytes = ty_string == "Vec < u8 >" || ty_string.ends_with(":: Vec < u8 >");
            if bare_string || bare_bytes {
                violations.push(format!(
                    "{rel}:{line}: field `{name}: {ty_string}` ends in `_token`/`_key`/`_secret` but uses bare `{ty_string}`; wrap in a workspace newtype or `secrecy::SecretString`"
                ));
            }
        }
    }
}

fn report(violations: &[String]) -> Result<()> {
    if violations.is_empty() {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        let _ = writeln!(
            handle,
            "check-secrets: 0 violations (secret-shaped fields properly wrapped)"
        );
        return Ok(());
    }
    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    for v in violations {
        let _ = writeln!(handle, "{v}");
    }
    bail!(
        "check-secrets: {} violation(s); see profiles/rust-cargo/architecture/secrets-handling.md",
        violations.len()
    );
}

fn normalize(s: &str) -> String {
    // `quote::ToTokens` always inserts spaces around tokens. Collapse
    // runs of whitespace to single spaces so equality/contains checks
    // are stable across rustc versions.
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

fn locate_line(src: &str, needle: &str) -> usize {
    let probe = format!("{needle}:");
    if let Some(idx) = src.find(&probe) {
        return src[..idx].lines().count().max(1);
    }
    if let Some(idx) = src.find(needle) {
        return src[..idx].lines().count().max(1);
    }
    1
}

//! `xtask check-openapi-handcraft` — hand-rolled `OpenAPI` documents are
//! forbidden in api crates.
//!
//! The api stack generates its `OpenAPI` document from `utoipa` derives at
//! compile time so the schema, the types, and the running handlers stay
//! in lockstep. Hand-rolled `serde_json::json!({"openapi": ..., "paths":
//! ..., "components": ...})` literals would silently drift from the
//! types they purport to describe within a release or two and become a
//! second source of truth nobody trusts.
//!
//! This guard scans every `.rs` file under
//! `bin/tanren-api/src/**` and `crates/tanren-{api,*-api,*-api-app}/src/**`
//! (matched lexically so a future api crate falls under the rule
//! automatically) and rejects any `json!` / `serde_json::json!` macro
//! invocation whose body mentions one of the well-known top-level
//! `OpenAPI` keys (`"openapi"`, `"paths"`, `"components"`). The substring
//! match is intentionally lenient — formatting is not load-bearing;
//! presence of the keys anywhere inside the macro body is sufficient
//! evidence that someone is hand-rolling a document.
//!
//! See `profiles/rust-cargo/architecture/openapi-generation.md`.

use anyhow::{Context, Result, bail};
use regex::Regex;
use std::fs;
use std::io::Write;
use std::path::Path;

const FORBIDDEN_KEYS: &[&str] = &["\"openapi\"", "\"paths\"", "\"components\""];

pub(crate) fn run(root: &Path) -> Result<()> {
    // `(?s)` so `.` matches newlines; `?` so the body capture stops at
    // the first `}` to keep the scan from swallowing the rest of the
    // file. The check then re-scans the captured slice for forbidden
    // keys; a second `json!` block in the same file is matched on the
    // next iteration of the captures.
    let macro_re = Regex::new(r"(?s)\bjson!\s*\(\s*\{(?P<body>.*?)\}\s*\)")
        .context("compile json!() regex")?;
    let macro_re_brace =
        Regex::new(r"(?s)\bjson!\s*\{(?P<body>.*?)\}").context("compile json!{} regex")?;

    let mut violations: Vec<String> = Vec::new();
    let bin_api = root.join("bin").join("tanren-api").join("src");
    if bin_api.exists() {
        scan_dir(root, &bin_api, &macro_re, &macro_re_brace, &mut violations)?;
    }
    let crates_dir = root.join("crates");
    if crates_dir.exists() {
        for entry in fs::read_dir(&crates_dir)
            .with_context(|| format!("read_dir {}", crates_dir.display()))?
        {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            // Match every `tanren-*-app` whose name carries `api`, plus
            // any `tanren-*-api*` crate, so the guard keeps biting as
            // the api surface grows.
            let is_target =
                name == "tanren-api-app" || (name.starts_with("tanren-") && name.contains("-api"));
            if !is_target {
                continue;
            }
            let crate_src = entry.path().join("src");
            if crate_src.exists() {
                scan_dir(
                    root,
                    &crate_src,
                    &macro_re,
                    &macro_re_brace,
                    &mut violations,
                )?;
            }
        }
    }

    if violations.is_empty() {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        let _ = writeln!(
            handle,
            "check-openapi-handcraft: 0 violations (api crates use utoipa-generated documents)"
        );
        return Ok(());
    }
    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    for v in &violations {
        let _ = writeln!(handle, "{v}");
    }
    bail!(
        "check-openapi-handcraft: {} violation(s); use utoipa derives, see profiles/rust-cargo/architecture/openapi-generation.md",
        violations.len()
    );
}

fn scan_dir(
    root: &Path,
    dir: &Path,
    paren_re: &Regex,
    brace_re: &Regex,
    violations: &mut Vec<String>,
) -> Result<()> {
    for entry in walkdir::WalkDir::new(dir)
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
        scan_file(root, path, paren_re, brace_re, violations)?;
    }
    Ok(())
}

fn scan_file(
    root: &Path,
    path: &Path,
    paren_re: &Regex,
    brace_re: &Regex,
    violations: &mut Vec<String>,
) -> Result<()> {
    let src = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    for re in [paren_re, brace_re] {
        for cap in re.captures_iter(&src) {
            let Some(m) = cap.get(0) else { continue };
            let body = match cap.name("body") {
                Some(b) => b.as_str(),
                None => continue,
            };
            if FORBIDDEN_KEYS.iter().any(|k| body.contains(k)) {
                let line = src[..m.start()].lines().count().max(1);
                violations.push(format!(
                    "{}:{}: hand-rolled `OpenAPI` document — `json!{{...}}` body contains an `OpenAPI` top-level key; generate via utoipa derives instead",
                    path.strip_prefix(root).unwrap_or(path).display(),
                    line
                ));
            }
        }
    }
    Ok(())
}

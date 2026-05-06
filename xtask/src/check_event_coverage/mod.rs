//! `xtask check-event-coverage` — every event variant must have a BDD step
//! asserting it fires.
//!
//! Walks `crates/tanren-app-services/src/events.rs` (and any sibling files
//! that define an enum whose name ends in `Event` or `EventKind`) and
//! collects every variant. Each variant must appear in at least one
//! `tests/bdd/features/**/*.feature` step body, in the form
//! `'<snake_case_variant_name>' event` (the canonical pattern documented
//! in `profiles/rust-cargo/global/just-ci-gate.md`).
//!
//! This check is intentionally tolerant: until events are defined, it
//! collects zero variants and exits 0. Once variants exist, it errors
//! per missing assertion.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use syn::{Item, ItemEnum};
use walkdir::WalkDir;

#[derive(Debug, Deserialize)]
struct PendingVariant {
    name: String,
    upcoming_spec: String,
    reason: String,
}

#[derive(Debug, Deserialize, Default)]
struct PendingFile {
    #[serde(default)]
    pending: Vec<PendingVariant>,
}

pub(crate) fn run(root: &Path) -> Result<()> {
    let app_services_src = root.join("crates").join("tanren-app-services").join("src");
    let mut variants: BTreeMap<String, (PathBuf, usize)> = BTreeMap::new();
    if app_services_src.exists() {
        for entry in WalkDir::new(&app_services_src)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "rs") {
                collect_event_variants(path, &mut variants)?;
            }
        }
    }

    if variants.is_empty() {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        let _ = writeln!(
            handle,
            "check-event-coverage: 0 violations (no event enums defined yet)"
        );
        return Ok(());
    }

    let pending = load_pending(&root.join("xtask").join("event-coverage-pending.toml"))?;

    let features_root = root.join("tests").join("bdd").join("features");
    let feature_text = collect_feature_text(&features_root)?;

    let mut violations: Vec<String> = Vec::new();
    let mut stale_pending: Vec<String> = Vec::new();
    for (variant, (path, line)) in &variants {
        let needle_a = format!("'{}' event", to_snake_case(variant));
        let needle_b = format!("\"{}\" event", to_snake_case(variant));
        let needle_c = format!("`{}` event", to_snake_case(variant));
        let covered = feature_text.contains(&needle_a)
            || feature_text.contains(&needle_b)
            || feature_text.contains(&needle_c);
        if covered {
            if pending.contains_key(variant) {
                stale_pending.push(format!(
                    "variant `{variant}` now has BDD coverage; remove its entry from xtask/event-coverage-pending.toml"
                ));
            }
            continue;
        }
        if pending.contains_key(variant) {
            continue;
        }
        violations.push(format!(
            "{}:{}: event variant `{}` has no BDD step asserting it fires",
            path.strip_prefix(root).unwrap_or(path).display(),
            line,
            variant
        ));
    }

    for name in pending.keys() {
        if !variants.contains_key(name) {
            stale_pending.push(format!(
                "pending entry `{name}` does not match any event variant in the workspace"
            ));
        }
    }

    if violations.is_empty() && stale_pending.is_empty() {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        let _ = writeln!(
            handle,
            "check-event-coverage: 0 violations ({} variant(s) covered, {} pending)",
            variants.len() - pending.len(),
            pending.len()
        );
        return Ok(());
    }

    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    for v in &violations {
        let _ = writeln!(handle, "{v}");
    }
    for v in &stale_pending {
        let _ = writeln!(handle, "{v}");
    }
    bail!(
        "check-event-coverage: {} violation(s), {} stale pending entry(ies)",
        violations.len(),
        stale_pending.len()
    );
}

fn load_pending(path: &Path) -> Result<HashMap<String, PendingVariant>> {
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let parsed: PendingFile =
        toml::from_str(&raw).with_context(|| format!("parse {} as TOML", path.display()))?;
    let mut out = HashMap::with_capacity(parsed.pending.len());
    for entry in parsed.pending {
        if entry.reason.trim().is_empty() {
            bail!(
                "event-coverage pending entry `{}` has empty `reason`",
                entry.name
            );
        }
        if entry.upcoming_spec.trim().is_empty() {
            bail!(
                "event-coverage pending entry `{}` has empty `upcoming_spec`",
                entry.name
            );
        }
        if let Some(prev) = out.insert(entry.name.clone(), entry) {
            bail!(
                "event-coverage pending file has duplicate entry for `{}`",
                prev.name
            );
        }
    }
    Ok(out)
}

fn collect_event_variants(
    path: &Path,
    variants: &mut BTreeMap<String, (PathBuf, usize)>,
) -> Result<()> {
    let src = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let Ok(file) = syn::parse_file(&src) else {
        return Ok(());
    };
    for item in &file.items {
        if let Item::Enum(ItemEnum {
            ident,
            variants: vs,
            ..
        }) = item
        {
            let name = ident.to_string();
            if !(name.ends_with("Event") || name.ends_with("EventKind")) {
                continue;
            }
            for v in vs {
                let line = lineno_for_byte_offset(&src, byte_offset_of(&src, &v.ident.to_string()));
                variants.insert(v.ident.to_string(), (path.to_path_buf(), line));
            }
        }
    }
    Ok(())
}

fn collect_feature_text(features_root: &Path) -> Result<String> {
    let mut buf = String::new();
    if !features_root.exists() {
        return Ok(buf);
    }
    for entry in WalkDir::new(features_root)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().extension().is_some_and(|e| e == "feature") {
            let s = fs::read_to_string(entry.path())
                .with_context(|| format!("read {}", entry.path().display()))?;
            buf.push('\n');
            buf.push_str(&s);
        }
    }
    Ok(buf)
}

fn to_snake_case(camel: &str) -> String {
    let mut out = String::with_capacity(camel.len() + 4);
    for (i, ch) in camel.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if i != 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

// First-occurrence byte offset for a needle. Approximate but adequate
// for surfacing line numbers in error messages.
fn byte_offset_of(haystack: &str, needle: &str) -> usize {
    haystack.find(needle).unwrap_or(0)
}

fn lineno_for_byte_offset(src: &str, offset: usize) -> usize {
    src[..offset.min(src.len())].lines().count().max(1)
}

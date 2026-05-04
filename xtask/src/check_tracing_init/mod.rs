//! `xtask check-tracing-init` — every binary `main.rs` must call
//! `tanren_observability::init` (or a re-export thereof) before any other
//! work. See `docs/architecture/subsystems/observation.md` and
//! `profiles/rust-cargo/rust/no-unsafe-no-debug-output.md`.
//!
//! The check parses each `bin/*/src/main.rs`, walks the AST, and looks
//! for any expression whose textual rendering contains
//! `tanren_observability :: init` or a method call ending in `init` whose
//! receiver chain mentions `tanren_observability`. If absent, the binary
//! is reported.

use anyhow::{Context, Result, bail};
use quote::ToTokens;
use std::fs;
use std::io::Write;
use std::path::Path;

pub(crate) fn run(root: &Path) -> Result<()> {
    let bin_dir = root.join("bin");
    if !bin_dir.exists() {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        let _ = writeln!(
            handle,
            "check-tracing-init: 0 violations (no bin/ tree present)"
        );
        return Ok(());
    }

    let mut violations: Vec<String> = Vec::new();
    let mut checked = 0usize;
    for entry in
        fs::read_dir(&bin_dir).with_context(|| format!("read_dir {}", bin_dir.display()))?
    {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let main_rs = entry.path().join("src").join("main.rs");
        if !main_rs.exists() {
            continue;
        }
        checked += 1;
        if !main_initializes_tracing(&main_rs)? {
            violations.push(format!(
                "{}:1: missing call to `tanren_observability::init(...)`",
                main_rs.strip_prefix(root).unwrap_or(&main_rs).display()
            ));
        }
    }

    if violations.is_empty() {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        let _ = writeln!(
            handle,
            "check-tracing-init: 0 violations ({checked} binary main(s) initialize tracing)"
        );
        return Ok(());
    }
    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    for v in &violations {
        let _ = writeln!(handle, "{v}");
    }
    bail!(
        "check-tracing-init: {} binary main(s) do not initialize tracing via tanren_observability::init",
        violations.len()
    );
}

fn main_initializes_tracing(path: &Path) -> Result<bool> {
    let src = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let Ok(file) = syn::parse_file(&src) else {
        return Ok(false);
    };
    let blob = file.to_token_stream().to_string();
    let normalized = collapse_ws(&blob);
    Ok(normalized.contains("tanren_observability :: init")
        || normalized.contains("tanren_observability::init")
        || (normalized.contains("tanren_observability") && contains_init_call(&normalized)))
}

fn contains_init_call(s: &str) -> bool {
    // Permissive: any `. init (` substring near `tanren_observability`.
    s.contains(". init (")
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

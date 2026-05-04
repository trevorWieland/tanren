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
//!
//! TUI exception. `bin/tanren-tui/src/main.rs` enables raw mode and the
//! alternate screen on stdout, so a stdout-bound subscriber would corrupt
//! the rendered frame. The TUI must call
//! `tanren_observability::init_to_file(...)` instead and must NOT call the
//! plain `init(...)`. See `docs/architecture/subsystems/observation.md`
//! ("Tracing initialization contract").

use anyhow::{Context, Result, bail};
use quote::ToTokens;
use std::fs;
use std::io::Write;
use std::path::Path;

const TUI_BIN_NAME: &str = "tanren-tui";

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
        let bin_name = entry.file_name();
        let main_rs = entry.path().join("src").join("main.rs");
        if !main_rs.exists() {
            continue;
        }
        checked += 1;
        let report = inspect_main(&main_rs)?;
        let display_path = main_rs.strip_prefix(root).unwrap_or(&main_rs).display();

        if !report.has_observability_init {
            violations.push(format!(
                "{display_path}:1: missing call to `tanren_observability::init(...)` or `init_to_file(...)`"
            ));
            continue;
        }

        if bin_name == TUI_BIN_NAME {
            if !report.has_file_init {
                violations.push(format!(
                    "{display_path}:1: bin/{TUI_BIN_NAME} must call `tanren_observability::init_to_file(...)` (the TUI owns stdout under raw mode + alternate screen — a stdout-bound subscriber would corrupt the rendered frame). See docs/architecture/subsystems/observation.md."
                ));
            }
            if report.has_plain_init {
                violations.push(format!(
                    "{display_path}:1: bin/{TUI_BIN_NAME} must NOT call the plain `tanren_observability::init(...)`; use `init_to_file(...)` instead."
                ));
            }
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
        "check-tracing-init: {} violation(s) in binary main(s)",
        violations.len()
    );
}

#[derive(Default, Debug)]
struct InitReport {
    /// True if the source mentions any `tanren_observability::init…` form.
    has_observability_init: bool,
    /// True if the source calls `tanren_observability::init_to_file(`.
    has_file_init: bool,
    /// True if the source calls the plain `tanren_observability::init(` —
    /// note this excludes `init_to_file`, since `init_to_file(` does not
    /// match `init(`.
    has_plain_init: bool,
}

fn inspect_main(path: &Path) -> Result<InitReport> {
    let src = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let Ok(file) = syn::parse_file(&src) else {
        return Ok(InitReport::default());
    };
    let blob = file.to_token_stream().to_string();
    let normalized = collapse_ws(&blob);
    let has_observability_init = normalized.contains("tanren_observability :: init")
        || normalized.contains("tanren_observability::init")
        || (normalized.contains("tanren_observability") && contains_init_call(&normalized));
    let has_file_init = normalized.contains("tanren_observability :: init_to_file")
        || normalized.contains("tanren_observability::init_to_file")
        || normalized.contains(". init_to_file (");
    let has_plain_init = (normalized.contains("tanren_observability :: init (")
        || normalized.contains("tanren_observability::init ("))
        && !has_file_init_only(&normalized);
    Ok(InitReport {
        has_observability_init,
        has_file_init,
        has_plain_init,
    })
}

/// Disambiguates: `tanren_observability::init_to_file(` contains
/// `tanren_observability::init` as a substring, but the next char is `_`,
/// not `(` — so the plain-init detector above (which matches `init (`
/// after collapsing whitespace) correctly rejects it. This helper is
/// retained for clarity if the matcher grows.
fn has_file_init_only(_normalized: &str) -> bool {
    false
}

fn contains_init_call(s: &str) -> bool {
    // Permissive: any `. init (` substring near `tanren_observability`.
    s.contains(". init (") || s.contains(". init_to_file (")
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

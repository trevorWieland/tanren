//! Repo maintenance commands for Tanren.

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Parser)]
#[command(
    name = "tanren-xtask",
    version,
    about = "Tanren repo maintenance commands"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Reject any `#[test]`, `#[cfg(test)]`, or `mod tests` outside the
    /// `tanren-bdd` crate. Tests live exclusively in BDD scenarios.
    CheckRustTestSurface,
    /// Reject inline `#[allow(...)]` and `#[expect(...)]` anywhere in
    /// workspace Rust source. Lint relaxations belong in a crate's
    /// `[lints.clippy]` section, not at the source-line level.
    CheckSuppression,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::CheckRustTestSurface => check_rust_test_surface(),
        Command::CheckSuppression => check_suppression(),
    }
}

fn workspace_root() -> Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .context("xtask manifest dir has no parent")?;
    Ok(root.to_owned())
}

fn rust_source_files(root: &Path) -> impl Iterator<Item = PathBuf> + use<> {
    let crates = root.join("crates");
    let bin = root.join("bin");
    let xtask = root.join("xtask");
    [crates, bin, xtask]
        .into_iter()
        .filter(|p| p.exists())
        .flat_map(|root| {
            WalkDir::new(root)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().is_file())
                .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "rs"))
                .map(walkdir::DirEntry::into_path)
        })
}

fn check_rust_test_surface() -> Result<()> {
    let root = workspace_root()?;
    let mut violations = Vec::<String>::new();
    for path in rust_source_files(&root) {
        if path.components().any(|c| c.as_os_str() == "tanren-bdd") {
            continue;
        }
        let content =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        for (lineno, line) in content.lines().enumerate() {
            let trimmed = line.trim_start();
            let hit = trimmed.starts_with("#[test]")
                || trimmed.starts_with("#[tokio::test]")
                || trimmed.starts_with("#[cfg(test)]")
                || trimmed.starts_with("mod tests");
            if hit {
                violations.push(format!(
                    "{}:{}: forbidden test surface — `{}`",
                    path.strip_prefix(&root).unwrap_or(&path).display(),
                    lineno + 1,
                    trimmed
                ));
            }
        }
    }
    if violations.is_empty() {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        let _ = writeln!(
            handle,
            "check-rust-test-surface: 0 violations (BDD-only test surface upheld)"
        );
        return Ok(());
    }
    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    for v in &violations {
        let _ = writeln!(handle, "{v}");
    }
    bail!(
        "check-rust-test-surface: {} violation(s); tests must live in tanren-bdd only",
        violations.len()
    );
}

fn check_suppression() -> Result<()> {
    let root = workspace_root()?;
    let mut violations = Vec::<String>::new();
    for path in rust_source_files(&root) {
        let content =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        for (lineno, line) in content.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("#[allow(") || trimmed.starts_with("#[expect(") {
                violations.push(format!(
                    "{}:{}: inline lint suppression — `{}`",
                    path.strip_prefix(&root).unwrap_or(&path).display(),
                    lineno + 1,
                    trimmed
                ));
            }
        }
    }
    if violations.is_empty() {
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        let _ = writeln!(handle, "check-suppression: 0 inline #[allow]/#[expect]");
        return Ok(());
    }
    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    for v in &violations {
        let _ = writeln!(handle, "{v}");
    }
    bail!(
        "check-suppression: {} violation(s); use [lints.clippy] in Cargo.toml instead",
        violations.len()
    );
}

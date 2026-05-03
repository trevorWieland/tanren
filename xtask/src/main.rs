//! Repo maintenance commands for Tanren.

mod bdd_tags;
mod check_bdd_wire_coverage;
mod check_event_coverage;
mod check_newtype_ids;
mod check_orphan_traits;
mod check_profiles;
mod check_secrets;
mod check_test_hooks;
mod check_tracing_init;

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
    #[command(name = "check-rust-test-surface")]
    RustTestSurface,
    /// Reject inline `#[allow(...)]` and `#[expect(...)]` anywhere in
    /// workspace Rust source. Lint relaxations belong in a crate's
    /// `[lints.clippy]` section, not at the source-line level.
    #[command(name = "check-suppression")]
    Suppression,
    /// Validate `tests/bdd/features/**/*.feature` against the F-0002 BDD
    /// convention: filename↔feature-tag match, closed tag allowlist,
    /// strict-equality interface coverage, behavior-catalog cross-check,
    /// and DAG-evidence coverage. See
    /// `docs/architecture/subsystems/behavior-proof.md` for the full
    /// contract.
    #[command(name = "check-bdd-tags")]
    BddTags,
    /// Reject struct fields whose name implies a secret but whose type is
    /// not a `secrecy` wrapper or workspace newtype listed in
    /// `xtask/secret-newtypes.toml`. See
    /// `profiles/rust-cargo/architecture/secrets-handling.md`.
    #[command(name = "check-secrets")]
    Secrets,
    /// Reject BDD step definitions that dispatch directly through
    /// `tanren_app_services::Handlers::*` rather than the
    /// per-interface `*Harness` traits. See
    /// `profiles/rust-cargo/testing/bdd-wire-harness.md`.
    #[command(name = "check-bdd-wire-coverage")]
    BddWireCoverage,
    /// Reject `pub fn`s whose doc-comment hints at test/fixture/seed use
    /// but lack a `#[cfg(test)]` / `#[cfg(feature = "test-hooks")]`
    /// gate. See `docs/architecture/subsystems/state.md`.
    #[command(name = "check-test-hooks")]
    TestHooks,
    /// Reject struct/enum field types that use bare `uuid::Uuid`
    /// outside the newtype declaration sites listed in
    /// `xtask/uuid-allowlist.toml`. See
    /// `profiles/rust-cargo/architecture/id-formats.md`.
    #[command(name = "check-newtype-ids")]
    NewtypeIds,
    /// Reject `bin/*/src/main.rs` files that do not initialize tracing
    /// via `tanren_observability::init`. See
    /// `docs/architecture/subsystems/observation.md`.
    #[command(name = "check-tracing-init")]
    TracingInit,
    /// Reject event variants (enums whose name ends in `Event` /
    /// `EventKind`) without a corresponding BDD scenario asserting the
    /// variant fires. See
    /// `profiles/rust-cargo/global/just-ci-gate.md`.
    #[command(name = "check-event-coverage")]
    EventCoverage,
    /// Validate that profile/architecture markdown links resolve and
    /// every referenced `just <recipe>` / `xtask <subcommand>` exists
    /// (or is listed in `xtask/check-profiles-pending.toml`).
    #[command(name = "check-profiles")]
    Profiles,
    /// Reject `pub trait` definitions that have no implementor in the
    /// workspace. See `profiles/rust-cargo/global/just-ci-gate.md`.
    #[command(name = "check-orphan-traits")]
    OrphanTraits,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = workspace_root()?;
    match cli.command {
        Command::RustTestSurface => check_rust_test_surface(),
        Command::Suppression => check_suppression(),
        Command::BddTags => bdd_tags::run(&root),
        Command::Secrets => check_secrets::run(&root),
        Command::BddWireCoverage => check_bdd_wire_coverage::run(&root),
        Command::TestHooks => check_test_hooks::run(&root),
        Command::NewtypeIds => check_newtype_ids::run(&root),
        Command::TracingInit => check_tracing_init::run(&root),
        Command::EventCoverage => check_event_coverage::run(&root),
        Command::Profiles => check_profiles::run(&root),
        Command::OrphanTraits => check_orphan_traits::run(&root),
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

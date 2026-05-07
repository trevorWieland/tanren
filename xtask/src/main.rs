//! Repo maintenance commands for Tanren.

mod bdd_tags;
mod check_bdd_wire_coverage;
mod check_event_coverage;
mod check_newtype_ids;
mod check_openapi_handcraft;
mod check_orphan_traits;
mod check_profiles;
mod check_secrets;
mod check_test_hooks;
mod check_tracing_init;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
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

/// Shared options exposed by every subcommand. `--root` lets the
/// regression-fixture suite (under `xtask/tests/`) point each guard at a
/// synthetic minimal source tree without rebuilding the workspace; in
/// normal CI runs the flag is absent and the guard scans the real
/// workspace root inferred from `CARGO_MANIFEST_DIR`.
#[derive(Debug, Args, Clone, Default)]
struct RootArg {
    /// Workspace root the check should walk. Defaults to the parent of
    /// the xtask manifest directory.
    #[arg(long, value_name = "PATH", global = false)]
    root: Option<PathBuf>,
}

impl RootArg {
    fn resolve(&self) -> Result<PathBuf> {
        match &self.root {
            Some(p) => Ok(p.clone()),
            None => workspace_root(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Reject any `#[test]`, `#[cfg(test)]`, or `mod tests` outside the
    /// `tanren-bdd` crate (and the xtask integration-test tree, which
    /// hosts the regression-fixture suite). Tests live exclusively in
    /// BDD scenarios; xtask's `tests/` is a closed-loop self-test of the
    /// guards themselves.
    #[command(name = "check-rust-test-surface")]
    RustTestSurface(RootArg),
    /// Reject inline `#[allow(...)]` and `#[expect(...)]` anywhere in
    /// workspace Rust source. Lint relaxations belong in a crate's
    /// `[lints.clippy]` section, not at the source-line level.
    #[command(name = "check-suppression")]
    Suppression(RootArg),
    /// Validate `tests/bdd/features/**/*.feature` against the F-0002 BDD
    /// convention: filename↔feature-tag match, closed tag allowlist,
    /// strict-equality surface coverage, behavior-catalog cross-check,
    /// and DAG-evidence coverage. See
    /// `docs/architecture/subsystems/behavior-proof.md` for the full
    /// contract.
    #[command(name = "check-bdd-tags")]
    BddTags(RootArg),
    /// Reject struct fields whose name implies a secret but whose type is
    /// not a `secrecy` wrapper or workspace newtype listed in
    /// `xtask/secret-newtypes.toml`. See
    /// `profiles/rust-cargo/architecture/secrets-handling.md`.
    #[command(name = "check-secrets")]
    Secrets(RootArg),
    /// Reject BDD step definitions that dispatch directly through
    /// `tanren_app_services::Handlers::*` rather than the
    /// per-interface `*Harness` traits. See
    /// `profiles/rust-cargo/testing/bdd-wire-harness.md`.
    #[command(name = "check-bdd-wire-coverage")]
    BddWireCoverage(RootArg),
    /// Reject `pub fn`s whose doc-comment hints at test/fixture/seed use
    /// but lack a `#[cfg(test)]` / `#[cfg(feature = "test-hooks")]`
    /// gate. See `docs/architecture/subsystems/state.md`.
    #[command(name = "check-test-hooks")]
    TestHooks(RootArg),
    /// Reject struct/enum field types that use bare `uuid::Uuid`
    /// outside the newtype declaration sites listed in
    /// `xtask/uuid-allowlist.toml`. See
    /// `profiles/rust-cargo/architecture/id-formats.md`.
    #[command(name = "check-newtype-ids")]
    NewtypeIds(RootArg),
    /// Reject `bin/*/src/main.rs` files that do not initialize tracing
    /// via `tanren_observability::init`. See
    /// `docs/architecture/subsystems/observation.md`.
    #[command(name = "check-tracing-init")]
    TracingInit(RootArg),
    /// Reject event variants (enums whose name ends in `Event` /
    /// `EventKind`) without a corresponding BDD scenario asserting the
    /// variant fires. See
    /// `profiles/rust-cargo/global/just-ci-gate.md`.
    #[command(name = "check-event-coverage")]
    EventCoverage(RootArg),
    /// Validate that profile/architecture markdown links resolve and
    /// every referenced `just <recipe>` / `xtask <subcommand>` exists
    /// (or is listed in `xtask/check-profiles-pending.toml`).
    #[command(name = "check-profiles")]
    Profiles(RootArg),
    /// Reject `pub trait` definitions that have no implementor in the
    /// workspace. See `profiles/rust-cargo/global/just-ci-gate.md`.
    #[command(name = "check-orphan-traits")]
    OrphanTraits(RootArg),
    /// Reject hand-rolled `serde_json::json!({"openapi": ..., "paths":
    /// ..., "components": ...})` literals in api crates. The api stack
    /// generates its `OpenAPI` document via `utoipa` derives; raw JSON
    /// literals would silently drift from the running server. See
    /// `profiles/rust-cargo/architecture/openapi-generation.md`.
    #[command(name = "check-openapi-handcraft")]
    OpenapiHandcraft(RootArg),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::RustTestSurface(r) => check_rust_test_surface(&r.resolve()?),
        Command::Suppression(r) => check_suppression(&r.resolve()?),
        Command::BddTags(r) => bdd_tags::run(&r.resolve()?),
        Command::Secrets(r) => check_secrets::run(&r.resolve()?),
        Command::BddWireCoverage(r) => check_bdd_wire_coverage::run(&r.resolve()?),
        Command::TestHooks(r) => check_test_hooks::run(&r.resolve()?),
        Command::NewtypeIds(r) => check_newtype_ids::run(&r.resolve()?),
        Command::TracingInit(r) => check_tracing_init::run(&r.resolve()?),
        Command::EventCoverage(r) => check_event_coverage::run(&r.resolve()?),
        Command::Profiles(r) => check_profiles::run(&r.resolve()?),
        Command::OrphanTraits(r) => check_orphan_traits::run(&r.resolve()?),
        Command::OpenapiHandcraft(r) => check_openapi_handcraft::run(&r.resolve()?),
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
        // The xtask crate's own integration-test tree (`xtask/tests/`)
        // hosts:
        //   - the regression-fixture suite (`xtask/tests/regressions.rs`),
        //     the only `#[test]` items the workspace allows outside
        //     `tanren-bdd`. Each fixture proves that a guard rejects a
        //     synthetic violation.
        //   - the synthetic source fixtures themselves
        //     (`xtask/tests/fixtures/<guard>/...`), which are
        //     intentionally invalid Rust by the workspace's own rules
        //     (raw `String` password fields, bare `Uuid` ids, ungated
        //     `pub fn seed_*`, etc.).
        // Sweeping either of these into the workspace-wide guards would
        // either falsely fail `check-rust-test-surface` /
        // `check-suppression`, or make the regression suite have to
        // tiptoe around its own checks. Skipping the tree at the source
        // level is the cleanest fix.
        .filter(|p| !is_under_xtask_tests(p))
}

fn is_under_xtask_tests(path: &Path) -> bool {
    let mut comps = path.components().peekable();
    while let Some(c) = comps.next() {
        if c.as_os_str() == "xtask" {
            if let Some(next) = comps.peek() {
                if next.as_os_str() == "tests" {
                    return true;
                }
            }
        }
    }
    false
}

fn check_rust_test_surface(root: &Path) -> Result<()> {
    let mut violations = Vec::<String>::new();
    for path in rust_source_files(root) {
        if path.components().any(|c| c.as_os_str() == "tanren-bdd") {
            continue;
        }
        let content =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let mut in_doc_block = false;
        let mut doc_block_runnable = false;
        let mut doc_block_start = 0usize;
        for (lineno, line) in content.lines().enumerate() {
            let trimmed = line.trim_start();
            let hit = trimmed.starts_with("#[test]")
                || trimmed.starts_with("#[tokio::test]")
                || trimmed.starts_with("#[cfg(test)]")
                || trimmed.starts_with("mod tests");
            if hit {
                violations.push(format!(
                    "{}:{}: forbidden test surface — `{}`",
                    path.strip_prefix(root).unwrap_or(&path).display(),
                    lineno + 1,
                    trimmed
                ));
            }
            // Reject runnable doc-tests (``` rust code blocks inside
            // `///` or `//!` comments without `ignore`/`no_run`/`text`
            // markers). Doc-tests are tests by `cargo test` semantics
            // and the BDD-only policy admits zero exceptions: any
            // executable test must live in `tanren-bdd` as a `.feature`
            // scenario. Allowed annotations: `text`, `ignore`,
            // `no_run`, `compile_fail` — all non-runnable.
            if trimmed.starts_with("///") || trimmed.starts_with("//!") {
                let body = trimmed
                    .trim_start_matches("///")
                    .trim_start_matches("//!")
                    .trim_start();
                if let Some(rest) = body.strip_prefix("```") {
                    if in_doc_block {
                        if doc_block_runnable {
                            violations.push(format!(
                                "{}:{}: runnable doc-test — annotate with `text`/`ignore`/`no_run`/`compile_fail` or move to a BDD scenario",
                                path.strip_prefix(root).unwrap_or(&path).display(),
                                doc_block_start + 1,
                            ));
                        }
                        in_doc_block = false;
                        doc_block_runnable = false;
                    } else {
                        in_doc_block = true;
                        doc_block_start = lineno;
                        let lang = rest.trim();
                        let runnable_lang = lang.is_empty()
                            || lang == "rust"
                            || lang.starts_with("rust,")
                            || lang.starts_with("rust ");
                        let opts: Vec<&str> = lang
                            .split(|c: char| c == ',' || c.is_whitespace())
                            .filter(|s| !s.is_empty())
                            .collect();
                        let suppressed = opts.iter().any(|o| {
                            *o == "text" || *o == "ignore" || *o == "no_run" || *o == "compile_fail"
                        });
                        doc_block_runnable = runnable_lang && !suppressed;
                    }
                }
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

fn check_suppression(root: &Path) -> Result<()> {
    let mut violations = Vec::<String>::new();
    for path in rust_source_files(root) {
        let content =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        for (lineno, line) in content.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("#[allow(") || trimmed.starts_with("#[expect(") {
                violations.push(format!(
                    "{}:{}: inline lint suppression — `{}`",
                    path.strip_prefix(root).unwrap_or(&path).display(),
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

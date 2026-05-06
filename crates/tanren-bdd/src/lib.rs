//! BDD step-definition home for Tanren.
//!
//! This is the only crate in the workspace permitted to define `#[test]`
//! items — `xtask check-rust-test-surface` mechanically rejects them
//! anywhere else. R-0001 sub-9 rewires the step bodies to dispatch
//! through the per-interface [`AccountHarness`] trait in
//! `tanren-testkit`, so the surface under proof matches the scenario's
//! interface tag — `@api` drives reqwest, `@cli` drives the binary,
//! `@mcp` drives the rmcp client, etc. `xtask check-bdd-wire-coverage`
//! mechanically rejects any step body that calls
//! `tanren_app_services::Handlers::*` directly.

pub mod steps;

use cucumber::World as CucumberWorld;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use tanren_testkit::{
    AccountHarness, ActorState, ApiHarness, CliHarness, FixtureSeed, HarnessKind, HarnessOutcome,
    InProcessHarness, McpHarness, TuiHarness, WebHarness,
};

/// Cucumber `World` shared across all Tanren BDD scenarios.
#[derive(Debug, Default, CucumberWorld)]
pub struct TanrenWorld {
    /// Deterministic fixture seed.
    pub seed: FixtureSeed,
    /// Lazily initialized account-flow context.
    pub account: Option<AccountContext>,
}

impl TanrenWorld {
    /// Construct (or return) the lazy account context.
    pub async fn ensure_account_ctx(&mut self) -> &mut AccountContext {
        if self.account.is_none() {
            self.account = Some(AccountContext::new_in_process().await);
        }
        self.account
            .as_mut()
            .expect("account context just initialized")
    }

    /// Refresh the account context with the harness chosen for the
    /// supplied scenario tags. Cucumber-rs does not give step bodies
    /// access to the active scenario's tags, so the BDD bin invokes
    /// this from a `Before` hook.
    pub async fn install_harness_for_tags<I, S>(&mut self, tags: I)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let kind = HarnessKind::from_tags(tags);
        let ctx = AccountContext::new_for(kind).await;
        self.account = Some(ctx);
    }
}

/// Per-scenario state carried by the cucumber world. Tracks per-actor
/// outcomes plus the active wire harness — all transport-specific
/// state lives inside the harness implementation.
pub struct AccountContext {
    /// Active wire harness for the current scenario.
    pub harness: Box<dyn AccountHarness>,
    /// Registry of actors by display name.
    pub actors: HashMap<String, ActorState>,
    /// The most recent action's outcome.
    pub last_outcome: Option<HarnessOutcome>,
    /// Per-scenario invitation tokens recorded by `Given a pending
    /// invitation token "..."` style steps.
    pub invitations: HashSet<String>,
}

impl std::fmt::Debug for AccountContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountContext")
            .field("harness_kind", &self.harness.kind())
            .field("actors", &self.actors.keys().collect::<Vec<_>>())
            .field("invitations", &self.invitations)
            .field(
                "last_outcome",
                &self.last_outcome.as_ref().map(short_outcome_label),
            )
            .finish()
    }
}

impl AccountContext {
    /// Build a context with the in-process harness — used for
    /// untagged scenarios.
    pub async fn new_in_process() -> Self {
        Self::new_for(HarnessKind::InProcess).await
    }

    /// Build a context with the harness matching the supplied tag
    /// kind. Falls back to the in-process harness if the requested
    /// transport fails to come up (e.g. a missing CLI binary on a
    /// fresh checkout) — the failure is recorded in `last_outcome`
    /// so it surfaces during the first step rather than blocking
    /// scenario discovery.
    pub async fn new_for(kind: HarnessKind) -> Self {
        let harness: Box<dyn AccountHarness> = match kind {
            HarnessKind::InProcess => Box::new(
                InProcessHarness::new(kind)
                    .await
                    .expect("ephemeral SQLite must connect for BDD"),
            ),
            HarnessKind::Api => Box::new(ApiHarness::spawn().await.expect("ApiHarness::spawn")),
            HarnessKind::Cli => Box::new(CliHarness::spawn().await.expect("CliHarness::spawn")),
            HarnessKind::Mcp => Box::new(McpHarness::spawn().await.expect("McpHarness::spawn")),
            HarnessKind::Tui => Box::new(TuiHarness::spawn().await.expect("TuiHarness::spawn")),
            // PR 11 ships the real-browser proof on the Node side via
            // `playwright-bdd`; the Rust path keeps in-process fallback
            // for fast feedback. See `tanren_testkit::harness::web`.
            HarnessKind::Web => Box::new(WebHarness::spawn().await.expect("WebHarness::spawn")),
        };
        Self {
            harness,
            actors: HashMap::new(),
            last_outcome: None,
            invitations: HashSet::new(),
        }
    }
}

fn short_outcome_label(outcome: &HarnessOutcome) -> &'static str {
    match outcome {
        HarnessOutcome::SignedUp(_) => "SignedUp",
        HarnessOutcome::SignedIn(_) => "SignedIn",
        HarnessOutcome::AcceptedInvitation(_) => "AcceptedInvitation",
        HarnessOutcome::Failure(_) => "Failure",
        HarnessOutcome::Other(_) => "Other",
    }
}

/// Run the cucumber harness against the supplied features directory.
/// The harness installs a `Before` hook that selects the per-interface
/// wire harness from the active scenario's tags.
pub async fn run_features(features_dir: impl Into<PathBuf>) {
    TanrenWorld::cucumber()
        .before(|_feature, _rule, scenario, world| {
            let tags = scenario.tags.clone();
            Box::pin(async move {
                world.install_harness_for_tags(tags).await;
            })
        })
        .fail_on_skipped()
        .run_and_exit(features_dir.into())
        .await;
}

#[cfg(test)]
mod tests {
    //! Unit-test guards for the BDD harness machinery itself.

    use super::TanrenWorld;
    use tanren_testkit::FixtureSeed;

    #[test]
    fn world_default_is_constructible() {
        let world = TanrenWorld::default();
        assert_eq!(world.seed, FixtureSeed::default());
    }

    #[test]
    fn world_seed_round_trips() {
        let world = TanrenWorld {
            seed: FixtureSeed::new(42),
            account: None,
        };
        assert_eq!(world.seed.value(), 42);
    }
}

#[cfg(test)]
mod uninstall_api_smoke;

#[cfg(test)]
mod uninstall_mcp_smoke;

#[cfg(test)]
mod uninstall_smoke {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use chrono::Utc;
    use sha2::{Digest, Sha256};
    use tanren_contract::{FileOwnership, InstallManifest, ManifestEntry};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TempRepo {
        path: PathBuf,
    }

    impl TempRepo {
        fn new(label: &str) -> Self {
            let id = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("tanren-uninstall-test-{label}-{id}"));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("create temp repo");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write_file(&self, rel: &str, content: &[u8]) {
            let full = self.path.join(rel);
            if let Some(parent) = full.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::write(full, content).expect("write file");
        }

        fn file_exists(&self, rel: &str) -> bool {
            self.path.join(rel).exists()
        }

        fn read_file(&self, rel: &str) -> String {
            fs::read_to_string(self.path.join(rel)).expect("read file")
        }

        fn write_manifest(&self, manifest: &InstallManifest) {
            let dir = self.path.join(".tanren");
            fs::create_dir_all(&dir).expect("create .tanren dir");
            let json = serde_json::to_string_pretty(manifest).expect("serialize manifest");
            fs::write(dir.join("install-manifest.json"), json).expect("write manifest");
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn sha256_hex(content: &[u8]) -> String {
        let digest = Sha256::digest(content);
        let mut hex = String::with_capacity(digest.len() * 2);
        for byte in digest.as_slice() {
            use std::fmt::Write;
            let _ = write!(hex, "{byte:02x}");
        }
        hex
    }

    fn locate_cli_binary() -> PathBuf {
        if let Ok(explicit) = std::env::var("TANREN_BIN_TANREN_CLI") {
            let p = PathBuf::from(explicit);
            if p.exists() {
                return p;
            }
        }
        let exe = std::env::current_exe().expect("current exe");
        let dir = exe.parent().expect("exe parent");
        let candidate = dir.join("tanren-cli");
        if candidate.exists() {
            return candidate;
        }
        if let Some(parent) = dir.parent() {
            let parent_candidate = parent.join("tanren-cli");
            if parent_candidate.exists() {
                return parent_candidate;
            }
        }
        let fallback = dir.join("tanren-cli");
        let msg = format!(
            "tanren-cli binary not found near {} — run `cargo build -p tanren-cli`",
            exe.display(),
        );
        assert!(fallback.exists(), "{msg}");
        fallback
    }

    fn run_uninstall(repo: &Path, confirm: bool) -> std::process::Output {
        let binary = locate_cli_binary();
        let repo_str = repo.display().to_string();
        let mut args = vec!["uninstall", "--repo", &repo_str];
        if confirm {
            args.push("--confirm");
        }
        Command::new(&binary)
            .args(&args)
            .output()
            .expect("run tanren-cli uninstall")
    }

    #[test]
    fn uninstall_no_manifest_prints_nothing_to_uninstall() {
        let repo = TempRepo::new("no-manifest");
        let output = run_uninstall(repo.path(), false);

        assert!(
            output.status.success(),
            "exit code should be 0, got {}\nstderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr),
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("nothing to uninstall"),
            "expected nothing-to-uninstall message, got: {stdout}",
        );
    }

    #[test]
    fn uninstall_preview_preserves_all_files() {
        let repo = TempRepo::new("preview");

        let generated_content = b"generated by tanren\n";
        let generated_hash = sha256_hex(generated_content);
        repo.write_file("generated.txt", generated_content);
        repo.write_file("user-spec.md", b"my spec content\n");

        repo.write_manifest(&InstallManifest {
            version: 1,
            entries: vec![
                ManifestEntry {
                    path: "generated.txt".into(),
                    ownership: FileOwnership::TanrenGenerated,
                    content_hash: generated_hash,
                    generated_at: Utc::now(),
                },
                ManifestEntry {
                    path: "user-spec.md".into(),
                    ownership: FileOwnership::UserOwned,
                    content_hash: sha256_hex(b"my spec content\n"),
                    generated_at: Utc::now(),
                },
            ],
            created_at: Utc::now(),
        });

        let output = run_uninstall(repo.path(), false);

        assert!(
            output.status.success(),
            "exit code should be 0, got {}\nstderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr),
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("will remove"),
            "expected will-remove section, got: {stdout}",
        );
        assert!(
            stdout.contains("will preserve"),
            "expected will-preserve section, got: {stdout}",
        );
        assert!(
            stdout.contains("--confirm"),
            "expected --confirm hint, got: {stdout}",
        );

        assert!(
            repo.file_exists("generated.txt"),
            "generated.txt should still exist after preview",
        );
        assert!(
            repo.file_exists("user-spec.md"),
            "user-spec.md should still exist after preview",
        );
        assert!(
            repo.file_exists(".tanren/install-manifest.json"),
            "manifest should still exist after preview",
        );
    }

    #[test]
    fn uninstall_confirmed_removes_unchanged_preserves_modified_and_user_owned() {
        let repo = TempRepo::new("confirmed");

        let unchanged_content = b"generated by tanren\n";
        let unchanged_hash = sha256_hex(unchanged_content);
        repo.write_file("generated.txt", unchanged_content);

        let original_standard = b"original standard\n";
        let original_hash = sha256_hex(original_standard);
        repo.write_file("standard.md", b"edited standard content\n");

        let user_content = b"my important spec\n";
        let user_hash = sha256_hex(user_content);
        repo.write_file("user-spec.md", user_content);

        repo.write_manifest(&InstallManifest {
            version: 1,
            entries: vec![
                ManifestEntry {
                    path: "generated.txt".into(),
                    ownership: FileOwnership::TanrenGenerated,
                    content_hash: unchanged_hash,
                    generated_at: Utc::now(),
                },
                ManifestEntry {
                    path: "standard.md".into(),
                    ownership: FileOwnership::TanrenGenerated,
                    content_hash: original_hash,
                    generated_at: Utc::now(),
                },
                ManifestEntry {
                    path: "user-spec.md".into(),
                    ownership: FileOwnership::UserOwned,
                    content_hash: user_hash,
                    generated_at: Utc::now(),
                },
            ],
            created_at: Utc::now(),
        });

        let output = run_uninstall(repo.path(), true);

        assert!(
            output.status.success(),
            "exit code should be 0, got {}\nstderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr),
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("uninstall applied"),
            "expected uninstall applied message, got: {stdout}",
        );

        assert!(
            !repo.file_exists("generated.txt"),
            "unchanged generated file should be removed",
        );

        assert!(
            repo.file_exists("standard.md"),
            "modified standard should be preserved",
        );
        assert_eq!(
            repo.read_file("standard.md"),
            "edited standard content\n",
            "modified standard content should be unchanged",
        );

        assert!(
            repo.file_exists("user-spec.md"),
            "user spec should be preserved",
        );
        assert_eq!(
            repo.read_file("user-spec.md"),
            "my important spec\n",
            "user spec content should be unchanged",
        );

        assert!(
            !repo.file_exists(".tanren/install-manifest.json"),
            "manifest should be removed",
        );
    }
}

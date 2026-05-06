//! Install-flow BDD harness — shells out to the `tanren-cli` binary's
//! `install` subcommand against a per-scenario temporary repository
//! directory.
//!
//! The harness owns the temp directory and cleans it up on drop. Each
//! `run_install` call spawns a `tanren-cli install --profile <p>
//! --repo <dir>` subprocess and captures stdout/stderr plus the exit
//! status. Step definitions read the captured output and inspect the
//! filesystem through the harness's `repo_dir()` accessor.

use std::path::PathBuf;
use std::process::Stdio;

use tokio::process::Command;

use super::cli::locate_workspace_binary;
use super::{HarnessError, HarnessResult};

pub struct InstallHarness {
    repo_dir: PathBuf,
    binary: PathBuf,
    last_invocation: Option<InstallInvocation>,
}

#[derive(Debug, Clone)]
pub struct InstallInvocation {
    pub exit_success: bool,
    pub stdout: String,
    pub stderr: String,
}

impl std::fmt::Debug for InstallHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstallHarness")
            .field("repo_dir", &self.repo_dir)
            .field("binary", &self.binary)
            .field("last_invocation", &self.last_invocation)
            .finish_non_exhaustive()
    }
}

impl InstallHarness {
    pub fn new() -> HarnessResult<Self> {
        let binary = locate_workspace_binary("tanren-cli")?;
        let repo_dir = Self::create_temp_repo()?;
        Ok(Self {
            repo_dir,
            binary,
            last_invocation: None,
        })
    }

    fn create_temp_repo() -> HarnessResult<PathBuf> {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "tanren-bdd-install-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&p)
            .map_err(|e| HarnessError::Transport(format!("create temp repo dir: {e}")))?;
        Ok(p)
    }

    pub fn repo_dir(&self) -> &std::path::Path {
        &self.repo_dir
    }

    pub fn last_invocation(&self) -> Option<&InstallInvocation> {
        self.last_invocation.as_ref()
    }

    pub async fn run_install(
        &mut self,
        profile: &str,
        integrations: Option<&[&str]>,
    ) -> HarnessResult<&InstallInvocation> {
        let mut args = vec![
            "install".to_owned(),
            "--profile".to_owned(),
            profile.to_owned(),
            "--repo".to_owned(),
            self.repo_dir.to_string_lossy().into_owned(),
        ];
        if let Some(ints) = integrations {
            if !ints.is_empty() {
                args.push("--integrations".to_owned());
                args.push(ints.join(","));
            }
        }

        let output = Command::new(&self.binary)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| HarnessError::Transport(format!("spawn tanren-cli install: {e}")))?;

        self.last_invocation = Some(InstallInvocation {
            exit_success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
        Ok(self.last_invocation.as_ref().expect("just set above"))
    }

    pub fn write_file(&self, relative_path: &str, content: &str) -> HarnessResult<()> {
        let full_path = self.repo_dir.join(relative_path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| HarnessError::Transport(format!("create dir: {e}")))?;
        }
        std::fs::write(&full_path, content)
            .map_err(|e| HarnessError::Transport(format!("write file: {e}")))?;
        Ok(())
    }

    pub fn read_file(&self, relative_path: &str) -> HarnessResult<String> {
        let full_path = self.repo_dir.join(relative_path);
        std::fs::read_to_string(&full_path)
            .map_err(|e| HarnessError::Transport(format!("read file: {e}")))
    }

    pub fn file_exists(&self, relative_path: &str) -> bool {
        self.repo_dir.join(relative_path).exists()
    }
}

impl Drop for InstallHarness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.repo_dir);
    }
}

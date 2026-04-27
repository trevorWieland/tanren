use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone)]
pub enum TanrenCliBinary {
    Direct(PathBuf),
    CargoRun { workspace_root: PathBuf },
    BuildOnce { workspace_root: PathBuf },
}

impl TanrenCliBinary {
    pub fn from_env(workspace_root: impl Into<PathBuf>) -> Self {
        if let Some(path) = std::env::var_os("TANREN_TEST_BIN_TANREN_CLI") {
            Self::Direct(PathBuf::from(path))
        } else {
            let workspace_root = workspace_root.into();
            match std::env::var("TANREN_BDD_BIN_MODE").as_deref() {
                Ok("cargo-run") => Self::CargoRun { workspace_root },
                Ok("build-once") => Self::BuildOnce { workspace_root },
                _ => Self::Direct(PathBuf::from("target/debug/tanren-cli")),
            }
        }
    }

    pub fn run(&self, args: Vec<String>, repo_root: Option<&Path>) -> Result<CommandOutput> {
        let mut command = match self {
            Self::Direct(path) => Command::new(path),
            Self::BuildOnce { workspace_root } => {
                Command::new(build_tanren_cli_once(workspace_root)?)
            }
            Self::CargoRun { workspace_root } => {
                let mut command = Command::new("cargo");
                command.current_dir(workspace_root).args([
                    "run",
                    "--quiet",
                    "-p",
                    "tanren-cli",
                    "--locked",
                    "--",
                ]);
                command
            }
        };

        command.args(args);
        if let Some(root) = repo_root {
            command.arg("--repo-root").arg(root);
        }

        let output = command.output().context("run tanren-cli")?;
        Ok(CommandOutput {
            status: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

fn build_tanren_cli_once(workspace_root: &Path) -> Result<PathBuf> {
    static BUILT_CLI: OnceLock<std::result::Result<PathBuf, String>> = OnceLock::new();
    let result =
        BUILT_CLI.get_or_init(|| build_tanren_cli(workspace_root).map_err(|err| err.to_string()));
    match result {
        Ok(path) => Ok(path.clone()),
        Err(message) => bail!("{message}"),
    }
}

fn build_tanren_cli(workspace_root: &Path) -> Result<PathBuf> {
    let output = Command::new("cargo")
        .current_dir(workspace_root)
        .args(["build", "--locked", "-p", "tanren-cli"])
        .output()
        .context("build tanren-cli for BDD subprocesses")?;
    if !output.status.success() {
        bail!(
            "build tanren-cli failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(target_debug_binary(workspace_root, "tanren-cli"))
}

fn target_debug_binary(workspace_root: &Path, name: &str) -> PathBuf {
    let mut path = std::env::var_os("CARGO_TARGET_DIR")
        .map_or_else(|| workspace_root.join("target"), PathBuf::from);
    if path.is_relative() {
        path = workspace_root.join(path);
    }
    path.push("debug");
    path.push(name);
    path
}

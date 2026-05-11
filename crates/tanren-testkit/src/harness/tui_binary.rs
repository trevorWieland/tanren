//! Locate or build the `tanren-tui` binary for the BDD harness.

use std::path::PathBuf;
use std::process::Command;

use super::HarnessError;
use super::cli::locate_workspace_binary;

pub(crate) fn locate_or_build_tui_binary() -> Result<PathBuf, HarnessError> {
    match locate_workspace_binary("tanren-tui") {
        Ok(path) => Ok(path),
        Err(initial) => {
            build_tui_binary()?;
            locate_workspace_binary("tanren-tui").map_err(|final_err| {
                HarnessError::Transport(format!(
                    "{initial}; attempted `cargo build --bin tanren-tui` but binary is still missing: {final_err}"
                ))
            })
        }
    }
}

fn build_tui_binary() -> Result<(), HarnessError> {
    let output = Command::new("cargo")
        .args(["build", "-q", "--locked", "--bin", "tanren-tui"])
        .current_dir(workspace_root())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| HarnessError::Transport(format!("spawn cargo build for `tanren-tui`: {e}")))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(HarnessError::Transport(format!(
        "cargo build --bin tanren-tui failed: {stderr}"
    )))
}

fn workspace_root() -> &'static std::path::Path {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root must exist")
}

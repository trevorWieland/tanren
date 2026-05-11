use std::path::PathBuf;

use regex::Regex;
use tanren_contract::AccountView;
use tanren_identity_policy::{AccountId, Identifier, OrgId};
use uuid::Uuid;

use super::common::code_to_reason;
use super::{HarnessError, HarnessResult};

pub(crate) fn compile_regex(pattern: &str, context: &str) -> HarnessResult<Regex> {
    Regex::new(pattern)
        .map_err(|e| HarnessError::Transport(format!("compile {context} regex: {e}")))
}

/// Locate a workspace binary by name. The BDD runner is at
/// `target/<profile>/tanren-bdd-runner`; sibling binaries live in
/// the same directory.
pub(crate) fn locate_workspace_binary(name: &str) -> HarnessResult<PathBuf> {
    if let Ok(explicit) = std::env::var(format!(
        "TANREN_BIN_{}",
        name.replace('-', "_").to_uppercase()
    )) {
        let p = PathBuf::from(explicit);
        if p.exists() {
            return Ok(p);
        }
    }
    let exe = std::env::current_exe()
        .map_err(|e| HarnessError::Transport(format!("current exe: {e}")))?;
    let dir = exe
        .parent()
        .ok_or_else(|| HarnessError::Transport("current exe has no parent".to_owned()))?;
    let mut candidate = dir.join(name);
    if cfg!(windows) {
        candidate.set_extension("exe");
    }
    if candidate.exists() {
        return Ok(candidate);
    }
    // Fallback: walk up to the workspace root and check
    // `target/{debug,release}/<bin>`.
    let mut cursor = dir;
    while let Some(parent) = cursor.parent() {
        for profile in ["debug", "release"] {
            let mut probe = parent.join("target").join(profile).join(name);
            if cfg!(windows) {
                probe.set_extension("exe");
            }
            if probe.exists() {
                return Ok(probe);
            }
        }
        cursor = parent;
    }
    Err(HarnessError::Transport(format!(
        "binary `{name}` not found alongside test executable {} — run `cargo build --workspace`",
        exe.display()
    )))
}

pub(crate) fn translate_cli_error(stderr: &[u8]) -> HarnessError {
    let text = String::from_utf8_lossy(stderr);
    // CLI emits `error: <code> — <summary>` lines.
    let re = match compile_regex(r"error:\s*([a-z_]+)\s*—\s*(.*)", "cli error line") {
        Ok(re) => re,
        Err(err) => return err,
    };
    if let Some(captures) = re.captures(&text) {
        let code = captures.get(1).map_or("", |m| m.as_str());
        let summary = captures.get(2).map_or("", |m| m.as_str()).trim().to_owned();
        if let Some(reason) = code_to_reason(code) {
            return HarnessError::Account(reason, summary);
        }
    }
    HarnessError::Transport(text.into_owned())
}

pub(crate) fn parse_session(
    stdout: &str,
    email: &str,
    display_name: &str,
) -> HarnessResult<(AccountView, bool)> {
    let re = compile_regex(
        r"account_id=([0-9a-fA-F-]+)\s+session=([^\s]+)",
        "account/session output",
    )?;
    let captures = re
        .captures(stdout)
        .ok_or_else(|| HarnessError::Transport(format!("could not parse cli stdout: {stdout}")))?;
    let id_raw = captures.get(1).map_or("", |m| m.as_str());
    let token = captures.get(2).map_or("", |m| m.as_str());
    let id = AccountId::from(
        Uuid::parse_str(id_raw)
            .map_err(|e| HarnessError::Transport(format!("parse account id: {e}")))?,
    );
    let identifier = Identifier::from_email(
        &tanren_identity_policy::Email::parse(email)
            .map_err(|e| HarnessError::Transport(format!("parse email: {e}")))?,
    );
    let account = AccountView {
        id,
        identifier,
        display_name: if display_name.is_empty() {
            String::new()
        } else {
            display_name.to_owned()
        },
        org: None,
    };
    Ok((account, !token.is_empty()))
}

pub(crate) fn parse_joined_org(stdout: &str) -> HarnessResult<OrgId> {
    let re = compile_regex(r"joined_org=([0-9a-fA-F-]+)", "joined_org output")?;
    let captures = re.captures(stdout).ok_or_else(|| {
        HarnessError::Transport(format!(
            "could not parse joined_org from cli stdout: {stdout}"
        ))
    })?;
    let raw = captures.get(1).map_or("", |m| m.as_str());
    Ok(OrgId::from(Uuid::parse_str(raw).map_err(|e| {
        HarnessError::Transport(format!("parse org id: {e}"))
    })?))
}

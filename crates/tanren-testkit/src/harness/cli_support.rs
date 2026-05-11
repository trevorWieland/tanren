use std::path::PathBuf;

use regex::Regex;

use uuid::Uuid;

use super::common::code_to_reason;
use super::{HarnessError, HarnessResult};
use std::str::FromStr as _;
use tanren_contract::{
    AccountView, GrantSource, ListOrganizationMembersResponse, ListOrganizationsResponse,
    OrganizationBehaviorId, OrganizationMemberPermissionGrant, OrganizationMemberView,
    OrganizationProofLink, OrganizationSourceLink, OrganizationView, ReadModelFreshness,
};
use tanren_identity_policy::{AccountId, Identifier, MembershipId, OrgId, OrganizationName};
use tanren_observation::{ClaimValueKind, CompletenessState, FreshnessState, VisibilityState};

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

/// Parse CLI `organization members` output into a response.
pub(crate) fn parse_member_list_output(
    stdout: &str,
) -> HarnessResult<ListOrganizationMembersResponse> {
    let summary_re = compile_regex(
        r"members=\d+\s+next_cursor=([0-9a-fA-F-]+|<none>)",
        "member list summary output",
    )?;
    let row_re = compile_regex(
        r"account_id=([0-9a-fA-F-]+)\s+identifier=([^\s]+)\s+permissions=(.*)",
        "member list row output",
    )?;
    let next_cursor = stdout
        .lines()
        .find_map(|l| summary_re.captures(l))
        .and_then(|c| c.get(1).map(|v| v.as_str()))
        .and_then(|raw| {
            if raw == "<none>" {
                Some(None)
            } else {
                Uuid::parse_str(raw).ok().map(MembershipId::from).map(Some)
            }
        })
        .unwrap_or(None);
    let mut members = Vec::new();
    for line in stdout.lines() {
        let Some(captures) = row_re.captures(line) else {
            continue;
        };
        let member_account_id = AccountId::from(
            Uuid::parse_str(captures.get(1).map_or("", |m| m.as_str()))
                .map_err(|e| HarnessError::Transport(format!("parse member account id: {e}")))?,
        );
        let identifier = captures.get(2).map_or("", |m| m.as_str()).to_owned();
        let raw_perms = captures.get(3).map_or("", |m| m.as_str());
        let granted_permissions = parse_member_grants(raw_perms)?;
        members.push(OrganizationMemberView {
            account_id: member_account_id,
            identifier,
            joined_at: chrono::Utc::now(),
            granted_permissions,
        });
    }
    Ok(ListOrganizationMembersResponse {
        members,
        next_cursor,
        source_link: OrganizationSourceLink {
            event_family: "organization".to_owned(),
            event_kind: "organization_member_joined".to_owned(),
        },
        freshness: ReadModelFreshness {
            projection: "organization_members_by_org".to_owned(),
            checkpoint: None,
            generated_at: chrono::Utc::now(),
            cursor: next_cursor.map(|c| c.to_string()),
            source: "organization_member_store".to_owned(),
            value_kind: ClaimValueKind::Measured,
            completeness: CompletenessState::Complete,
            freshness_state: FreshnessState::Fresh,
            visibility: VisibilityState::Visible,
        },
        proof_link: OrganizationProofLink {
            behavior_id: OrganizationBehaviorId::B0065ListOrganizationMembers,
        },
    })
}

fn parse_member_grants(raw: &str) -> HarnessResult<Vec<OrganizationMemberPermissionGrant>> {
    let mut out = Vec::new();
    if raw.is_empty() {
        return Ok(out);
    }
    for grant_str in raw.split(',').filter(|s| !s.is_empty()) {
        let parts: Vec<&str> = grant_str.splitn(3, ':').collect();
        if parts.len() < 3 {
            continue;
        }
        let permission = tanren_identity_policy::OrganizationPermission::from_str(parts[0])
            .map_err(|_| HarnessError::Transport(format!("unknown permission: {}", parts[0])))?;
        let grant_source = GrantSource::from_str(parts[1])
            .map_err(|_| HarnessError::Transport(format!("unknown grant source: {}", parts[1])))?;
        let granted_by = AccountId::from(
            Uuid::parse_str(parts[2])
                .map_err(|e| HarnessError::Transport(format!("parse granted_by: {e}")))?,
        );
        out.push(OrganizationMemberPermissionGrant {
            permission,
            grant_source,
            granted_by_account_id: granted_by,
        });
    }
    Ok(out)
}

/// Parse CLI `organization list` output into a response.
pub(crate) fn parse_organization_list_output(
    stdout: &str,
) -> HarnessResult<ListOrganizationsResponse> {
    let summary_re = compile_regex(
        r"organizations=\d+\s+next_cursor=([0-9a-fA-F-]+|<none>)",
        "organization list summary output",
    )?;
    let row_re = compile_regex(
        r"organization_id=([0-9a-fA-F-]+)\s+name=([^\s]+)",
        "organization list row output",
    )?;
    let next_cursor = stdout
        .lines()
        .find_map(|l| summary_re.captures(l))
        .and_then(|c| c.get(1).map(|v| v.as_str()))
        .and_then(|raw| {
            if raw == "<none>" {
                Some(None)
            } else {
                Uuid::parse_str(raw).ok().map(MembershipId::from).map(Some)
            }
        })
        .unwrap_or(None);
    let mut organizations = Vec::new();
    for line in stdout.lines() {
        if line.starts_with("organizations=") {
            continue;
        }
        let Some(captures) = row_re.captures(line) else {
            continue;
        };
        let id = OrgId::from(
            Uuid::parse_str(captures.get(1).map_or("", |m| m.as_str()))
                .map_err(|e| HarnessError::Transport(format!("parse organization id: {e}")))?,
        );
        let name = OrganizationName::parse(captures.get(2).map_or("", |m| m.as_str()))
            .map_err(|e| HarnessError::Transport(format!("parse organization name: {e}")))?;
        organizations.push(OrganizationView {
            id,
            name,
            capabilities: Vec::new(),
        });
    }
    Ok(ListOrganizationsResponse {
        organizations,
        next_cursor,
        source_link: OrganizationSourceLink {
            event_family: "organization".to_owned(),
            event_kind: "organization_created".to_owned(),
        },
        freshness: ReadModelFreshness {
            projection: "organizations_by_account_membership".to_owned(),
            checkpoint: None,
            generated_at: chrono::Utc::now(),
            cursor: next_cursor.map(|c| c.to_string()),
            source: "organization_membership_store".to_owned(),
            value_kind: ClaimValueKind::Measured,
            completeness: CompletenessState::Complete,
            freshness_state: FreshnessState::Fresh,
            visibility: VisibilityState::Visible,
        },
    })
}

use chrono::{DateTime, Utc};
use regex::Regex;
use tanren_contract::{
    AccountView, MY_PERMISSIONS_DEFAULT_LIMIT, MyPermissionsPageMeta, MyPermissionsReadMeta,
    MyPermissionsResponse,
};
use tanren_identity_policy::{AccountId, Identifier, OrgId};
use uuid::Uuid;

use super::{HarnessError, HarnessResult};

pub(super) fn parse_session(
    stdout: &str,
    email: &str,
    display_name: &str,
) -> HarnessResult<(AccountView, bool)> {
    let re = Regex::new(r"account_id=([0-9a-fA-F-]+)\s+session=([^\s]+)").expect("constant regex");
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

pub(super) fn parse_joined_org(stdout: &str) -> HarnessResult<OrgId> {
    let re = Regex::new(r"joined_org=([0-9a-fA-F-]+)").expect("constant regex");
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

pub(super) fn parse_permissions_output(stdout: &str) -> HarnessResult<MyPermissionsResponse> {
    let mut organizations = Vec::new();
    let mut projects = Vec::new();
    let mut returned: u16 = 0;
    let mut page_limit = MY_PERMISSIONS_DEFAULT_LIMIT;
    let mut page_request_cursor: Option<String> = None;
    let mut page_next_cursor: Option<String> = None;
    let mut read_source = "permission_introspection_permission_grants_table_v1".to_owned();
    let mut read_generated_at = Utc::now();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() || line == "permissions=none" {
            continue;
        }
        if line.starts_with("page ") {
            let page_meta = parse_page_metadata_line(line)?;
            page_limit = page_meta.limit;
            returned = page_meta.returned;
            page_request_cursor = page_meta.request_cursor;
            page_next_cursor = page_meta.next_cursor;
            continue;
        }
        if line.starts_with("read_metadata ") {
            let read_meta = parse_read_metadata_line(line)?;
            read_source = read_meta.source;
            read_generated_at = read_meta.generated_at;
            continue;
        }
        let Some((scope, scope_id, entry)) = parse_permission_line(line)? else {
            continue;
        };
        match scope {
            CliPermissionScope::Organization => {
                organizations.push(tanren_contract::MyOrganizationPermissions {
                    org_id: OrgId::from(Uuid::parse_str(&scope_id).map_err(|e| {
                        HarnessError::Transport(format!("parse org scope id: {e}"))
                    })?),
                    permissions: vec![entry],
                });
            }
            CliPermissionScope::Project => projects.push(tanren_contract::MyProjectPermissions {
                project_id: tanren_identity_policy::ProjectId::from(
                    Uuid::parse_str(&scope_id).map_err(|e| {
                        HarnessError::Transport(format!("parse project scope id: {e}"))
                    })?,
                ),
                permissions: vec![entry],
            }),
        }
    }
    Ok(MyPermissionsResponse {
        page: MyPermissionsPageMeta {
            limit: page_limit,
            returned,
            request_cursor: page_request_cursor,
            next_cursor: page_next_cursor,
        },
        read_metadata: MyPermissionsReadMeta {
            source: read_source,
            generated_at: read_generated_at,
        },
        organizations,
        projects,
    })
}

fn normalize_optional(raw: String) -> Option<String> {
    if raw == "none" { None } else { Some(raw) }
}

#[derive(Debug)]
struct ParsedCliPageMeta {
    limit: u16,
    returned: u16,
    request_cursor: Option<String>,
    next_cursor: Option<String>,
}

fn parse_page_metadata_line(line: &str) -> HarnessResult<ParsedCliPageMeta> {
    let limit = capture(line, r"limit=(\d+)")?
        .parse()
        .map_err(|e| HarnessError::Transport(format!("parse page limit from cli stdout: {e}")))?;
    let returned = capture(line, r"returned=(\d+)")?.parse().map_err(|e| {
        HarnessError::Transport(format!("parse page returned from cli stdout: {e}"))
    })?;
    Ok(ParsedCliPageMeta {
        limit,
        returned,
        request_cursor: normalize_optional(capture(line, r"request_cursor=(\S+)")?),
        next_cursor: normalize_optional(capture(line, r"next_cursor=(\S+)")?),
    })
}

#[derive(Debug)]
struct ParsedCliReadMeta {
    source: String,
    generated_at: DateTime<Utc>,
}

fn parse_read_metadata_line(line: &str) -> HarnessResult<ParsedCliReadMeta> {
    let generated_at = DateTime::parse_from_rfc3339(&capture(line, r"generated_at=(\S+)")?)
        .map_err(|e| {
            HarnessError::Transport(format!(
                "parse read metadata generated_at from cli stdout: {e}"
            ))
        })?
        .with_timezone(&Utc);
    Ok(ParsedCliReadMeta {
        source: capture(line, r"source=(\S+)")?,
        generated_at,
    })
}

#[derive(Debug, Clone, Copy)]
enum CliPermissionScope {
    Organization,
    Project,
}

fn parse_permission_line(
    line: &str,
) -> HarnessResult<
    Option<(
        CliPermissionScope,
        String,
        tanren_contract::MyPermissionEntry,
    )>,
> {
    let scope = if line.contains("scope=organization") {
        CliPermissionScope::Organization
    } else if line.contains("scope=project") {
        CliPermissionScope::Project
    } else {
        return Ok(None);
    };
    let permission = parse_permission_name(line)?;
    let effective_state = if line.contains("effective_state=Constrained") {
        tanren_identity_policy::PermissionEffectiveState::Constrained
    } else {
        tanren_identity_policy::PermissionEffectiveState::Granted
    };
    let grant_source = if line.contains("source=Direct") {
        tanren_identity_policy::PermissionGrantSource::Direct
    } else {
        let role_template = capture(line, r#"RoleTemplateName\("([^"]+)"\)"#)?;
        tanren_identity_policy::PermissionGrantSource::RoleTemplate {
            role_template: tanren_identity_policy::RoleTemplateName::new(role_template),
        }
    };
    let grant_source_reference = capture(line, r"source_reference=(\S+)")?;
    let policy_constraint = if line.contains("constraint_reason=none") {
        None
    } else {
        let reason = capture(
            line,
            r#"constraint_reason=PolicyConstraintReason\("([^"]+)"\)"#,
        )?;
        let source = if line.contains("constraint_source=OrganizationPolicy") {
            tanren_identity_policy::PolicyConstraintSource::OrganizationPolicy
        } else {
            tanren_identity_policy::PolicyConstraintSource::ProjectPolicy
        };
        Some(tanren_contract::PermissionConstraintView {
            reason: tanren_identity_policy::PolicyConstraintReason::new(reason),
            source,
            source_reference: capture(line, r"constraint_source_reference=(\S+)")?,
        })
    };
    let entry = tanren_contract::MyPermissionEntry {
        permission: tanren_identity_policy::PermissionName::new(permission),
        effective_state,
        grant_source,
        grant_source_reference,
        policy_constraint,
    };
    Ok(Some((
        scope,
        capture(line, r"scope_id=([0-9a-fA-F-]+)")?,
        entry,
    )))
}

fn capture(line: &str, pattern: &str) -> HarnessResult<String> {
    let re = Regex::new(pattern).expect("constant regex");
    let captures = re
        .captures(line)
        .ok_or_else(|| HarnessError::Transport(format!("parse permissions line: {line}")))?;
    Ok(captures.get(1).map_or("", |m| m.as_str()).to_owned())
}

fn parse_permission_name(line: &str) -> HarnessResult<String> {
    if let Ok(name) = capture(line, r#"permission=PermissionName\("([^"]+)"\)"#) {
        return Ok(name);
    }
    let raw = capture(line, r#"permission=("[^"]+"|\S+)"#)?;
    Ok(raw.trim_matches('"').to_owned())
}

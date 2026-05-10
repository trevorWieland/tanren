use std::str::FromStr;
use std::time::Duration;

use expectrl::Regex;
use secrecy::{ExposeSecret, SecretString};
use tanren_contract::AccountFailureReason;
use tanren_identity_policy::{AccountId, OrgId, OrganizationPermission};
use uuid::Uuid;

use super::common::code_to_reason;
use super::{HarnessError, HarnessResult};

pub(super) const TUI_EXPECT_TIMEOUT: Duration = Duration::from_secs(10);
pub(super) const READY_MARKER: &str = "tui_witness op=app kind=ready";
pub(super) const RE_SIGN_UP_SUCCESS: &str = r"tui_witness op=sign_up kind=success account_id=([0-9a-fA-F-]+) session_token_present=(true|false)";
pub(super) const RE_SIGN_IN_SUCCESS: &str = r"tui_witness op=sign_in kind=success account_id=([0-9a-fA-F-]+) session_token_present=(true|false)";
pub(super) const RE_ACCEPT_SUCCESS: &str = r"tui_witness op=accept_invitation kind=success account_id=([0-9a-fA-F-]+) joined_org=([0-9a-fA-F-]+) session_token_present=(true|false)";
pub(super) const RE_CREATE_SUCCESS: &str = r"tui_witness op=create_organization kind=success organization_id=([0-9a-fA-F-]+) granted_permissions=([a-z_,]*) initial_project_count=(\d+) proof_behavior_id=([^\s]+) source_event=([^\s]+)";
pub(super) const RE_LIST_SUCCESS: &str =
    r"tui_witness op=list_organizations kind=success count=(\d+)";
pub(super) const RE_LIST_ROW: &str =
    r#"tui_witness op=list_organizations kind=row organization_id=([0-9a-fA-F-]+) name="([^"]*)""#;
pub(super) const RE_CHECK_SUCCESS: &str = r"tui_witness op=check_organization_permission kind=success account_id=([0-9a-fA-F-]+) org_id=([0-9a-fA-F-]+) permission=([a-z_]+)";
pub(super) const RE_ERROR_SIGN_UP: &str = r"tui_witness op=sign_up kind=error code=([a-z_]+)";
pub(super) const RE_ERROR_SIGN_IN: &str = r"tui_witness op=sign_in kind=error code=([a-z_]+)";
pub(super) const RE_ERROR_ACCEPT: &str =
    r"tui_witness op=accept_invitation kind=error code=([a-z_]+)";
pub(super) const RE_ERROR_CREATE: &str =
    r"tui_witness op=create_organization kind=error code=([a-z_]+)";
pub(super) const RE_ERROR_LIST: &str =
    r"tui_witness op=list_organizations kind=error code=([a-z_]+)";
pub(super) const RE_ERROR_CHECK: &str =
    r"tui_witness op=check_organization_permission kind=error code=([a-z_]+)";

#[derive(Clone, Debug)]
pub(super) struct AccountCredentials {
    pub(super) email: String,
    pub(super) password: SecretString,
}

pub(super) fn parse_account_id(raw: &str, context: &str) -> HarnessResult<AccountId> {
    let uuid = Uuid::parse_str(raw.trim())
        .map_err(|e| HarnessError::Transport(format!("parse account id ({context}): {e}")))?;
    Ok(AccountId::from(uuid))
}

pub(super) fn parse_org_id(raw: &str, context: &str) -> HarnessResult<OrgId> {
    let uuid = Uuid::parse_str(raw.trim())
        .map_err(|e| HarnessError::Transport(format!("parse org id ({context}): {e}")))?;
    Ok(OrgId::from(uuid))
}

pub(super) fn parse_count(raw: &str) -> HarnessResult<usize> {
    raw.trim()
        .parse::<usize>()
        .map_err(|e| HarnessError::Transport(format!("parse organizations count: {e}")))
}

pub(super) fn parse_initial_project_count(raw: &str) -> HarnessResult<u64> {
    raw.trim()
        .parse::<u64>()
        .map_err(|e| HarnessError::Transport(format!("parse initial_project_count: {e}")))
}

pub(super) fn parse_permissions(raw: &str) -> HarnessResult<Vec<OrganizationPermission>> {
    let mut out = Vec::new();
    for piece in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let permission = OrganizationPermission::from_str(piece).map_err(|_| {
            HarnessError::Transport(format!("unknown permission key in tui output: {piece}"))
        })?;
        out.push(permission);
    }
    Ok(out)
}

pub(super) fn parse_source_event(raw: &str) -> HarnessResult<(String, String)> {
    let trimmed = raw.trim();
    let (family, kind) = trimmed.split_once('.').ok_or_else(|| {
        HarnessError::Transport(format!("parse source_event from tui output: {trimmed}"))
    })?;
    Ok((family.to_owned(), kind.to_owned()))
}

pub(super) fn parse_reason_code(raw: &str) -> HarnessResult<AccountFailureReason> {
    code_to_reason(raw.trim()).ok_or_else(|| {
        HarnessError::Transport(format!("unknown failure code in tui output: {raw}"))
    })
}

pub(super) fn open_form(
    session: &mut expectrl::Session,
    menu_index: usize,
    context: &str,
) -> HarnessResult<()> {
    for _ in 0..menu_index {
        send(session, "\t", context)?;
    }
    send(session, "\r", context)?;
    Ok(())
}

pub(super) fn sign_in_in_session(
    session: &mut expectrl::Session,
    credentials: &AccountCredentials,
) -> HarnessResult<()> {
    open_form(session, 1, "open sign-in form")?;
    send(session, &credentials.email, "fill sign-in email")?;
    send(session, "\t", "sign-in next field")?;
    send(
        session,
        credentials.password.expose_secret(),
        "fill sign-in password",
    )?;
    send(session, "\r", "submit sign-in form")?;

    match expect_regex_capture(session, RE_SIGN_IN_SUCCESS, 1, "sign-in success marker") {
        Ok(_) => {
            send(session, "\r", "back to menu from sign-in outcome")?;
            Ok(())
        }
        Err(success_err) => {
            if let Ok(code) =
                expect_regex_capture(session, RE_ERROR_SIGN_IN, 1, "sign-in error marker")
            {
                let reason = parse_reason_code(&code)?;
                Err(HarnessError::Account(reason, reason.summary().to_owned()))
            } else {
                Err(success_err)
            }
        }
    }
}

pub(super) fn expect_literal(
    session: &mut expectrl::Session,
    needle: &str,
    context: &str,
) -> HarnessResult<()> {
    session
        .expect(needle)
        .map_err(|e| HarnessError::Transport(format!("expect `{needle}` ({context}): {e}")))?;
    Ok(())
}

pub(super) fn expect_regex_capture(
    session: &mut expectrl::Session,
    pattern: &str,
    group: usize,
    context: &str,
) -> HarnessResult<String> {
    let captures = session
        .expect(Regex(pattern))
        .map_err(|e| HarnessError::Transport(format!("expect /{pattern}/ ({context}): {e}")))?;
    let bytes = captures.get(group).ok_or_else(|| {
        HarnessError::Transport(format!(
            "missing regex capture group {group} for /{pattern}/ ({context})"
        ))
    })?;
    String::from_utf8(bytes.to_vec())
        .map_err(|e| HarnessError::Transport(format!("capture utf8 decode ({context}): {e}")))
}

pub(super) fn expect_two_regex_captures(
    session: &mut expectrl::Session,
    pattern: &str,
    group_a: usize,
    group_b: usize,
    context: &str,
) -> HarnessResult<(String, String)> {
    let captures = session
        .expect(Regex(pattern))
        .map_err(|e| HarnessError::Transport(format!("expect /{pattern}/ ({context}): {e}")))?;
    let a = captures.get(group_a).ok_or_else(|| {
        HarnessError::Transport(format!(
            "missing regex capture group {group_a} for /{pattern}/ ({context})"
        ))
    })?;
    let b = captures.get(group_b).ok_or_else(|| {
        HarnessError::Transport(format!(
            "missing regex capture group {group_b} for /{pattern}/ ({context})"
        ))
    })?;
    let a = String::from_utf8(a.to_vec()).map_err(|e| {
        HarnessError::Transport(format!("capture utf8 decode group_a ({context}): {e}"))
    })?;
    let b = String::from_utf8(b.to_vec()).map_err(|e| {
        HarnessError::Transport(format!("capture utf8 decode group_b ({context}): {e}"))
    })?;
    Ok((a, b))
}

pub(super) fn expect_three_regex_captures(
    session: &mut expectrl::Session,
    pattern: &str,
    group_a: usize,
    group_b: usize,
    group_c: usize,
    context: &str,
) -> HarnessResult<(String, String, String)> {
    let captures = session
        .expect(Regex(pattern))
        .map_err(|e| HarnessError::Transport(format!("expect /{pattern}/ ({context}): {e}")))?;
    let a = captures.get(group_a).ok_or_else(|| {
        HarnessError::Transport(format!(
            "missing regex capture group {group_a} for /{pattern}/ ({context})"
        ))
    })?;
    let b = captures.get(group_b).ok_or_else(|| {
        HarnessError::Transport(format!(
            "missing regex capture group {group_b} for /{pattern}/ ({context})"
        ))
    })?;
    let c = captures.get(group_c).ok_or_else(|| {
        HarnessError::Transport(format!(
            "missing regex capture group {group_c} for /{pattern}/ ({context})"
        ))
    })?;
    let a = String::from_utf8(a.to_vec()).map_err(|e| {
        HarnessError::Transport(format!("capture utf8 decode group_a ({context}): {e}"))
    })?;
    let b = String::from_utf8(b.to_vec()).map_err(|e| {
        HarnessError::Transport(format!("capture utf8 decode group_b ({context}): {e}"))
    })?;
    let c = String::from_utf8(c.to_vec()).map_err(|e| {
        HarnessError::Transport(format!("capture utf8 decode group_c ({context}): {e}"))
    })?;
    Ok((a, b, c))
}

pub(super) fn expect_five_regex_captures(
    session: &mut expectrl::Session,
    pattern: &str,
    groups: [usize; 5],
    context: &str,
) -> HarnessResult<(String, String, String, String, String)> {
    let captures = session
        .expect(Regex(pattern))
        .map_err(|e| HarnessError::Transport(format!("expect /{pattern}/ ({context}): {e}")))?;
    let mut out = Vec::with_capacity(5);
    for group in groups {
        let bytes = captures.get(group).ok_or_else(|| {
            HarnessError::Transport(format!(
                "missing regex capture group {group} for /{pattern}/ ({context})"
            ))
        })?;
        let value = String::from_utf8(bytes.to_vec()).map_err(|e| {
            HarnessError::Transport(format!(
                "capture utf8 decode group {group} ({context}): {e}"
            ))
        })?;
        out.push(value);
    }
    Ok((
        out.remove(0),
        out.remove(0),
        out.remove(0),
        out.remove(0),
        out.remove(0),
    ))
}

pub(super) fn send(
    session: &mut expectrl::Session,
    text: &str,
    context: &str,
) -> HarnessResult<()> {
    session
        .send(text.as_bytes())
        .map_err(|e| HarnessError::Transport(format!("send input ({context}): {e}")))?;
    Ok(())
}

pub(super) fn close_session(session: &mut expectrl::Session) -> HarnessResult<()> {
    send(session, "\u{3}", "terminate session with Ctrl-C")?;
    session
        .expect(expectrl::Eof)
        .map_err(|e| HarnessError::Transport(format!("expect eof while closing session: {e}")))?;
    Ok(())
}

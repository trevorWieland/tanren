use std::collections::{BTreeMap, BTreeSet, HashMap};

use secrecy::SecretString;
use tanren_contract::{PermissionGrantView, SignInRequest, SignUpRequest};
use tanren_identity_policy::{
    AccountId, Email, OrgId, PermissionGrantId, PermissionGrantSource, PermissionName, RoleId,
    RoleScope, ScopedRole,
};

use crate::{AccountContext, RoleScenarioState};

pub(super) const ROLE_OPERATOR_EMAIL: &str = "role-operator@tanren.test";
pub(super) const ROLE_OPERATOR_PASSWORD: &str = "role-operator-password";

pub(super) fn parse_permissions_csv(raw: &str) -> Vec<PermissionName> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| PermissionName::parse(s).expect("scenario permission list must parse"))
        .collect()
}

pub(super) fn permission_set(perms: &[PermissionName]) -> BTreeSet<String> {
    perms.iter().map(|p| p.as_str().to_owned()).collect()
}

pub(super) fn permission_set_from_grant_map(
    grants: &BTreeMap<String, PermissionGrantId>,
) -> BTreeSet<String> {
    grants.keys().cloned().collect()
}

pub(super) fn permission_grant_ids_by_permission(
    grants: &[PermissionGrantView],
) -> BTreeMap<String, PermissionGrantId> {
    let mut by_permission = HashMap::<String, PermissionGrantId>::new();
    for grant in grants {
        let permission = grant.permission.as_str().to_owned();
        let previous = by_permission.insert(permission.clone(), grant.id);
        assert!(
            previous.is_none(),
            "permission {permission} had duplicate direct grants in apply/read response"
        );
    }
    by_permission.into_iter().collect()
}

pub(super) fn matching_grant_id_set(grant_ids: &[PermissionGrantId]) -> BTreeSet<String> {
    grant_ids.iter().map(ToString::to_string).collect()
}

pub(super) fn parse_outcome_word(word: &str) -> bool {
    assert!(
        word == "allowed" || word == "denied",
        "permission-check outcome must be `allowed` or `denied` (got {word})"
    );
    word == "allowed"
}

pub(super) fn assert_role_template_grant(
    source: PermissionGrantSource,
    revocation: Option<tanren_identity_policy::PermissionGrantRevocation>,
    role_id: RoleId,
) {
    assert_eq!(source, PermissionGrantSource::RoleTemplate { role_id });
    assert_eq!(revocation, None);
}

pub(super) fn synthetic_permissions(permission_count: usize) -> Vec<PermissionName> {
    (0..permission_count)
        .map(|idx| {
            PermissionName::parse(&format!("project.synthetic_{idx}"))
                .expect("synthetic scenario permission must parse")
        })
        .collect()
}

pub(super) fn scenario_role_scope(role: &mut RoleScenarioState) -> RoleScope {
    if let Some(scope) = role.scope {
        scope
    } else {
        let scope = RoleScope::Organization {
            org_id: OrgId::fresh(),
        };
        role.scope = Some(scope);
        scope
    }
}

pub(super) fn active_role(role: &RoleScenarioState) -> ScopedRole {
    role.active_role
        .expect("active role template must be created first")
}

pub(super) async fn ensure_scenario_principal_account(
    ctx: &mut AccountContext,
    alias: String,
) -> AccountId {
    if let Some(account_id) = ctx.role.principals.get(&alias) {
        return *account_id;
    }

    let email = Email::parse(&format!("{alias}@tanren.test"))
        .expect("scenario principal email literal must parse");
    let password = SecretString::from(format!("{alias}-password"));
    let session = ctx
        .harness
        .sign_up(SignUpRequest {
            email: email.clone(),
            password: password.clone(),
            display_name: format!("Principal {alias}"),
        })
        .await
        .expect("scenario principal sign-up should succeed");
    let account_id = session.account.id;
    ctx.role.principals.insert(alias, account_id);

    ctx.harness
        .sign_in(SignInRequest {
            email: Email::parse(ROLE_OPERATOR_EMAIL)
                .expect("role-operator scenario email literal must parse"),
            password: SecretString::from(ROLE_OPERATOR_PASSWORD.to_owned()),
        })
        .await
        .expect("role-operator sign-in should succeed after principal provisioning");
    account_id
}

pub(super) fn missing_principal_account_id(_role: &RoleScenarioState, _alias: String) -> AccountId {
    AccountId::fresh()
}

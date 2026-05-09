//! Role-template step definitions for B-0038.
//!
//! Step bodies dispatch through the per-interface
//! [`RoleHarness`](tanren_testkit::RoleHarness) trait.

use std::collections::BTreeSet;

use cucumber::{given, then, when};
use secrecy::SecretString;
use tanren_contract::{
    ApplyRoleRequest, CreateRoleRequest, DeleteRoleRequest, EditRoleRequest,
    PermissionCheckRequest, SignUpRequest,
};
use tanren_identity_policy::{
    AccountId, Email, OrgId, PermissionName, PermissionScope, PrincipalRef, RoleName, RoleScope,
    ScopedRole,
};

use crate::{RoleScenarioState, TanrenWorld};

#[given(expr = "a clean role-template environment")]
async fn given_clean_role_env(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    let operator_email = Email::parse("role-operator@tanren.test")
        .expect("role-operator scenario email literal must parse");
    ctx.harness
        .sign_up(SignUpRequest {
            email: operator_email,
            password: SecretString::from("role-operator-password".to_owned()),
            display_name: "Role Operator".to_owned(),
        })
        .await
        .expect("role-operator sign-up should succeed");
    ctx.role = RoleScenarioState::default();
}

#[given(expr = "an organization role scope")]
async fn given_org_role_scope(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    let org_id = OrgId::fresh();
    let scope = RoleScope::Organization { org_id };
    ctx.harness
        .seed_role_admin_for_authenticated_actor(
            scope,
            vec![
                PermissionName::parse("roles.manage")
                    .expect("roles.manage permission literal must parse"),
                PermissionName::parse("roles.read")
                    .expect("roles.read permission literal must parse"),
            ],
        )
        .await
        .expect("seed role-admin permissions for authenticated actor");
    ctx.role.scope = Some(scope);
}

#[when(expr = "the operator creates role template {string} with permissions {string}")]
async fn when_create_role(world: &mut TanrenWorld, name: String, permissions: String) {
    let ctx = world.ensure_account_ctx().await;
    let scope = scenario_role_scope(&mut ctx.role);
    let request = CreateRoleRequest {
        scope,
        name: RoleName::parse(&name).expect("scenario role names must parse"),
        permissions: parse_permissions_csv(&permissions),
    };
    let created = ctx
        .harness
        .create_role(request)
        .await
        .expect("create role template should succeed");
    ctx.role.active_role = Some(ScopedRole {
        role_id: created.role.id,
        scope: created.role.scope,
    });
    ctx.role.last_error_code = None;
    ctx.role.last_permission_check = None;
}

#[when(
    expr = "the operator edits the active role template to name {string} and permissions {string}"
)]
async fn when_edit_role(world: &mut TanrenWorld, name: String, permissions: String) {
    let ctx = world.ensure_account_ctx().await;
    let role = active_role(&ctx.role);
    let request = EditRoleRequest {
        role,
        name: RoleName::parse(&name).expect("scenario role names must parse"),
        permissions: parse_permissions_csv(&permissions),
    };
    let edited = ctx
        .harness
        .edit_role(request)
        .await
        .expect("edit role template should succeed");
    ctx.role.active_role = Some(ScopedRole {
        role_id: edited.role.id,
        scope: edited.role.scope,
    });
    ctx.role.last_error_code = None;
    ctx.role.last_permission_check = None;
}

#[when(expr = "the operator deletes the active role template")]
async fn when_delete_role(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    let role = active_role(&ctx.role);
    let deleted = ctx
        .harness
        .delete_role(DeleteRoleRequest { role })
        .await
        .expect("delete role template should succeed");
    assert_eq!(deleted.role, role, "delete response must echo deleted role");
    ctx.role.last_error_code = None;
    ctx.role.last_permission_check = None;
}

#[when(expr = "the operator applies the role template to account principal {word}")]
async fn when_apply_role_to_account(world: &mut TanrenWorld, alias: String) {
    let ctx = world.ensure_account_ctx().await;
    let role = active_role(&ctx.role);
    let grant_scope = permission_scope_from_role_scope(scenario_role_scope(&mut ctx.role));
    let account_id = scenario_principal_account_id(&mut ctx.role, alias);
    let principal = PrincipalRef::Account { account_id };
    let applied = ctx
        .harness
        .apply_role(ApplyRoleRequest {
            role,
            principal,
            grant_scope,
        })
        .await
        .expect("apply role template should succeed");
    assert_eq!(applied.role, role, "apply response role mismatch");
    assert!(
        !applied.grants.is_empty(),
        "apply response should include direct grants"
    );
    for grant in &applied.grants {
        assert_eq!(grant.principal, principal, "grant principal mismatch");
        assert_eq!(grant.scope, grant_scope, "grant scope mismatch");
        assert_eq!(
            grant.source_role_id, role.role_id,
            "grant source role must match applied role"
        );
    }
    ctx.role.last_error_code = None;
    ctx.role.last_permission_check = None;
}

#[when(expr = "the operator checks permission {string} for account principal {word}")]
async fn when_check_permission_for_account(
    world: &mut TanrenWorld,
    permission: String,
    alias: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let account_id = scenario_principal_account_id(&mut ctx.role, alias);
    let request = PermissionCheckRequest {
        principal: PrincipalRef::Account { account_id },
        permission: PermissionName::parse(&permission).expect("scenario permission must parse"),
        scope: permission_scope_from_role_scope(scenario_role_scope(&mut ctx.role)),
    };
    let response = ctx
        .harness
        .check_permission(request)
        .await
        .expect("permission check for account principal should succeed");
    ctx.role.last_permission_check = Some(response);
    ctx.role.last_error_code = None;
}

#[when(expr = "the operator checks permission {string} for the role template principal")]
async fn when_check_permission_for_role_principal(world: &mut TanrenWorld, permission: String) {
    let ctx = world.ensure_account_ctx().await;
    let role = active_role(&ctx.role);
    let request = PermissionCheckRequest {
        principal: PrincipalRef::Role {
            role_id: role.role_id,
        },
        permission: PermissionName::parse(&permission).expect("scenario permission must parse"),
        scope: permission_scope_from_role_scope(scenario_role_scope(&mut ctx.role)),
    };
    match ctx.harness.check_permission(request).await {
        Ok(response) => {
            ctx.role.last_permission_check = Some(response);
            ctx.role.last_error_code = Some("unexpected_success".to_owned());
        }
        Err(err) => {
            ctx.role.last_error_code = Some(err.code());
            ctx.role.last_permission_check = None;
        }
    }
}

#[then(expr = "the active role template has permissions {string}")]
async fn then_role_has_permissions(world: &mut TanrenWorld, permissions: String) {
    let ctx = world.ensure_account_ctx().await;
    let role = active_role(&ctx.role);
    let actual = ctx
        .harness
        .read_role_template(role)
        .await
        .expect("read role template should succeed")
        .expect("active role template should exist");
    assert_eq!(
        permission_set(&actual.permissions),
        permission_set(&parse_permissions_csv(&permissions)),
        "role permission bundle mismatch"
    );
}

#[then(expr = "the active role template no longer exists")]
async fn then_role_no_longer_exists(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    let role = active_role(&ctx.role);
    let actual = ctx
        .harness
        .read_role_template(role)
        .await
        .expect("read role template should succeed");
    assert!(actual.is_none(), "role template should have been deleted");
}

#[then(expr = "account principal {word} has direct grants {string}")]
#[then(expr = "account principal {word} retains direct grants {string}")]
async fn then_account_has_direct_grants(
    world: &mut TanrenWorld,
    alias: String,
    permissions: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let role = active_role(&ctx.role);
    let account_id = scenario_principal_account_id(&mut ctx.role, alias);
    let principal = PrincipalRef::Account { account_id };
    let grants = ctx
        .harness
        .read_direct_grants(principal)
        .await
        .expect("read direct grants should succeed");
    let expected_permissions = parse_permissions_csv(&permissions);
    let expected_scope = permission_scope_from_role_scope(scenario_role_scope(&mut ctx.role));
    assert!(
        !grants.is_empty(),
        "principal must have at least one direct grant"
    );
    assert_eq!(
        grants.len(),
        expected_permissions.len(),
        "direct-grant count must match expected permission bundle size"
    );
    for grant in &grants {
        assert_eq!(grant.principal, principal, "grant principal mismatch");
        assert_eq!(grant.scope, expected_scope, "grant scope mismatch");
        assert_eq!(
            grant.source_role_id, role.role_id,
            "grant source role must match active role id"
        );
    }
    assert_eq!(
        permission_set(
            &grants
                .iter()
                .map(|grant| grant.permission.clone())
                .collect::<Vec<_>>(),
        ),
        permission_set(&expected_permissions),
        "direct-grant permission set mismatch"
    );
}

#[then(expr = "the permission check result is {word}")]
async fn then_permission_check_result(world: &mut TanrenWorld, outcome: String) {
    let ctx = world.ensure_account_ctx().await;
    let expected = parse_outcome_word(&outcome);
    let actual = ctx
        .role
        .last_permission_check
        .as_ref()
        .expect("permission check response must exist")
        .allowed;
    assert_eq!(
        actual, expected,
        "permission check result mismatch: expected {expected}, got {actual}"
    );
}

#[then(expr = "the role request fails with code {string}")]
async fn then_role_fails_with(world: &mut TanrenWorld, code: String) {
    let ctx = world.ensure_account_ctx().await;
    let actual = ctx
        .role
        .last_error_code
        .clone()
        .unwrap_or_else(|| "no_error".to_owned());
    assert_eq!(actual, code, "unexpected role failure code");
}

fn parse_permissions_csv(raw: &str) -> Vec<PermissionName> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| PermissionName::parse(s).expect("scenario permission list must parse"))
        .collect()
}

fn permission_set(perms: &[PermissionName]) -> BTreeSet<String> {
    perms.iter().map(|p| p.as_str().to_owned()).collect()
}

fn parse_outcome_word(word: &str) -> bool {
    assert!(
        word == "allowed" || word == "denied",
        "permission-check outcome must be `allowed` or `denied` (got {word})"
    );
    word == "allowed"
}

fn scenario_role_scope(role: &mut RoleScenarioState) -> RoleScope {
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

fn permission_scope_from_role_scope(scope: RoleScope) -> PermissionScope {
    match scope {
        RoleScope::Account { account_id } => PermissionScope::Account { account_id },
        RoleScope::Organization { org_id } => PermissionScope::Organization { org_id },
        RoleScope::Project { project_id } => PermissionScope::Project { project_id },
    }
}

fn active_role(role: &RoleScenarioState) -> ScopedRole {
    role.active_role
        .expect("active role template must be created first")
}

fn scenario_principal_account_id(role: &mut RoleScenarioState, alias: String) -> AccountId {
    *role
        .principals
        .entry(alias)
        .or_insert_with(AccountId::fresh)
}

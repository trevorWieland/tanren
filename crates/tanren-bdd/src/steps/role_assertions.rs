//! Supplemental role-template assertion steps for B-0038.

use cucumber::then;
use tanren_identity_policy::{PermissionName, PrincipalRef};

use crate::TanrenWorld;
use crate::steps::role_support::{
    ensure_scenario_principal_account, matching_grant_id_set, parse_permissions_csv,
    permission_set, permission_set_from_grant_map, scenario_role_scope,
};

#[then(
    expr = "the permission check matches direct grant ids for account principal {word} and permission {string}"
)]
async fn then_permission_check_matches_direct_grant_ids(
    world: &mut TanrenWorld,
    alias: String,
    permission: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let account_id = ensure_scenario_principal_account(ctx, alias.clone()).await;
    let permission = PermissionName::parse(&permission).expect("scenario permission must parse");
    let expected_scope = scenario_role_scope(&mut ctx.role).as_permission_scope();
    let response = ctx
        .role
        .last_permission_check
        .as_ref()
        .expect("permission check response must exist");
    assert_eq!(
        response.principal,
        (PrincipalRef::Account { account_id }),
        "permission check principal mismatch"
    );
    assert_eq!(
        &response.permission, &permission,
        "permission check permission mismatch"
    );
    assert_eq!(
        response.scope, expected_scope,
        "permission check scope mismatch"
    );
    assert!(response.allowed, "permission check must be allowed");

    let expected = ctx
        .role
        .grant_ids_by_alias
        .get(&alias)
        .and_then(|grants| grants.get(permission.as_str()))
        .copied()
        .expect("expected grant id for alias + permission");
    assert_eq!(
        matching_grant_id_set(&response.matching_grant_ids),
        matching_grant_id_set(&[expected]),
        "matching_grant_ids must exactly match expected direct grant id"
    );
}

#[then(expr = "the permission check has no matching grant ids")]
async fn then_permission_check_has_no_matching_grant_ids(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    let response = ctx
        .role
        .last_permission_check
        .as_ref()
        .expect("permission check response must exist");
    assert!(
        !response.allowed,
        "denied permission checks must report allowed=false"
    );
    assert!(
        response.matching_grant_ids.is_empty(),
        "denied permission checks must return an empty matching_grant_ids set"
    );
}

#[then(
    expr = "applying the role template to account principal {word} is idempotent for permissions {string}"
)]
async fn then_applying_role_is_idempotent(
    world: &mut TanrenWorld,
    alias: String,
    permissions: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let snapshots = ctx
        .role
        .apply_grant_snapshots
        .get(&alias)
        .cloned()
        .expect("apply snapshots missing for alias");
    assert!(
        snapshots.len() >= 2,
        "idempotency witness requires two apply observations for alias"
    );
    let previous = snapshots
        .get(snapshots.len() - 2)
        .expect("previous apply snapshot must exist");
    let latest = snapshots
        .last()
        .expect("latest apply snapshot must exist after apply");
    let expected_permissions = parse_permissions_csv(&permissions);
    let expected_set = permission_set(&expected_permissions);
    assert_eq!(
        permission_set_from_grant_map(previous),
        expected_set,
        "first apply snapshot permission set mismatch"
    );
    assert_eq!(
        permission_set_from_grant_map(latest),
        expected_set,
        "second apply snapshot permission set mismatch"
    );
    assert_eq!(
        latest, previous,
        "duplicate apply should return stable grant ids per permission"
    );

    let account_id = ensure_scenario_principal_account(ctx, alias).await;
    let principal = PrincipalRef::Account { account_id };
    let expected_scope = scenario_role_scope(&mut ctx.role).as_permission_scope();
    let grants = ctx
        .harness
        .read_direct_grants(principal)
        .await
        .expect("read direct grants should succeed");
    for grant in &grants {
        assert_eq!(grant.principal, principal, "grant principal mismatch");
        assert_eq!(grant.scope, expected_scope, "grant scope mismatch");
    }
    let current = crate::steps::role_support::permission_grant_ids_by_permission(&grants);
    assert_eq!(
        permission_set_from_grant_map(&current),
        expected_set,
        "direct-grant permission set mismatch after duplicate apply"
    );
    assert_eq!(
        current, *latest,
        "duplicate apply should keep one stable direct grant id per permission"
    );
}

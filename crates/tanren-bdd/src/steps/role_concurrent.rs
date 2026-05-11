//! Concurrent role-apply idempotency step definitions for B-0038.

use cucumber::{then, when};
use tanren_contract::ApplyRoleRequest;
use tanren_identity_policy::PrincipalRef;

use crate::TanrenWorld;
use crate::steps::role_support::{
    active_role, ensure_scenario_principal_account, parse_permissions_csv,
    permission_grant_ids_by_permission, permission_set, permission_set_from_grant_map,
    scenario_role_scope,
};

#[when(expr = "{int} concurrent applications apply the role template to account principal {word}")]
async fn when_concurrent_apply_role(world: &mut TanrenWorld, count: usize, alias: String) {
    let ctx = world.ensure_account_ctx().await;
    let role = active_role(&ctx.role);
    let grant_scope = scenario_role_scope(&mut ctx.role).as_permission_scope();
    let account_id = ensure_scenario_principal_account(ctx, alias.clone()).await;
    let principal = PrincipalRef::Account { account_id };
    let request = ApplyRoleRequest {
        role,
        principal,
        grant_scope,
    };
    let outcomes = ctx.harness.apply_role_concurrent(request, count).await;
    let mut all_succeeded = true;
    for (i, outcome) in outcomes.into_iter().enumerate() {
        match outcome {
            Ok(response) => {
                assert_eq!(response.role, role, "concurrent apply {i} role mismatch");
            }
            Err(_) => {
                all_succeeded = false;
            }
        }
    }
    assert!(
        all_succeeded,
        "all concurrent apply-role calls must succeed"
    );
    let grants = ctx
        .harness
        .read_direct_grants(PrincipalRef::Account { account_id })
        .await
        .expect("read direct grants should succeed after concurrent apply");
    let grant_ids = permission_grant_ids_by_permission(&grants);
    ctx.role
        .grant_ids_by_alias
        .insert(alias.clone(), grant_ids.clone());
    ctx.role
        .apply_grant_snapshots
        .entry(alias)
        .or_default()
        .push(grant_ids);
    ctx.role.last_error_code = None;
    ctx.role.last_permission_check = None;
}

#[then(
    expr = "exactly one direct grant per permission exists for account principal {word} with permissions {string}"
)]
async fn then_exactly_one_grant_per_permission(
    world: &mut TanrenWorld,
    alias: String,
    permissions: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let account_id = ensure_scenario_principal_account(ctx, alias.clone()).await;
    let principal = PrincipalRef::Account { account_id };
    let expected_permissions = parse_permissions_csv(&permissions);
    let grants = ctx
        .harness
        .read_direct_grants(principal)
        .await
        .expect("read direct grants should succeed");
    assert_eq!(
        grants.len(),
        expected_permissions.len(),
        "concurrent apply must produce exactly one grant per permission, got {} grants for {} permissions",
        grants.len(),
        expected_permissions.len()
    );
    let actual_set = permission_set(
        &grants
            .iter()
            .map(|g| g.permission.clone())
            .collect::<Vec<_>>(),
    );
    let expected_set = permission_set(&expected_permissions);
    assert_eq!(
        actual_set, expected_set,
        "grant permission set mismatch after concurrent apply"
    );
}

#[then(
    expr = "the direct grant ids for account principal {word} match across {int} repeated observations of permissions {string}"
)]
async fn then_grant_ids_stable_across_observations(
    world: &mut TanrenWorld,
    alias: String,
    observation_count: usize,
    permissions: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let account_id = ensure_scenario_principal_account(ctx, alias.clone()).await;
    let principal = PrincipalRef::Account { account_id };
    let expected_permissions = parse_permissions_csv(&permissions);
    let expected_set = permission_set(&expected_permissions);
    let initial = ctx
        .role
        .grant_ids_by_alias
        .get(&alias)
        .cloned()
        .expect("initial grant ids must be recorded from concurrent apply");
    for obs in 1..=observation_count {
        let grants = ctx
            .harness
            .read_direct_grants(principal)
            .await
            .expect("read direct grants should succeed");
        let current = permission_grant_ids_by_permission(&grants);
        assert_eq!(
            permission_set_from_grant_map(&current),
            expected_set,
            "observation {obs} permission set mismatch"
        );
        assert_eq!(
            current, initial,
            "observation {obs} grant ids must match initial concurrent-apply snapshot"
        );
    }
}

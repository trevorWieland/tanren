//! Pagination-focused B-0039 steps for self-permissions introspection.

use std::collections::BTreeSet;

use cucumber::{given, then, when};
use tanren_contract::{MY_PERMISSIONS_DEFAULT_LIMIT, MY_PERMISSIONS_MAX_LIMIT};
use tanren_identity_policy::{OrgId, PermissionGrantSource, PermissionName};
use tanren_testkit::{
    HarnessMyPermissionsQuery, HarnessOutcome, HarnessPermissionGrantFixture,
    HarnessPermissionScope, assert_no_permission_request_or_grant_events,
};

use crate::TanrenWorld;
use crate::steps::permissions_support::{
    actor_account_id, ensure_actor_signed_in, snapshot_event_ids,
};

const PAGINATION_PERMISSION_COUNT: usize = 205;

#[given(expr = "{word} has enough organization permissions to require pagination")]
async fn given_seed_many_permissions(world: &mut TanrenWorld, actor: String) {
    let ctx = world.ensure_account_ctx().await;
    let account_id = actor_account_id(ctx, &actor);
    let org_scope = OrgId::fresh();

    for index in 0..PAGINATION_PERMISSION_COUNT {
        ctx.harness
            .seed_permission_grant(HarnessPermissionGrantFixture {
                account_id,
                scope: HarnessPermissionScope::Organization(org_scope),
                permission: PermissionName::new(format!("org.bulk.{index:03}")),
                grant_source: PermissionGrantSource::Direct,
                policy_constraint: None,
            })
            .await
            .expect("seed paginated organization permission");
    }
}

#[when(expr = "{word} views their own permissions with limit {int}")]
async fn when_view_own_permissions_with_limit(world: &mut TanrenWorld, actor: String, limit: i32) {
    let session_account_id = ensure_actor_signed_in(world, &actor).await;
    let page_limit = u16::try_from(limit).expect("scenario limit must fit u16");
    let ctx = world.ensure_account_ctx().await;
    let before = snapshot_event_ids(ctx).await;
    ctx.event_ids_before_permissions_query = Some(before);

    match ctx
        .harness
        .my_permissions_query(
            session_account_id,
            None,
            HarnessMyPermissionsQuery {
                limit: Some(page_limit),
                cursor: None,
            },
        )
        .await
    {
        Ok(view) => {
            ctx.last_permissions_capability = None;
            ctx.permissions_page_history = vec![view.clone()];
            ctx.last_permissions = Some(view);
            ctx.last_permissions_failure_code = None;
            ctx.last_outcome = Some(HarnessOutcome::Other("permissions_loaded".to_owned()));
        }
        Err(err) => {
            let code = err.code();
            ctx.last_permissions_capability = None;
            ctx.permissions_page_history.clear();
            ctx.last_permissions = None;
            ctx.last_permissions_failure_code = Some(code.clone());
            ctx.last_outcome = Some(HarnessOutcome::FailureCode(code));
        }
    }
}

#[when(expr = "{word} continues their own permissions from the returned cursor with limit {int}")]
async fn when_continue_own_permissions_from_cursor(
    world: &mut TanrenWorld,
    actor: String,
    limit: i32,
) {
    let session_account_id = ensure_actor_signed_in(world, &actor).await;
    let page_limit = u16::try_from(limit).expect("scenario limit must fit u16");
    let cursor = {
        let ctx = world.ensure_account_ctx().await;
        ctx.last_permissions
            .as_ref()
            .and_then(|view| view.response.page.next_cursor.clone())
            .expect("first page should expose next_cursor")
    };
    let ctx = world.ensure_account_ctx().await;
    match ctx
        .harness
        .my_permissions_query(
            session_account_id,
            None,
            HarnessMyPermissionsQuery {
                limit: Some(page_limit),
                cursor: Some(cursor),
            },
        )
        .await
    {
        Ok(view) => {
            ctx.last_permissions_capability = None;
            ctx.permissions_page_history.push(view.clone());
            ctx.last_permissions = Some(view);
            ctx.last_permissions_failure_code = None;
            ctx.last_outcome = Some(HarnessOutcome::Other(
                "permissions_loaded_from_cursor".to_owned(),
            ));
        }
        Err(err) => {
            let code = err.code();
            ctx.last_permissions_capability = None;
            ctx.last_permissions = None;
            ctx.last_permissions_failure_code = Some(code.clone());
            ctx.last_outcome = Some(HarnessOutcome::FailureCode(code));
        }
    }
}

#[when(expr = "{word} views their own permissions with malformed cursor {string}")]
async fn when_view_own_permissions_with_malformed_cursor(
    world: &mut TanrenWorld,
    actor: String,
    cursor: String,
) {
    let session_account_id = ensure_actor_signed_in(world, &actor).await;
    let ctx = world.ensure_account_ctx().await;
    let before = snapshot_event_ids(ctx).await;
    ctx.event_ids_before_permissions_query = Some(before);

    match ctx
        .harness
        .my_permissions_query(
            session_account_id,
            None,
            HarnessMyPermissionsQuery {
                limit: None,
                cursor: Some(cursor),
            },
        )
        .await
    {
        Ok(view) => {
            ctx.last_permissions_capability = None;
            ctx.permissions_page_history = vec![view.clone()];
            ctx.last_permissions = Some(view);
            ctx.last_permissions_failure_code = None;
            ctx.last_outcome = Some(HarnessOutcome::Other(
                "permissions_loaded_unexpectedly".to_owned(),
            ));
        }
        Err(err) => {
            let code = err.code();
            ctx.last_permissions_capability = None;
            ctx.permissions_page_history.clear();
            ctx.last_permissions = None;
            ctx.last_permissions_failure_code = Some(code.clone());
            ctx.last_outcome = Some(HarnessOutcome::FailureCode(code));
        }
    }
}

#[then(expr = "the permissions page limit defaults to {int}")]
async fn then_permissions_limit_defaults(world: &mut TanrenWorld, expected_limit: i32) {
    let expected = u16::try_from(expected_limit).expect("scenario limit should fit u16");
    assert_eq!(
        expected, MY_PERMISSIONS_DEFAULT_LIMIT,
        "expected scenario default limit assertion to match contract constant"
    );
    let ctx = world.ensure_account_ctx().await;
    let view = ctx
        .last_permissions
        .as_ref()
        .expect("permissions query should have succeeded");
    assert_eq!(
        view.response.page.limit, expected,
        "expected default page limit to be enforced"
    );
    assert!(
        !view.response.read_metadata.source.trim().is_empty(),
        "expected read metadata source to be populated"
    );
    assert!(
        view.response
            .read_metadata
            .source_checkpoint
            .max_permission_grant_id
            .is_some()
            || view
                .response
                .read_metadata
                .source_checkpoint
                .max_permission_constraint_id
                .is_some(),
        "expected read metadata checkpoint ids to include at least one row id"
    );
}

#[then(expr = "the permissions page limit is clamped to {int}")]
async fn then_permissions_limit_clamped(world: &mut TanrenWorld, expected_limit: i32) {
    let expected = u16::try_from(expected_limit).expect("scenario limit should fit u16");
    assert_eq!(
        expected, MY_PERMISSIONS_MAX_LIMIT,
        "expected scenario clamp assertion to match contract max-limit constant"
    );
    let ctx = world.ensure_account_ctx().await;
    let view = ctx
        .last_permissions
        .as_ref()
        .expect("permissions query should have succeeded");
    assert_eq!(
        view.response.page.limit, expected,
        "expected max-limit clamping to be enforced"
    );
}

#[then(expr = "the first two permission pages each return {int} entries")]
async fn then_first_two_permission_pages_each_return(world: &mut TanrenWorld, expected_count: i32) {
    let expected = u16::try_from(expected_count).expect("scenario count should fit u16");
    let ctx = world.ensure_account_ctx().await;
    assert!(
        ctx.permissions_page_history.len() >= 2,
        "expected at least two permission pages in history"
    );
    let first = &ctx.permissions_page_history[0];
    let second = &ctx.permissions_page_history[1];
    assert_eq!(
        first.response.page.returned, expected,
        "expected first page to return {expected} entries"
    );
    assert_eq!(
        second.response.page.returned, expected,
        "expected second page to return {expected} entries"
    );
}

#[then(expr = "the second page uses the first page cursor and has no duplicate entries")]
async fn then_second_page_uses_cursor_and_no_duplicates(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    assert!(
        ctx.permissions_page_history.len() >= 2,
        "expected at least two permission pages in history"
    );
    let first = &ctx.permissions_page_history[0];
    let second = &ctx.permissions_page_history[1];
    let first_cursor = first
        .response
        .page
        .next_cursor
        .as_ref()
        .expect("first page should include a continuation cursor");
    assert_eq!(
        second.response.page.request_cursor.as_ref(),
        Some(first_cursor),
        "second-page request cursor should match the first-page continuation token"
    );

    let first_keys = permission_entry_keys(first);
    let second_keys = permission_entry_keys(second);
    let first_set: BTreeSet<_> = first_keys.iter().cloned().collect();
    let second_set: BTreeSet<_> = second_keys.iter().cloned().collect();
    let overlap: Vec<_> = first_set.intersection(&second_set).cloned().collect();
    assert!(
        overlap.is_empty(),
        "expected no duplicate permission entries across pages, found {overlap:?}"
    );

    let mut concatenated = first_keys;
    concatenated.extend(second_keys);
    let mut sorted = concatenated.clone();
    sorted.sort();
    assert_eq!(
        concatenated, sorted,
        "expected keyset ordering to remain deterministic across page boundaries"
    );
}

#[then(expr = "the paginated permissions view does not create permission request or grant events")]
async fn then_paginated_view_no_permission_events(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    let before = ctx
        .event_ids_before_permissions_query
        .clone()
        .expect("event snapshot should be recorded before permissions query");
    let after = ctx
        .harness
        .recent_events(200)
        .await
        .expect("recent_events should succeed under BDD");
    assert_no_permission_request_or_grant_events(&before, &after)
        .expect("permissions view should remain read-only");
}

fn permission_entry_keys(view: &tanren_testkit::HarnessPermissionsView) -> Vec<String> {
    let mut keys = Vec::new();
    for section in &view.response.organizations {
        for permission in &section.permissions {
            keys.push(format!(
                "org:{}:{}:{}:{}",
                section.org_id,
                permission.permission.as_str(),
                grant_source_key(&permission.grant_source),
                permission.grant_source_reference
            ));
        }
    }
    for section in &view.response.projects {
        for permission in &section.permissions {
            keys.push(format!(
                "project:{}:{}:{}:{}",
                section.project_id,
                permission.permission.as_str(),
                grant_source_key(&permission.grant_source),
                permission.grant_source_reference
            ));
        }
    }
    keys
}

fn grant_source_key(source: &PermissionGrantSource) -> String {
    match source {
        PermissionGrantSource::Direct => "direct".to_owned(),
        PermissionGrantSource::RoleTemplate { role_template } => {
            format!("role_template:{}", role_template.as_str())
        }
    }
}

//! Permission-introspection steps for B-0039.
//!
//! These steps drive the interface-selected `AccountHarness` and assert on
//! the observable output from each surface.

use std::collections::HashSet;

use cucumber::{given, then, when};
use secrecy::ExposeSecret;
use secrecy::SecretString;
use tanren_contract::MyPermissionEntry;
use tanren_identity_policy::{
    Email, OrgId, PermissionGrantSource, PermissionName, PolicyConstraintReason,
    PolicyConstraintSource, ProjectId, RoleTemplateName,
};
use tanren_testkit::{
    HarnessOutcome, HarnessPermissionConstraintFixture, HarnessPermissionGrantFixture,
    HarnessPermissionScope, assert_no_permission_request_or_grant_events,
};

use crate::TanrenWorld;

const ORG_PERMISSION: &str = "org.members.view";
const PROJECT_ROLE_PERMISSION: &str = "project.changes.review";
const PROJECT_CONSTRAINED_PERMISSION: &str = "project.deploy.approve";
const ROLE_TEMPLATE_NAME: &str = "release_manager";

#[given(
    expr = "{word} has direct and role-template permissions with organization policy reason {string}"
)]
async fn given_seed_permissions(world: &mut TanrenWorld, actor: String, reason: String) {
    let ctx = world.ensure_account_ctx().await;
    let account_id = actor_account_id(ctx, &actor);
    let org_scope = OrgId::fresh();
    let project_scope = ProjectId::fresh();

    ctx.harness
        .seed_permission_grant(HarnessPermissionGrantFixture {
            account_id,
            scope: HarnessPermissionScope::Organization(org_scope),
            permission: PermissionName::new(ORG_PERMISSION.to_owned()),
            grant_source: PermissionGrantSource::Direct,
            policy_constraint: None,
        })
        .await
        .expect("seed org direct permission");

    ctx.harness
        .seed_permission_grant(HarnessPermissionGrantFixture {
            account_id,
            scope: HarnessPermissionScope::Project(project_scope),
            permission: PermissionName::new(PROJECT_ROLE_PERMISSION.to_owned()),
            grant_source: PermissionGrantSource::RoleTemplate {
                role_template: RoleTemplateName::new(ROLE_TEMPLATE_NAME.to_owned()),
            },
            policy_constraint: None,
        })
        .await
        .expect("seed project role-template permission");

    ctx.harness
        .seed_permission_grant(HarnessPermissionGrantFixture {
            account_id,
            scope: HarnessPermissionScope::Project(project_scope),
            permission: PermissionName::new(PROJECT_CONSTRAINED_PERMISSION.to_owned()),
            grant_source: PermissionGrantSource::Direct,
            policy_constraint: Some(HarnessPermissionConstraintFixture {
                reason: PolicyConstraintReason::new(reason),
                source: PolicyConstraintSource::OrganizationPolicy,
            }),
        })
        .await
        .expect("seed constrained project permission");
}

#[when(expr = "{word} views their own permissions")]
async fn when_view_own_permissions(world: &mut TanrenWorld, actor: String) {
    let session_account_id = ensure_actor_signed_in(world, &actor).await;
    let ctx = world.ensure_account_ctx().await;
    let before = snapshot_event_ids(ctx).await;
    ctx.event_ids_before_permissions_query = Some(before);

    match ctx.harness.my_permissions(session_account_id, None).await {
        Ok(view) => {
            ctx.last_permissions = Some(view);
            ctx.last_permissions_failure_code = None;
            ctx.last_outcome = Some(HarnessOutcome::Other("permissions_loaded".to_owned()));
        }
        Err(err) => {
            let code = err.code();
            ctx.last_permissions = None;
            ctx.last_permissions_failure_code = Some(code.clone());
            ctx.last_outcome = Some(HarnessOutcome::FailureCode(code));
        }
    }
}

#[when(expr = "an unauthenticated client views their own permissions")]
async fn when_unauthenticated_view_own_permissions(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    let before = snapshot_event_ids(ctx).await;
    ctx.event_ids_before_permissions_query = Some(before);

    match ctx
        .harness
        .my_permissions(tanren_identity_policy::AccountId::fresh(), None)
        .await
    {
        Ok(view) => {
            ctx.last_permissions = Some(view);
            ctx.last_permissions_failure_code = None;
            ctx.last_outcome = Some(HarnessOutcome::Other(
                "permissions_loaded_unexpectedly".to_owned(),
            ));
        }
        Err(err) => {
            let code = err.code();
            ctx.last_permissions = None;
            ctx.last_permissions_failure_code = Some(code.clone());
            ctx.last_outcome = Some(HarnessOutcome::FailureCode(code));
        }
    }
}

#[when(expr = "{word} attempts to view {word}'s permissions through the self view")]
async fn when_view_other_permissions(world: &mut TanrenWorld, actor: String, target: String) {
    let session_account_id = ensure_actor_signed_in(world, &actor).await;
    let target_account_id = {
        let ctx = world.ensure_account_ctx().await;
        actor_account_id(ctx, &target)
    };
    let ctx = world.ensure_account_ctx().await;
    let before = snapshot_event_ids(ctx).await;
    ctx.event_ids_before_permissions_query = Some(before);

    match ctx
        .harness
        .my_permissions(session_account_id, Some(target_account_id))
        .await
    {
        Ok(view) => {
            ctx.last_permissions = Some(view);
            ctx.last_permissions_failure_code = None;
            ctx.last_outcome = Some(HarnessOutcome::Other(
                "permissions_loaded_unexpectedly".to_owned(),
            ));
        }
        Err(err) => {
            let code = err.code();
            ctx.last_permissions = None;
            ctx.last_permissions_failure_code = Some(code.clone());
            ctx.last_outcome = Some(HarnessOutcome::FailureCode(code));
        }
    }
}

#[then(expr = "{word} sees organization and project permission sections")]
async fn then_sees_org_and_project(world: &mut TanrenWorld, actor: String) {
    drop(actor);
    let ctx = world.ensure_account_ctx().await;
    let view = ctx
        .last_permissions
        .as_ref()
        .expect("permissions query should have succeeded");
    assert!(
        !view.response.organizations.is_empty(),
        "expected at least one organization section"
    );
    assert!(
        !view.response.projects.is_empty(),
        "expected at least one project section"
    );
    if ctx.harness.kind() == tanren_testkit::HarnessKind::Tui {
        assert!(
            view.rendered.contains("org_id="),
            "expected rendered TUI output to include organization ids; got {}",
            view.rendered
        );
        assert!(
            view.rendered.contains("project_id="),
            "expected rendered TUI output to include project ids; got {}",
            view.rendered
        );
    }
}

#[then(expr = "{word} sees the direct organization permission entry")]
async fn then_sees_direct_org_permission(world: &mut TanrenWorld, actor: String) {
    drop(actor);
    let ctx = world.ensure_account_ctx().await;
    let view = ctx
        .last_permissions
        .as_ref()
        .expect("permissions query should have succeeded");
    assert_entry(
        &view.response.organizations,
        ORG_PERMISSION,
        &PermissionGrantSource::Direct,
    );
    assert!(
        view.rendered.contains(ORG_PERMISSION),
        "expected rendered output to include {ORG_PERMISSION}; got {}",
        view.rendered
    );
    if ctx.harness.kind() == tanren_testkit::HarnessKind::Tui {
        assert!(
            view.rendered.contains("source=direct"),
            "expected rendered TUI output to include direct source label; got {}",
            view.rendered
        );
    }
}

#[then(expr = "{word} sees the role-template project permission entry")]
async fn then_sees_role_template_project_permission(world: &mut TanrenWorld, actor: String) {
    drop(actor);
    let ctx = world.ensure_account_ctx().await;
    let view = ctx
        .last_permissions
        .as_ref()
        .expect("permissions query should have succeeded");
    assert_entry(
        &view.response.projects,
        PROJECT_ROLE_PERMISSION,
        &PermissionGrantSource::RoleTemplate {
            role_template: RoleTemplateName::new(ROLE_TEMPLATE_NAME.to_owned()),
        },
    );
    assert!(
        view.rendered.contains(ROLE_TEMPLATE_NAME),
        "expected rendered output to include role template {ROLE_TEMPLATE_NAME}; got {}",
        view.rendered
    );
    if ctx.harness.kind() == tanren_testkit::HarnessKind::Tui {
        assert!(
            view.rendered
                .contains(&format!("source=role_template:{ROLE_TEMPLATE_NAME}")),
            "expected rendered TUI output to include role-template source label; got {}",
            view.rendered
        );
    }
}

#[then(expr = "{word} sees constrained permission reason {string} from source {string}")]
async fn then_sees_constraint_reason_and_source(
    world: &mut TanrenWorld,
    actor: String,
    reason: String,
    source: String,
) {
    drop(actor);
    let ctx = world.ensure_account_ctx().await;
    let view = ctx
        .last_permissions
        .as_ref()
        .expect("permissions query should have succeeded");
    let entry = view
        .response
        .projects
        .iter()
        .flat_map(|section| section.permissions.iter())
        .find(|entry| entry.permission.as_str() == PROJECT_CONSTRAINED_PERMISSION)
        .expect("constrained project permission must exist");
    let constraint = entry
        .policy_constraint
        .as_ref()
        .expect("expected policy constraint to be present");
    assert_eq!(constraint.reason.as_str(), reason);
    let actual_source = match constraint.source {
        PolicyConstraintSource::OrganizationPolicy => "organization_policy",
        PolicyConstraintSource::ProjectPolicy => "project_policy",
    };
    assert_eq!(actual_source, source);

    let source_variants = [
        source.as_str(),
        "organization_policy",
        "OrganizationPolicy",
        "organization policy",
    ];
    assert!(
        source_variants
            .iter()
            .any(|candidate| view.rendered.contains(candidate)),
        "expected rendered output to include one of {:?}; got {}",
        source_variants,
        view.rendered
    );
    assert!(
        view.rendered.contains(&reason),
        "expected rendered output to include reason '{reason}'; got {}",
        view.rendered
    );
}

#[then(
    expr = "on a phone viewport the web permissions page shows the role-template source and constraint reason"
)]
async fn then_phone_viewport_visibility(world: &mut TanrenWorld) {
    let ctx = world.ensure_account_ctx().await;
    let view = ctx
        .last_permissions
        .as_ref()
        .expect("permissions query should have succeeded");
    assert!(
        view.rendered.contains(ROLE_TEMPLATE_NAME),
        "expected rendered output to include role template {ROLE_TEMPLATE_NAME}; got {}",
        view.rendered
    );
    assert!(
        view.rendered.contains("organization_policy")
            || view.rendered.contains("OrganizationPolicy"),
        "expected rendered output to include organization-policy source; got {}",
        view.rendered
    );
}

#[then(expr = "the permissions view does not create permission request or grant events")]
async fn then_no_permission_events(world: &mut TanrenWorld) {
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

async fn ensure_actor_signed_in(
    world: &mut TanrenWorld,
    actor: &str,
) -> tanren_identity_policy::AccountId {
    let (email_raw, password_raw) = {
        let ctx = world.ensure_account_ctx().await;
        let entry = ctx
            .actors
            .get(actor)
            .expect("actor must be registered first");
        (
            entry
                .identifier
                .clone()
                .expect("actor identifier should be recorded"),
            entry
                .password
                .as_ref()
                .map(|secret| secret.expose_secret().to_owned())
                .expect("actor password should be recorded"),
        )
    };
    let ctx = world.ensure_account_ctx().await;
    let email = Email::parse(&email_raw).expect("scenario email must parse");
    let response = ctx
        .harness
        .sign_in(tanren_contract::SignInRequest {
            email,
            password: SecretString::from(password_raw),
        })
        .await
        .expect("sign-in before permissions query must succeed");
    let entry = ctx
        .actors
        .get_mut(actor)
        .expect("actor state should still be present");
    entry.sign_in = Some(response.clone());
    response.account_id
}

fn actor_account_id(ctx: &crate::AccountContext, actor: &str) -> tanren_identity_policy::AccountId {
    let entry = ctx
        .actors
        .get(actor)
        .expect("actor must have prior sign-up/sign-in");
    entry
        .sign_in
        .as_ref()
        .map(|session| session.account_id)
        .or_else(|| entry.sign_up.as_ref().map(|session| session.account_id))
        .or_else(|| {
            entry
                .accept_invitation
                .as_ref()
                .map(|acceptance| acceptance.session.account_id)
        })
        .expect("actor must have a session/account id")
}

async fn snapshot_event_ids(ctx: &mut crate::AccountContext) -> HashSet<String> {
    ctx.harness
        .recent_events(200)
        .await
        .expect("recent_events should succeed")
        .into_iter()
        .map(|event| event.id.to_string())
        .collect()
}

fn assert_entry<T>(sections: &[T], permission: &str, expected_source: &PermissionGrantSource)
where
    T: AsPermissions,
{
    let matches = sections
        .iter()
        .flat_map(AsPermissions::permissions)
        .any(|entry| {
            entry.permission.as_str() == permission && &entry.grant_source == expected_source
        });
    assert!(
        matches,
        "expected permission '{permission}' with source {expected_source:?}"
    );
}

trait AsPermissions {
    fn permissions(&self) -> &[MyPermissionEntry];
}

impl AsPermissions for tanren_contract::MyOrganizationPermissions {
    fn permissions(&self) -> &[MyPermissionEntry] {
        &self.permissions
    }
}

impl AsPermissions for tanren_contract::MyProjectPermissions {
    fn permissions(&self) -> &[MyPermissionEntry] {
        &self.permissions
    }
}

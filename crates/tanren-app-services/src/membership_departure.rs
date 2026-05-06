//! Membership departure handlers: voluntary leave and admin-initiated
//! removal.
//!
//! Both flows share the same lifecycle event ([`MembershipDeparted`])
//! and the 'last administrative-permission holder cannot depart'
//! invariant. In-flight work is surfaced to the actor before the action
//! completes so nothing is silently orphaned. Both flows are attributed
//! and appear in the org's permission change history.

use chrono::{DateTime, Utc};
use tanren_contract::{
    AccountFailureReason, InFlightWorkItem, LeaveOrganizationRequest, MembershipDepartureResponse,
    OrgMembershipView, RemoveMemberRequest,
};
use tanren_identity_policy::{AccountId, OrgId, OrgPermissions};
use tanren_store::{
    AccountStore, DepartMemberAtomicRequest, DepartMemberError, DepartMemberEventContext,
    MembershipRecord,
};

use crate::events::{
    AccountEventKind, DepartureFailed, DepartureMode, MemberRemovedNotification,
    MembershipDeparted, envelope,
};
use crate::{AppServiceError, Clock};

pub(crate) async fn leave_organization<S>(
    store: &S,
    clock: &Clock,
    account_id: AccountId,
    request: LeaveOrganizationRequest,
    acknowledge_in_flight_work: bool,
) -> Result<MembershipDepartureResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();
    let org_id = request.org_id;

    let memberships = store.list_memberships_for_account(account_id).await?;

    let Some(membership) = find_membership_for_org(&memberships, org_id) else {
        emit_departure_failed(
            store,
            AccountFailureReason::NotOrgMember,
            account_id,
            account_id,
            org_id,
            now,
        )
        .await?;
        return Err(AppServiceError::Account(AccountFailureReason::NotOrgMember));
    };

    require_not_last_admin(store, membership, account_id, account_id, now).await?;

    let in_flight = store
        .list_in_flight_work_for_member(account_id, org_id)
        .await?;
    if !in_flight.is_empty() && !acknowledge_in_flight_work {
        return Ok(MembershipDepartureResponse {
            completed: false,
            in_flight_work: in_flight.into_iter().map(|_| InFlightWorkItem {}).collect(),
            departed_org: None,
            selectable_organizations: build_selectable_orgs_excluding(&memberships, org_id),
        });
    }

    match store
        .depart_member_atomic(DepartMemberAtomicRequest {
            membership_id: membership.id,
            account_id,
            org_id,
            now,
            events_builder: Box::new(|ctx: &DepartMemberEventContext| {
                vec![envelope(
                    AccountEventKind::MembershipDeparted,
                    &MembershipDeparted {
                        actor: ctx.account_id,
                        target: ctx.account_id,
                        org: ctx.org_id,
                        mode: DepartureMode::Leave,
                        at: ctx.now,
                    },
                )]
            }),
        })
        .await
    {
        Ok(_) => {}
        Err(DepartMemberError::MembershipNotFound) => {
            emit_departure_failed(
                store,
                AccountFailureReason::NotOrgMember,
                account_id,
                account_id,
                org_id,
                now,
            )
            .await?;
            return Err(AppServiceError::Account(AccountFailureReason::NotOrgMember));
        }
        Err(DepartMemberError::Store(e)) => return Err(AppServiceError::Store(e)),
    }

    let remaining = store.list_memberships_for_account(account_id).await?;
    Ok(MembershipDepartureResponse {
        completed: true,
        in_flight_work: vec![],
        departed_org: Some(org_id),
        selectable_organizations: build_selectable_orgs(&remaining),
    })
}

pub(crate) async fn remove_member<S>(
    store: &S,
    clock: &Clock,
    actor_account_id: AccountId,
    request: RemoveMemberRequest,
    acknowledge_in_flight_work: bool,
) -> Result<MembershipDepartureResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();
    let org_id = request.org_id;
    let target_account_id = request.member_account_id;

    require_admin(store, actor_account_id, target_account_id, org_id, now).await?;

    let target_memberships = store
        .list_memberships_for_account(target_account_id)
        .await?;
    let Some(target_membership) = find_membership_for_org(&target_memberships, org_id) else {
        emit_departure_failed(
            store,
            AccountFailureReason::NotOrgMember,
            actor_account_id,
            target_account_id,
            org_id,
            now,
        )
        .await?;
        return Err(AppServiceError::Account(AccountFailureReason::NotOrgMember));
    };

    require_not_last_admin(
        store,
        target_membership,
        actor_account_id,
        target_account_id,
        now,
    )
    .await?;

    let in_flight = store
        .list_in_flight_work_for_member(target_account_id, org_id)
        .await?;
    if !in_flight.is_empty() && !acknowledge_in_flight_work {
        return Ok(MembershipDepartureResponse {
            completed: false,
            in_flight_work: in_flight.into_iter().map(|_| InFlightWorkItem {}).collect(),
            departed_org: None,
            selectable_organizations: build_selectable_orgs_excluding(&target_memberships, org_id),
        });
    }

    let actor_id = actor_account_id;
    match store
        .depart_member_atomic(DepartMemberAtomicRequest {
            membership_id: target_membership.id,
            account_id: target_account_id,
            org_id,
            now,
            events_builder: Box::new(move |ctx: &DepartMemberEventContext| {
                vec![
                    envelope(
                        AccountEventKind::MembershipDeparted,
                        &MembershipDeparted {
                            actor: actor_id,
                            target: ctx.account_id,
                            org: ctx.org_id,
                            mode: DepartureMode::Remove,
                            at: ctx.now,
                        },
                    ),
                    envelope(
                        AccountEventKind::MemberRemovedNotification,
                        &MemberRemovedNotification {
                            removed_account: ctx.account_id,
                            org: ctx.org_id,
                            removed_by: actor_id,
                            at: ctx.now,
                        },
                    ),
                ]
            }),
        })
        .await
    {
        Ok(_) => {}
        Err(DepartMemberError::MembershipNotFound) => {
            emit_departure_failed(
                store,
                AccountFailureReason::NotOrgMember,
                actor_account_id,
                target_account_id,
                org_id,
                now,
            )
            .await?;
            return Err(AppServiceError::Account(AccountFailureReason::NotOrgMember));
        }
        Err(DepartMemberError::Store(e)) => return Err(AppServiceError::Store(e)),
    }

    let remaining = store
        .list_memberships_for_account(target_account_id)
        .await?;
    Ok(MembershipDepartureResponse {
        completed: true,
        in_flight_work: vec![],
        departed_org: Some(org_id),
        selectable_organizations: build_selectable_orgs(&remaining),
    })
}

async fn require_admin<S>(
    store: &S,
    actor_account_id: AccountId,
    target_account_id: AccountId,
    org_id: OrgId,
    now: DateTime<Utc>,
) -> Result<(), AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let actor_memberships = store.list_memberships_for_account(actor_account_id).await?;
    let Some(actor_membership) = find_membership_for_org(&actor_memberships, org_id) else {
        emit_departure_failed(
            store,
            AccountFailureReason::PermissionDenied,
            actor_account_id,
            target_account_id,
            org_id,
            now,
        )
        .await?;
        return Err(AppServiceError::Account(
            AccountFailureReason::PermissionDenied,
        ));
    };
    if !is_admin_membership(actor_membership) {
        emit_departure_failed(
            store,
            AccountFailureReason::PermissionDenied,
            actor_account_id,
            target_account_id,
            org_id,
            now,
        )
        .await?;
        return Err(AppServiceError::Account(
            AccountFailureReason::PermissionDenied,
        ));
    }
    Ok(())
}

async fn require_not_last_admin<S>(
    store: &S,
    membership: &MembershipRecord,
    actor: AccountId,
    target: AccountId,
    now: DateTime<Utc>,
) -> Result<(), AppServiceError>
where
    S: AccountStore + ?Sized,
{
    if !is_admin_membership(membership) {
        return Ok(());
    }
    let admin_count = store
        .count_admin_memberships_for_org(membership.org_id)
        .await?;
    if admin_count <= 1 {
        emit_departure_failed(
            store,
            AccountFailureReason::LastAdminPermissionHolder,
            actor,
            target,
            membership.org_id,
            now,
        )
        .await?;
        return Err(AppServiceError::Account(
            AccountFailureReason::LastAdminPermissionHolder,
        ));
    }
    Ok(())
}

fn find_membership_for_org(
    memberships: &[MembershipRecord],
    org_id: OrgId,
) -> Option<&MembershipRecord> {
    memberships.iter().find(|m| m.org_id == org_id)
}

fn is_admin_membership(membership: &MembershipRecord) -> bool {
    membership
        .org_permissions
        .as_ref()
        .is_some_and(OrgPermissions::is_administrative)
}

fn build_selectable_orgs(memberships: &[MembershipRecord]) -> Vec<OrgMembershipView> {
    memberships
        .iter()
        .map(|m| OrgMembershipView {
            org_id: m.org_id,
            permissions: m
                .org_permissions
                .clone()
                .unwrap_or_else(OrgPermissions::member),
        })
        .collect()
}

fn build_selectable_orgs_excluding(
    memberships: &[MembershipRecord],
    exclude_org: OrgId,
) -> Vec<OrgMembershipView> {
    memberships
        .iter()
        .filter(|m| m.org_id != exclude_org)
        .map(|m| OrgMembershipView {
            org_id: m.org_id,
            permissions: m
                .org_permissions
                .clone()
                .unwrap_or_else(OrgPermissions::member),
        })
        .collect()
}

async fn emit_departure_failed<S>(
    store: &S,
    reason: AccountFailureReason,
    actor: AccountId,
    target: AccountId,
    org: OrgId,
    now: DateTime<Utc>,
) -> Result<(), AppServiceError>
where
    S: AccountStore + ?Sized,
{
    store
        .append_event(
            envelope(
                AccountEventKind::DepartureFailed,
                &DepartureFailed {
                    reason,
                    actor,
                    target,
                    org,
                    at: now,
                },
            ),
            now,
        )
        .await?;
    Ok(())
}

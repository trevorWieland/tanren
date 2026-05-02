//! Account-flow handlers: sign-up, sign-in, accept-invitation.
//!
//! Handlers are mechanism-neutral at the contract surface but mechanism-
//! specific underneath: R-0001 pins identifier+password as the simplest
//! credible choice. Hashing uses sha-256 over `salt || password`; salt
//! and session token are 16-byte chunks of `Uuid::new_v4()`. The choice
//! is revisable behind the `tanren_identity_policy::CredentialVerifier`
//! trait without touching the wire shapes.

use sha2::{Digest, Sha256};
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason, AccountView,
    SessionView, SignInRequest, SignInResponse, SignUpRequest, SignUpResponse,
};
use tanren_store::{AccountRecord, NewAccount, Store};
use uuid::Uuid;

use crate::events::{AccountCreated, InvitationAccepted, SignedIn, envelope};
use crate::{AppServiceError, Clock};

pub(crate) async fn sign_up(
    store: &Store,
    clock: &Clock,
    request: SignUpRequest,
) -> Result<SignUpResponse, AppServiceError> {
    let identifier = normalize_identifier(&request.email);
    let display_name = request.display_name.trim().to_owned();
    if identifier.is_empty() || request.password.is_empty() || display_name.is_empty() {
        return Err(AppServiceError::Account(
            AccountFailureReason::InvalidCredential,
        ));
    }

    if store
        .find_account_by_identifier(&identifier)
        .await?
        .is_some()
    {
        return Err(AppServiceError::Account(
            AccountFailureReason::DuplicateIdentifier,
        ));
    }

    let now = clock.now();
    let salt = random_bytes();
    let password_hash = hash_password(&salt, &request.password);
    let id = Uuid::now_v7();
    let account = store
        .insert_account(NewAccount {
            id,
            identifier,
            display_name,
            password_hash,
            password_salt: salt,
            created_at: now,
            org_id: None,
        })
        .await
        .map_err(map_insert_error)?;

    let session = mint_session(store, account.id).await?;
    store
        .append_event(envelope(
            "account_created",
            &AccountCreated {
                account_id: account.id,
                identifier: account.identifier.clone(),
                org: None,
                created_at: now,
            },
        ))
        .await?;

    Ok(SignUpResponse {
        account: account_view(&account),
        session,
    })
}

pub(crate) async fn sign_in(
    store: &Store,
    clock: &Clock,
    request: SignInRequest,
) -> Result<SignInResponse, AppServiceError> {
    let identifier = normalize_identifier(&request.email);
    if identifier.is_empty() || request.password.is_empty() {
        return Err(AppServiceError::Account(
            AccountFailureReason::InvalidCredential,
        ));
    }
    let Some(account) = store.find_account_by_identifier(&identifier).await? else {
        return Err(AppServiceError::Account(
            AccountFailureReason::InvalidCredential,
        ));
    };
    if !verify_password(
        &account.password_salt,
        &account.password_hash,
        &request.password,
    ) {
        return Err(AppServiceError::Account(
            AccountFailureReason::InvalidCredential,
        ));
    }

    let session = mint_session(store, account.id).await?;
    store
        .append_event(envelope(
            "signed_in",
            &SignedIn {
                account_id: account.id,
                at: clock.now(),
            },
        ))
        .await?;
    Ok(SignInResponse {
        account: account_view(&account),
        session,
    })
}

pub(crate) async fn accept_invitation(
    store: &Store,
    clock: &Clock,
    request: AcceptInvitationRequest,
) -> Result<AcceptInvitationResponse, AppServiceError> {
    let display_name = request.display_name.trim().to_owned();
    if request.invitation_token.is_empty() || request.password.is_empty() || display_name.is_empty()
    {
        return Err(AppServiceError::Account(
            AccountFailureReason::InvalidCredential,
        ));
    }
    let Some(invitation) = store
        .find_invitation_by_token(&request.invitation_token)
        .await?
    else {
        return Err(AppServiceError::Account(
            AccountFailureReason::InvitationNotFound,
        ));
    };
    if invitation.consumed_at.is_some() {
        return Err(AppServiceError::Account(
            AccountFailureReason::InvitationAlreadyConsumed,
        ));
    }
    let now = clock.now();
    if invitation.expires_at <= now {
        return Err(AppServiceError::Account(
            AccountFailureReason::InvitationExpired,
        ));
    }

    // The invitee picks a *new* identifier when accepting. R-0001 derives
    // it from the display name + token to keep the handler self-contained
    // until R-0005 lets invitations carry a target identifier.
    let identifier =
        normalize_identifier(&format!("{display_name} via {}", &request.invitation_token));
    if store
        .find_account_by_identifier(&identifier)
        .await?
        .is_some()
    {
        return Err(AppServiceError::Account(
            AccountFailureReason::DuplicateIdentifier,
        ));
    }

    let salt = random_bytes();
    let password_hash = hash_password(&salt, &request.password);
    let id = Uuid::now_v7();
    let account = store
        .insert_account(NewAccount {
            id,
            identifier,
            display_name,
            password_hash,
            password_salt: salt,
            created_at: now,
            org_id: Some(invitation.inviting_org_id),
        })
        .await
        .map_err(map_insert_error)?;
    store
        .insert_membership(account.id, invitation.inviting_org_id)
        .await?;
    store
        .mark_invitation_consumed(&invitation.token, now)
        .await?;

    let session = mint_session(store, account.id).await?;
    store
        .append_event(envelope(
            "account_created",
            &AccountCreated {
                account_id: account.id,
                identifier: account.identifier.clone(),
                org: Some(invitation.inviting_org_id),
                created_at: now,
            },
        ))
        .await?;
    store
        .append_event(envelope(
            "invitation_accepted",
            &InvitationAccepted {
                token: invitation.token.clone(),
                account_id: account.id,
                joined_org: invitation.inviting_org_id,
                at: now,
            },
        ))
        .await?;

    Ok(AcceptInvitationResponse {
        account: account_view(&account),
        session,
        joined_org: invitation.inviting_org_id,
    })
}

fn account_view(record: &AccountRecord) -> AccountView {
    AccountView {
        id: record.id,
        identifier: record.identifier.clone(),
        display_name: record.display_name.clone(),
        org: record.org_id,
    }
}

async fn mint_session(store: &Store, account_id: Uuid) -> Result<SessionView, AppServiceError> {
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let session = store.insert_session(token, account_id).await?;
    Ok(SessionView {
        account_id: session.account_id,
        token: session.token,
    })
}

fn normalize_identifier(raw: &str) -> String {
    raw.trim().to_lowercase()
}

fn random_bytes() -> Vec<u8> {
    Uuid::new_v4().as_bytes().to_vec()
}

fn hash_password(salt: &[u8], password: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(salt);
    hasher.update(password.as_bytes());
    hasher.finalize().to_vec()
}

fn verify_password(salt: &[u8], expected: &[u8], password: &str) -> bool {
    let mut hasher = Sha256::new();
    hasher.update(salt);
    hasher.update(password.as_bytes());
    let computed = hasher.finalize();
    constant_time_eq(computed.as_slice(), expected)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn map_insert_error(err: tanren_store::StoreError) -> AppServiceError {
    let message = err.to_string().to_lowercase();
    if message.contains("unique") || message.contains("duplicate") {
        AppServiceError::Account(AccountFailureReason::DuplicateIdentifier)
    } else {
        AppServiceError::Store(err)
    }
}

/// Convenience for tests / fixtures: hash a password the same way the
/// sign-up handler does. Does not depend on a Store. Public so the BDD
/// step-definition crate can seed an account row without spinning up a
/// fake handler.
#[must_use]
pub fn hash_for_fixture(password: &str) -> (Vec<u8>, Vec<u8>) {
    let salt = random_bytes();
    let hash = hash_password(&salt, password);
    (salt, hash)
}

//! Account-flow handlers: sign-up, sign-in, accept-invitation.
//!
//! Handlers are mechanism-neutral at the contract surface but mechanism-
//! specific underneath: R-0001 pins identifier+password as the simplest
//! credible choice, with hashing delegated to a
//! [`CredentialVerifier`](tanren_identity_policy::CredentialVerifier)
//! trait object. Production binaries inject the
//! [`Argon2idVerifier`](tanren_identity_policy::Argon2idVerifier);
//! BDD scenarios inject the cheap-parameter `fast_for_tests` preset.
//!
//! Session tokens are 256 bits of CSPRNG randomness wrapped in
//! `SessionToken` (URL-safe base64, no padding).
//!
//! Handlers consume `&dyn AccountStore` (the port defined in
//! `tanren_store::traits`); the SeaORM-backed `Store` is the adapter
//! injected by interface binaries. The atomic `consume_invitation` call
//! replaces the previous find-then-check-then-update sequence — no two
//! callers can both successfully accept the same token even under
//! concurrent load.

use chrono::{DateTime, Duration, Utc};
use secrecy::ExposeSecret;
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason, AccountView,
    SessionView, SignInRequest, SignInResponse, SignUpRequest, SignUpResponse,
};
use tanren_identity_policy::{AccountId, CredentialVerifier, Identifier, SessionToken};
use tanren_store::{AccountRecord, AccountStore, ConsumeInvitationError, NewAccount};

use crate::events::{
    AccountCreated, AccountEventKind, InvitationAcceptFailed, InvitationAccepted, SignInFailed,
    SignUpRejected, SignedIn, envelope,
};
use crate::{AppServiceError, Clock};

/// Default session lifetime. Held centrally so the cookie-session policy
/// landing in PR 8 observes a single value.
const SESSION_LIFETIME_DAYS: i64 = 30;

pub(crate) async fn sign_up<S>(
    store: &S,
    clock: &Clock,
    verifier: &dyn CredentialVerifier,
    request: SignUpRequest,
) -> Result<SignUpResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let identifier = Identifier::from_email(&request.email);
    let display_name = request.display_name.trim().to_owned();
    let now = clock.now();

    if request.password.expose_secret().is_empty() || display_name.is_empty() {
        emit_signup_rejected(
            store,
            AccountFailureReason::ValidationFailed,
            &identifier,
            now,
        )
        .await?;
        return Err(AppServiceError::Account(
            AccountFailureReason::ValidationFailed,
        ));
    }

    if store
        .find_account_by_identifier(&identifier)
        .await?
        .is_some()
    {
        emit_signup_rejected(
            store,
            AccountFailureReason::DuplicateIdentifier,
            &identifier,
            now,
        )
        .await?;
        return Err(AppServiceError::Account(
            AccountFailureReason::DuplicateIdentifier,
        ));
    }

    let password_phc = verifier
        .hash(&request.password)
        .map_err(|err| AppServiceError::InvalidInput(err.to_string()))?;
    let id = AccountId::fresh();
    let account = match store
        .insert_account(NewAccount {
            id,
            identifier: identifier.clone(),
            display_name,
            password_phc,
            created_at: now,
            org_id: None,
        })
        .await
        .map_err(map_insert_error)
    {
        Ok(a) => a,
        Err(AppServiceError::Account(reason)) => {
            emit_signup_rejected(store, reason, &identifier, now).await?;
            return Err(AppServiceError::Account(reason));
        }
        Err(other) => return Err(other),
    };

    let session = mint_session(store, account.id, now).await?;
    store
        .append_event(
            envelope(
                AccountEventKind::AccountCreated,
                &AccountCreated {
                    account_id: account.id,
                    identifier: account.identifier.as_str().to_owned(),
                    org: None,
                    created_at: now,
                },
            ),
            now,
        )
        .await?;

    Ok(SignUpResponse {
        account: account_view(&account),
        session,
    })
}

pub(crate) async fn sign_in<S>(
    store: &S,
    clock: &Clock,
    verifier: &dyn CredentialVerifier,
    request: SignInRequest,
) -> Result<SignInResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();
    let identifier = Identifier::from_email(&request.email);

    if request.password.expose_secret().is_empty() {
        emit_signin_failed(
            store,
            AccountFailureReason::ValidationFailed,
            &identifier,
            now,
        )
        .await?;
        return Err(AppServiceError::Account(
            AccountFailureReason::ValidationFailed,
        ));
    }
    let Some(account) = store.find_account_by_email(&request.email).await? else {
        emit_signin_failed(
            store,
            AccountFailureReason::InvalidCredential,
            &identifier,
            now,
        )
        .await?;
        return Err(AppServiceError::Account(
            AccountFailureReason::InvalidCredential,
        ));
    };
    if verifier
        .verify(&request.password, &account.password_phc)
        .is_err()
    {
        emit_signin_failed(
            store,
            AccountFailureReason::InvalidCredential,
            &identifier,
            now,
        )
        .await?;
        return Err(AppServiceError::Account(
            AccountFailureReason::InvalidCredential,
        ));
    }

    let session = mint_session(store, account.id, now).await?;
    store
        .append_event(
            envelope(
                AccountEventKind::SignedIn,
                &SignedIn {
                    account_id: account.id,
                    at: now,
                },
            ),
            now,
        )
        .await?;
    Ok(SignInResponse {
        account: account_view(&account),
        session,
    })
}

pub(crate) async fn accept_invitation<S>(
    store: &S,
    clock: &Clock,
    verifier: &dyn CredentialVerifier,
    request: AcceptInvitationRequest,
) -> Result<AcceptInvitationResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let display_name = request.display_name.trim().to_owned();
    let now = clock.now();
    let token = request.invitation_token.clone();

    if request.password.expose_secret().is_empty() || display_name.is_empty() {
        emit_invitation_accept_failed(store, AccountFailureReason::ValidationFailed, &token, now)
            .await?;
        return Err(AppServiceError::Account(
            AccountFailureReason::ValidationFailed,
        ));
    }

    // The invitee supplies the email when accepting. The contract carries
    // `request.email` as a first-class field; the handler trusts it
    // (subsequent specs will reconcile against the invitation row's
    // target_identifier when R-0005's invite flow lands).
    let identifier = Identifier::from_email(&request.email);
    if store
        .find_account_by_identifier(&identifier)
        .await?
        .is_some()
    {
        emit_invitation_accept_failed(
            store,
            AccountFailureReason::DuplicateIdentifier,
            &token,
            now,
        )
        .await?;
        return Err(AppServiceError::Account(
            AccountFailureReason::DuplicateIdentifier,
        ));
    }

    // Atomic consume: a single conditional UPDATE on the store
    // transitions the invitation from pending → consumed. Concurrent
    // callers serialise to exactly one success; the rest receive
    // `AlreadyConsumed`.
    let consumed = match store.consume_invitation(&token, now).await {
        Ok(consumed) => consumed,
        Err(consume_err) => {
            let reason = match consume_err {
                ConsumeInvitationError::NotFound => AccountFailureReason::InvitationNotFound,
                ConsumeInvitationError::AlreadyConsumed => {
                    AccountFailureReason::InvitationAlreadyConsumed
                }
                ConsumeInvitationError::Expired => AccountFailureReason::InvitationExpired,
                ConsumeInvitationError::Store(err) => return Err(AppServiceError::Store(err)),
            };
            emit_invitation_accept_failed(store, reason, &token, now).await?;
            return Err(AppServiceError::Account(reason));
        }
    };

    let password_phc = verifier
        .hash(&request.password)
        .map_err(|err| AppServiceError::InvalidInput(err.to_string()))?;
    let id = AccountId::fresh();
    let account = store
        .insert_account(NewAccount {
            id,
            identifier,
            display_name,
            password_phc,
            created_at: now,
            org_id: Some(consumed.inviting_org_id),
        })
        .await
        .map_err(map_insert_error)?;
    store
        .insert_membership(account.id, consumed.inviting_org_id, now)
        .await?;

    let session = mint_session(store, account.id, now).await?;
    store
        .append_event(
            envelope(
                AccountEventKind::AccountCreated,
                &AccountCreated {
                    account_id: account.id,
                    identifier: account.identifier.as_str().to_owned(),
                    org: Some(consumed.inviting_org_id),
                    created_at: now,
                },
            ),
            now,
        )
        .await?;
    store
        .append_event(
            envelope(
                AccountEventKind::InvitationAccepted,
                &InvitationAccepted {
                    token,
                    account_id: account.id,
                    joined_org: consumed.inviting_org_id,
                    at: now,
                },
            ),
            now,
        )
        .await?;

    Ok(AcceptInvitationResponse {
        account: account_view(&account),
        session,
        joined_org: consumed.inviting_org_id,
    })
}

async fn emit_signup_rejected<S>(
    store: &S,
    reason: AccountFailureReason,
    identifier: &Identifier,
    now: DateTime<Utc>,
) -> Result<(), AppServiceError>
where
    S: AccountStore + ?Sized,
{
    store
        .append_event(
            envelope(
                AccountEventKind::SignUpRejected,
                &SignUpRejected {
                    reason,
                    identifier: identifier.as_str().to_owned(),
                    at: now,
                },
            ),
            now,
        )
        .await?;
    Ok(())
}

async fn emit_signin_failed<S>(
    store: &S,
    reason: AccountFailureReason,
    identifier: &Identifier,
    now: DateTime<Utc>,
) -> Result<(), AppServiceError>
where
    S: AccountStore + ?Sized,
{
    store
        .append_event(
            envelope(
                AccountEventKind::SignInFailed,
                &SignInFailed {
                    reason,
                    identifier: identifier.as_str().to_owned(),
                    at: now,
                },
            ),
            now,
        )
        .await?;
    Ok(())
}

async fn emit_invitation_accept_failed<S>(
    store: &S,
    reason: AccountFailureReason,
    token: &tanren_identity_policy::InvitationToken,
    now: DateTime<Utc>,
) -> Result<(), AppServiceError>
where
    S: AccountStore + ?Sized,
{
    store
        .append_event(
            envelope(
                AccountEventKind::InvitationAcceptFailed,
                &InvitationAcceptFailed {
                    reason,
                    token: token.clone(),
                    at: now,
                },
            ),
            now,
        )
        .await?;
    Ok(())
}

fn account_view(record: &AccountRecord) -> AccountView {
    AccountView {
        id: record.id,
        identifier: record.identifier.clone(),
        display_name: record.display_name.clone(),
        org: record.org_id,
    }
}

async fn mint_session<S>(
    store: &S,
    account_id: AccountId,
    now: DateTime<Utc>,
) -> Result<SessionView, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let token = SessionToken::generate();
    let expires_at = now + Duration::days(SESSION_LIFETIME_DAYS);
    let session = store
        .insert_session(token, account_id, now, expires_at)
        .await?;
    Ok(SessionView {
        account_id: session.account_id,
        token: session.token,
        expires_at: session.expires_at,
    })
}

fn map_insert_error(err: tanren_store::StoreError) -> AppServiceError {
    let message = err.to_string().to_lowercase();
    if message.contains("unique") || message.contains("duplicate") {
        AppServiceError::Account(AccountFailureReason::DuplicateIdentifier)
    } else {
        AppServiceError::Store(err)
    }
}

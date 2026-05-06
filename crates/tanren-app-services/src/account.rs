//! Account-flow handlers: sign-up, sign-in, accept-invitation.
//!
//! Handlers are mechanism-neutral at the contract surface but mechanism-
//! specific underneath: R-0001 pins identifier+password as the simplest
//! credible choice, with hashing delegated to a [`CredentialVerifier`]
//! trait object. Production binaries inject the [`Argon2idVerifier`];
//! BDD scenarios inject the cheap-parameter `fast_for_tests` preset.
//!
//! [`Argon2idVerifier`]: tanren_identity_policy::Argon2idVerifier
//!
//! Session tokens are 256 bits of CSPRNG randomness wrapped in
//! `SessionToken` (URL-safe base64, no padding).
//!
//! Handlers consume `&dyn AccountStore` (the port defined in
//! `tanren_store::traits`); the SeaORM-backed `Store` is the adapter
//! injected by interface binaries. The atomic `accept_invitation_atomic`
//! call wraps the consume + insert account + insert membership + insert
//! session + append events sequence in one DB transaction, so a
//! transient failure mid-flow leaves the invitation pending and the
//! user can retry. Concurrent acceptances of the same token still
//! serialise to exactly one success — the rest receive
//! `InvitationAlreadyConsumed`.

use chrono::{DateTime, Duration, Utc};
use secrecy::ExposeSecret;
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason, AccountView,
    SessionView, SignInRequest, SignInResponse, SignUpRequest, SignUpResponse,
};
use tanren_identity_policy::{
    AccountId, CredentialVerifier, Identifier, MembershipId, SessionToken,
};
use tanren_store::{
    AcceptInvitationAtomicRequest, AcceptInvitationError, AcceptInvitationEventContext,
    AccountRecord, AccountStore, NewAccount,
};

use crate::events::{
    AccountCreated, AccountEventKind, InvitationAcceptFailed, InvitationAccepted,
    PermissionGranted, SignInFailed, SignUpRejected, SignedIn, envelope,
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
    // Pre-flight duplicate check: lets us fail fast for the common
    // "this email is already an account" case without paying the
    // password-hash cost. The atomic call below ALSO catches a
    // duplicate under race (unique index inside the transaction), so
    // skipping this read would still be correct — it just costs one
    // hash on the reject path.
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

    let password_phc = verifier
        .hash(&request.password)
        .map_err(|err| AppServiceError::InvalidInput(err.to_string()))?;
    let id = AccountId::fresh();
    let session_token = SessionToken::generate();
    let session_expires_at = now + Duration::days(SESSION_LIFETIME_DAYS);

    // Atomic accept: consume + insert account + insert membership +
    // insert session + append both success events run in one DB
    // transaction. A failure on any step rolls the whole flow back so
    // the invitation row stays pending and the user can retry —
    // closing the previous gap where a transient failure after the
    // consume burned the token without producing an account.
    let outcome = match store
        .accept_invitation_atomic(AcceptInvitationAtomicRequest {
            token: token.clone(),
            now,
            account: NewAccount {
                id,
                identifier,
                display_name,
                password_phc,
                created_at: now,
                // `org_id` is overridden inside the atomic call from
                // the consumed invitation row; the value here is
                // irrelevant.
                org_id: None,
            },
            membership_id: MembershipId::fresh(),
            session_token,
            session_expires_at,
            events_builder: build_accept_invitation_events_builder(),
        })
        .await
    {
        Ok(outcome) => outcome,
        Err(err) => {
            let reason = match map_accept_invitation_error(err) {
                Ok(reason) => reason,
                Err(app_err) => return Err(app_err),
            };
            emit_invitation_accept_failed(store, reason, &token, now).await?;
            return Err(AppServiceError::Account(reason));
        }
    };

    Ok(AcceptInvitationResponse {
        account: account_view(&outcome.account),
        session: SessionView {
            account_id: outcome.session.account_id,
            token: outcome.session.token,
            expires_at: outcome.session.expires_at,
        },
        joined_org: outcome.joined_org,
    })
}

/// Build the `Box<dyn FnOnce>` the store invokes inside its transaction
/// to stamp the success-path event envelopes. The store crate does not
/// know the typed event payload shapes — they live in this crate — so
/// the closure builds the wire envelopes once the store has determined
/// the inviting-org id from the consumed invitation row.
fn build_accept_invitation_events_builder() -> tanren_store::AcceptInvitationEventsBuilder {
    Box::new(
        |ctx: &AcceptInvitationEventContext| -> Vec<serde_json::Value> {
            vec![
                envelope(
                    AccountEventKind::AccountCreated,
                    &AccountCreated {
                        account_id: ctx.account_id,
                        identifier: ctx.identifier.as_str().to_owned(),
                        org: Some(ctx.joined_org),
                        created_at: ctx.now,
                    },
                ),
                envelope(
                    AccountEventKind::InvitationAccepted,
                    &InvitationAccepted {
                        token: ctx.token.clone(),
                        account_id: ctx.account_id,
                        joined_org: ctx.joined_org,
                        at: ctx.now,
                    },
                ),
                envelope(
                    AccountEventKind::PermissionGranted,
                    &PermissionGranted {
                        account_id: ctx.account_id,
                        org_id: ctx.joined_org,
                        permissions: ctx.granted_permissions.clone(),
                        at: ctx.now,
                    },
                ),
            ]
        },
    )
}

/// Translate the store-layer taxonomy error into either an
/// [`AccountFailureReason`] (for emit-then-fail flows) or a non-taxonomy
/// [`AppServiceError`] that should bypass the failure-event emit and
/// propagate directly.
fn map_accept_invitation_error(
    err: AcceptInvitationError,
) -> Result<AccountFailureReason, AppServiceError> {
    match err {
        AcceptInvitationError::InvitationNotFound => Ok(AccountFailureReason::InvitationNotFound),
        AcceptInvitationError::InvitationAlreadyConsumed => {
            Ok(AccountFailureReason::InvitationAlreadyConsumed)
        }
        AcceptInvitationError::InvitationExpired => Ok(AccountFailureReason::InvitationExpired),
        AcceptInvitationError::DuplicateIdentifier => Ok(AccountFailureReason::DuplicateIdentifier),
        AcceptInvitationError::Store(store_err) => Err(AppServiceError::Store(store_err)),
    }
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

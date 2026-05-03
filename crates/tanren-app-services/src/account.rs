//! Account-flow handlers: sign-up, sign-in, accept-invitation.
//!
//! Handlers are mechanism-neutral at the contract surface but mechanism-
//! specific underneath: R-0001 pins identifier+password as the simplest
//! credible choice. Hashing currently uses sha-256 over `salt || password`
//! — PR 5 swaps in `Argon2id` behind the
//! `tanren_identity_policy::CredentialVerifier` trait without touching the
//! wire shapes. Session tokens are 256 bits of CSPRNG randomness wrapped
//! in `SessionToken` (URL-safe base64, no padding).
//!
//! Handlers consume `&dyn AccountStore` (the port defined in
//! `tanren_store::traits`); the SeaORM-backed `Store` is the adapter
//! injected by interface binaries. The atomic `consume_invitation` call
//! replaces the previous find-then-check-then-update sequence — no two
//! callers can both successfully accept the same token even under
//! concurrent load.

use chrono::Duration;
use secrecy::ExposeSecret;
use sha2::{Digest, Sha256};
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason, AccountView,
    SessionView, SignInRequest, SignInResponse, SignUpRequest, SignUpResponse,
};
use tanren_identity_policy::{AccountId, Identifier, SessionToken};
use tanren_store::{AccountRecord, AccountStore, ConsumeInvitationError, NewAccount};

use crate::events::{AccountCreated, InvitationAccepted, SignedIn, envelope};
use crate::{AppServiceError, Clock};

/// Default session lifetime. Held centrally so the cookie-session policy
/// landing in PR 8 observes a single value.
const SESSION_LIFETIME_DAYS: i64 = 30;

pub(crate) async fn sign_up<S>(
    store: &S,
    clock: &Clock,
    request: SignUpRequest,
) -> Result<SignUpResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let identifier = Identifier::from_email(&request.email);
    let display_name = request.display_name.trim().to_owned();
    if request.password.expose_secret().is_empty() || display_name.is_empty() {
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
    let salt = random_salt(&now, identifier.as_str());
    let password_hash = hash_password(&salt, request.password.expose_secret());
    let id = AccountId::fresh();
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

    let session = mint_session(store, account.id, now).await?;
    store
        .append_event(
            envelope(
                "account_created",
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
    request: SignInRequest,
) -> Result<SignInResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    if request.password.expose_secret().is_empty() {
        return Err(AppServiceError::Account(
            AccountFailureReason::InvalidCredential,
        ));
    }
    let Some(account) = store.find_account_by_email(&request.email).await? else {
        return Err(AppServiceError::Account(
            AccountFailureReason::InvalidCredential,
        ));
    };
    if !verify_password(
        &account.password_salt,
        &account.password_hash,
        request.password.expose_secret(),
    ) {
        return Err(AppServiceError::Account(
            AccountFailureReason::InvalidCredential,
        ));
    }

    let now = clock.now();
    let session = mint_session(store, account.id, now).await?;
    store
        .append_event(
            envelope(
                "signed_in",
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
    request: AcceptInvitationRequest,
) -> Result<AcceptInvitationResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let display_name = request.display_name.trim().to_owned();
    if request.password.expose_secret().is_empty() || display_name.is_empty() {
        return Err(AppServiceError::Account(
            AccountFailureReason::InvalidCredential,
        ));
    }

    // The invitee picks the email/identifier pair when accepting. PR 7
    // will source the identifier from the invitation row instead — for
    // now the contract carries `request.email` as a first-class field
    // and the handler trusts it.
    let identifier = Identifier::from_email(&request.email);
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
    // Atomic consume: a single conditional UPDATE on the store
    // transitions the invitation from pending → consumed. Concurrent
    // callers serialise to exactly one success; the rest receive
    // `AlreadyConsumed`.
    let consumed = match store
        .consume_invitation(&request.invitation_token, now)
        .await
    {
        Ok(consumed) => consumed,
        Err(ConsumeInvitationError::NotFound) => {
            return Err(AppServiceError::Account(
                AccountFailureReason::InvitationNotFound,
            ));
        }
        Err(ConsumeInvitationError::AlreadyConsumed) => {
            return Err(AppServiceError::Account(
                AccountFailureReason::InvitationAlreadyConsumed,
            ));
        }
        Err(ConsumeInvitationError::Expired) => {
            return Err(AppServiceError::Account(
                AccountFailureReason::InvitationExpired,
            ));
        }
        Err(ConsumeInvitationError::Store(err)) => return Err(AppServiceError::Store(err)),
    };

    let salt = random_salt(&now, identifier.as_str());
    let password_hash = hash_password(&salt, request.password.expose_secret());
    let id = AccountId::fresh();
    let account = store
        .insert_account(NewAccount {
            id,
            identifier,
            display_name,
            password_hash,
            password_salt: salt,
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
                "account_created",
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
                "invitation_accepted",
                &InvitationAccepted {
                    token: request.invitation_token.clone(),
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
    now: chrono::DateTime<chrono::Utc>,
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

/// Compute a deterministic-but-unique salt for the legacy SHA-256
/// hashing path. PR 5 deletes this in favour of `Argon2id` which
/// embeds its own random salt; until then we derive 32 bytes from
/// `sha256(now_iso || identifier)` so the same password hashes
/// differently across accounts (and across invocations).
fn random_salt(now: &chrono::DateTime<chrono::Utc>, identifier: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(now.to_rfc3339().as_bytes());
    hasher.update(b"|");
    hasher.update(identifier.as_bytes());
    hasher.finalize().to_vec()
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

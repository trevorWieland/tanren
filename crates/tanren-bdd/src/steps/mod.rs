//! Step-definition modules.
//!
//! - `account`: B-0043 account flow.
//! - `deployment_posture`: B-0137 deployment posture flow.

use std::future::Future;
use std::time::Duration;

use secrecy::SecretString;
use tanren_contract::{SignInRequest, SignUpRequest};
use tanren_identity_policy::Email;
use tanren_testkit::{HarnessOutcome, record_failure};

use crate::TanrenWorld;

pub mod account;
pub mod deployment_posture;

pub(crate) const TRANSPORT_RETRY_BACKOFF: [Duration; 5] = [
    Duration::from_millis(20),
    Duration::from_millis(40),
    Duration::from_millis(80),
    Duration::from_millis(160),
    Duration::from_millis(320),
];
const EVENTUAL_CONSISTENCY_BACKOFF: [Duration; 4] = [
    Duration::from_millis(20),
    Duration::from_millis(40),
    Duration::from_millis(80),
    Duration::from_millis(120),
];

macro_rules! retry_on_transport {
    ($operation:expr) => {{
        let mut retries = 0usize;
        loop {
            match $operation.await {
                Ok(value) => break Ok(value),
                Err(tanren_testkit::HarnessError::Transport(_))
                    if retries < $crate::steps::TRANSPORT_RETRY_BACKOFF.len() =>
                {
                    let delay = $crate::steps::TRANSPORT_RETRY_BACKOFF[retries];
                    retries += 1;
                    tokio::time::sleep(delay).await;
                }
                Err(err) => break Err(err),
            }
        }
    }};
}

pub(crate) use retry_on_transport;

pub(crate) async fn poll_until<F, Fut>(mut probe: F) -> bool
where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    if probe().await {
        return true;
    }
    for delay in EVENTUAL_CONSISTENCY_BACKOFF {
        tokio::time::sleep(delay).await;
        if probe().await {
            return true;
        }
    }
    false
}

pub(crate) async fn sign_up_actor_with_duplicate_sign_in_fallback(
    world: &mut TanrenWorld,
    actor: &str,
    email: &str,
    password: &str,
    display_name: &str,
) {
    let ctx = world.ensure_account_ctx().await;
    let result = retry_on_transport!({
        let parsed_email = Email::parse(email).expect("scenario emails must parse");
        let request = SignUpRequest {
            email: parsed_email,
            password: SecretString::from(password.to_owned()),
            display_name: display_name.to_owned(),
        };
        ctx.harness.sign_up(request)
    });
    let entry = ctx.actors.entry(actor.to_owned()).or_default();
    entry.identifier = Some(email.to_owned());
    entry.password = Some(SecretString::from(password.to_owned()));
    let outcome = match result {
        Ok(session) => {
            entry.sign_up = Some(session.clone());
            HarnessOutcome::SignedUp(session)
        }
        Err(err) if err.code() == "duplicate_identifier" => {
            let duplicate_err = err;
            let parsed_email = Email::parse(email).expect("scenario emails must parse");
            let sign_in = ctx
                .harness
                .sign_in(SignInRequest {
                    email: parsed_email,
                    password: SecretString::from(
                        entry
                            .password
                            .as_ref()
                            .map_or("", secrecy::ExposeSecret::expose_secret)
                            .to_owned(),
                    ),
                })
                .await;
            match sign_in {
                Ok(session) => {
                    entry.sign_up = Some(session.clone());
                    entry.sign_in = Some(session.clone());
                    HarnessOutcome::SignedUp(session)
                }
                Err(sign_in_err) if sign_in_err.code() == "invalid_credential" => {
                    record_failure(duplicate_err, entry)
                }
                Err(sign_in_err) => record_failure(sign_in_err, entry),
            }
        }
        Err(err) => record_failure(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

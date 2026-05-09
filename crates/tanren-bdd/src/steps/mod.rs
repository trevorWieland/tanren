//! Step-definition modules.
//!
//! - `account`: B-0043 account flow.
//! - `deployment_posture`: B-0137 deployment posture flow.

use std::future::Future;
use std::time::Duration;

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

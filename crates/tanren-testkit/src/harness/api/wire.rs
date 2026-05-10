use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::Client;
use tokio::time::sleep;

use crate::harness::{HarnessError, HarnessResult};

const TRANSPORT_RETRY_ATTEMPTS: usize = 3;
const TRANSPORT_RETRY_DELAY: Duration = Duration::from_millis(50);

pub(super) async fn wait_for_server_ready(base_url: &str) -> HarnessResult<()> {
    let health_url = format!("{base_url}/accounts/active");
    let probe = Client::builder()
        .timeout(Duration::from_millis(250))
        .build()
        .map_err(|e| HarnessError::Transport(format!("probe client build: {e}")))?;
    let mut last_error: Option<String> = None;
    for _ in 0..100 {
        match probe.get(&health_url).send().await {
            Ok(_) => {
                return Ok(());
            }
            Err(err) => {
                last_error = Some(err.to_string());
                sleep(Duration::from_millis(20)).await;
            }
        }
    }
    let detail = last_error.unwrap_or_else(|| "unknown error".to_owned());
    Err(HarnessError::Transport(format!(
        "api harness server did not become ready at {base_url}: {detail}"
    )))
}

fn should_retry_transport(err: &reqwest::Error) -> bool {
    err.is_connect() || err.is_timeout()
}

pub(crate) async fn send_with_retry<F>(
    mut build_request: F,
    operation: &'static str,
) -> HarnessResult<reqwest::Response>
where
    F: FnMut() -> reqwest::RequestBuilder,
{
    let mut last_error: Option<String> = None;
    for attempt in 1..=TRANSPORT_RETRY_ATTEMPTS {
        match build_request().send().await {
            Ok(response) => return Ok(response),
            Err(err) if should_retry_transport(&err) && attempt < TRANSPORT_RETRY_ATTEMPTS => {
                last_error = Some(err.to_string());
                sleep(TRANSPORT_RETRY_DELAY).await;
            }
            Err(err) => {
                return Err(HarnessError::Transport(format!("{operation}: {err}")));
            }
        }
    }
    let detail = last_error.unwrap_or_else(|| "unknown transport error".to_owned());
    Err(HarnessError::Transport(format!(
        "{operation}: exhausted retries ({TRANSPORT_RETRY_ATTEMPTS} attempts): {detail}"
    )))
}

pub(crate) fn scenario_db_path(prefix: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "tanren-bdd-{prefix}-{}-{}.db",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    p
}

pub(crate) fn sqlite_url(path: &Path) -> String {
    format!("sqlite://{}?mode=rwc", path.display())
}

use reqwest::Client;
use tokio::time::{Duration, sleep};

use super::{HarnessError, HarnessResult};

pub(super) async fn wait_until_ready(client: &Client, base_url: &str) -> HarnessResult<()> {
    const MAX_ATTEMPTS: u8 = 40;
    let health_url = format!("{base_url}/health");
    for attempt in 1..=MAX_ATTEMPTS {
        match client.get(&health_url).send().await {
            Ok(response) if response.status().is_success() => return Ok(()),
            Ok(_) | Err(_) if attempt < MAX_ATTEMPTS => sleep(Duration::from_millis(25)).await,
            Ok(response) => {
                return Err(HarnessError::Transport(format!(
                    "api harness health check failed with status {} after {MAX_ATTEMPTS} attempts",
                    response.status()
                )));
            }
            Err(err) => {
                return Err(HarnessError::Transport(format!(
                    "api harness health check failed after {MAX_ATTEMPTS} attempts: {err}"
                )));
            }
        }
    }
    Err(HarnessError::Transport(
        "api harness health check exhausted retries".to_owned(),
    ))
}

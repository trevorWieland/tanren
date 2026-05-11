use reqwest::Client;

use super::{HarnessError, HarnessResult};

pub(super) async fn expire_session(base_url: &str, client: &Client) -> HarnessResult<()> {
    let url = format!("{base_url}/test-hooks/sessions/expire-current");
    let res = client
        .post(&url)
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|e| HarnessError::Transport(format!("expire_session: {e}")))?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(HarnessError::Transport(
            res.text().await.unwrap_or_default(),
        ))
    }
}

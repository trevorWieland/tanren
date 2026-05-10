use reqwest::Client;
use serde_json::Value;
use tanren_contract::{SignedInAccountView, SwitchActiveAccountRequest};
use tanren_identity_policy::AccountId;

use super::super::api_codec::failure_from_body;
use super::WINDOW_ID_HEADER;
use super::wire::send_with_retry;
use crate::harness::{HarnessError, HarnessResult};

pub(super) async fn switch_active_accounts_in_windows_concurrent(
    base_url: &str,
    client: Client,
    requests: Vec<(String, AccountId)>,
) -> Vec<(String, HarnessResult<Vec<SignedInAccountView>>)> {
    let mut handles = Vec::with_capacity(requests.len());
    let mut window_ids = Vec::with_capacity(requests.len());
    for (window_id, target_account_id) in requests {
        let url = format!("{base_url}/accounts/active/switch");
        let client = client.clone();
        window_ids.push(window_id.clone());
        handles.push(tokio::spawn(async move {
            let response = send_with_retry(
                || {
                    client
                        .post(&url)
                        .header(WINDOW_ID_HEADER, &window_id)
                        .json(&SwitchActiveAccountRequest { target_account_id })
                },
                "POST /accounts/active/switch",
            )
            .await;
            let result = match response {
                Ok(response) => decode_switch_response(response).await,
                Err(err) => Err(err),
            };
            (window_id, result)
        }));
    }

    let mut out = Vec::with_capacity(handles.len());
    for (index, handle) in handles.into_iter().enumerate() {
        let fallback_window_id = window_ids[index].clone();
        let outcome = match handle.await {
            Ok(result) => result,
            Err(err) => (
                fallback_window_id,
                Err(HarnessError::Transport(format!("join: {err}"))),
            ),
        };
        out.push(outcome);
    }
    out
}

async fn decode_switch_response(
    response: reqwest::Response,
) -> HarnessResult<Vec<SignedInAccountView>> {
    let status = response.status();
    let json: Value = response
        .json()
        .await
        .map_err(|err| HarnessError::Transport(format!("decode body: {err}")))?;
    if !status.is_success() {
        return Err(failure_from_body(&json));
    }
    serde_json::from_value(json["accounts"].clone())
        .map_err(|err| HarnessError::Transport(format!("decode active accounts: {err}")))
}

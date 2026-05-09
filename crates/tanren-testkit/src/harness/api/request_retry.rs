use reqwest::{Client, Method, RequestBuilder, Response};
use tokio::time::{Duration, sleep};

use crate::harness::{HarnessError, HarnessResult};

const TRANSPORT_RETRY_ATTEMPTS: usize = 3;
const TRANSPORT_RETRY_DELAY: Duration = Duration::from_millis(30);

pub(super) async fn send(
    client: &Client,
    method: Method,
    url: &str,
    operation: &str,
) -> HarnessResult<Response> {
    execute_with_retry(operation, || client.request(method.clone(), url)).await
}

pub(super) async fn send_get_with_retry(
    client: &Client,
    url: &str,
    operation: &str,
) -> HarnessResult<Response> {
    execute_with_retry(operation, || client.request(Method::GET, url)).await
}

pub(super) async fn send_json<T: serde::Serialize + ?Sized>(
    client: &Client,
    method: Method,
    url: &str,
    body: &T,
    operation: &str,
) -> HarnessResult<Response> {
    execute_with_retry(operation, || client.request(method.clone(), url).json(body)).await
}

async fn execute_with_retry<F>(operation: &str, mut build_request: F) -> HarnessResult<Response>
where
    F: FnMut() -> RequestBuilder,
{
    let mut last_error = None;
    for attempt in 1..=TRANSPORT_RETRY_ATTEMPTS {
        match build_request().send().await {
            Ok(response) => return Ok(response),
            Err(error) => {
                if !is_retryable_transport_error(&error) || attempt == TRANSPORT_RETRY_ATTEMPTS {
                    return Err(HarnessError::Transport(format!(
                        "{operation}: {}",
                        match last_error {
                            Some(previous) => format!(
                                "last error after {attempt} attempts: {error}; previous: {previous}"
                            ),
                            None => error.to_string(),
                        }
                    )));
                }
                last_error = Some(error.to_string());
                sleep(TRANSPORT_RETRY_DELAY).await;
            }
        }
    }
    unreachable!("retry loop always returns before completion");
}

fn is_retryable_transport_error(error: &reqwest::Error) -> bool {
    error.is_connect() || error.is_timeout() || error.is_request()
}

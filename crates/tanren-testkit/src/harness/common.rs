use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::http::HeaderValue;
use tanren_app_services::Store;
use tanren_contract::{
    AcceptInvitationRequest, AccountFailureReason, SignInRequest, SignUpRequest,
};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio::time::{Instant, sleep};

use super::{HarnessError, HarnessResult};

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

pub(crate) fn sign_up_body(req: &SignUpRequest) -> serde_json::Value {
    use secrecy::ExposeSecret;
    serde_json::json!({
        "email": req.email.as_str(),
        "password": req.password.expose_secret(),
        "display_name": req.display_name,
    })
}

pub(crate) fn sign_in_body(req: &SignInRequest) -> serde_json::Value {
    use secrecy::ExposeSecret;
    serde_json::json!({
        "email": req.email.as_str(),
        "password": req.password.expose_secret(),
    })
}

pub(crate) fn accept_invitation_body(req: &AcceptInvitationRequest) -> serde_json::Value {
    use secrecy::ExposeSecret;
    serde_json::json!({
        "email": req.email.as_str(),
        "password": req.password.expose_secret(),
        "display_name": req.display_name,
    })
}

pub(crate) fn code_to_reason(code: &str) -> Option<AccountFailureReason> {
    Some(match code {
        "auth_required" => AccountFailureReason::AuthRequired,
        "permission_denied" => AccountFailureReason::PermissionDenied,
        "duplicate_identifier" => AccountFailureReason::DuplicateIdentifier,
        "invalid_credential" => AccountFailureReason::InvalidCredential,
        "validation_failed" => AccountFailureReason::ValidationFailed,
        "invitation_not_found" => AccountFailureReason::InvitationNotFound,
        "invitation_expired" => AccountFailureReason::InvitationExpired,
        "invitation_already_consumed" => AccountFailureReason::InvitationAlreadyConsumed,
        _ => return None,
    })
}

pub(crate) async fn wait_for_http_ready(base_url: &str, timeout: Duration) -> HarnessResult<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(250))
        .build()
        .map_err(|e| HarnessError::Transport(format!("readiness client build: {e}")))?;
    let url = format!("{base_url}/");
    let deadline = Instant::now() + timeout;
    loop {
        match client.get(&url).send().await {
            Ok(_) => return Ok(()),
            Err(_) if Instant::now() < deadline => {
                sleep(Duration::from_millis(25)).await;
            }
            Err(err) => {
                return Err(HarnessError::Transport(format!(
                    "readiness probe failed for {url}: {err}"
                )));
            }
        }
    }
}

pub(crate) async fn spawn_api_server(
    store: Arc<Store>,
    cookie_database_url: &str,
) -> HarnessResult<(String, JoinHandle<()>)> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| HarnessError::Transport(format!("bind listener: {e}")))?;
    let local_addr = listener
        .local_addr()
        .map_err(|e| HarnessError::Transport(format!("local addr: {e}")))?;
    let base_url = format!("http://{local_addr}");
    let cors_origin = HeaderValue::from_str(&base_url)
        .map_err(|e| HarnessError::Transport(format!("cors header: {e}")))?;
    let app =
        tanren_api_app::build_app_with_store(store, cookie_database_url, vec![cors_origin], false)
            .await
            .map_err(|e| HarnessError::Transport(format!("build app: {e}")))?;
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    wait_for_http_ready(&base_url, Duration::from_secs(2)).await?;
    Ok((base_url, server))
}

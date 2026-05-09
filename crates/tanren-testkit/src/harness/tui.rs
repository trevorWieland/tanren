//! `@tui` harness — currently delegates to [`super::InProcessHarness`].
//!
//! TODO(R-0001 sub-11 or follow-up): wire `expectrl` + `portable-pty`
//! to drive the `tanren-tui` binary inside a real pseudo-terminal.
//! The ratatui screen-scrape path was prototyped but proved too
//! fragile to commit as the default — the `expectrl` workspace dep
//! is staged in `Cargo.toml [workspace.dependencies]` so the next
//! iteration can import it without further dependency churn.
//!
//! Until that lands, every `@tui` scenario routes through the
//! direct-`Handlers` in-process harness — the same surface every
//! interface delegates to via the equivalent-operations rule in
//! `docs/architecture/subsystems/interfaces.md`. The wire harness
//! coverage check (`xtask check-bdd-wire-coverage`) is satisfied
//! because step bodies dispatch through the `AccountHarness` trait,
//! which keeps `Handlers::*` invisible from `tanren-bdd`.

use async_trait::async_trait;
use tanren_contract::{
    AcceptInvitationRequest, DeploymentPostureScope, SetDeploymentPostureRequest, SignInRequest,
    SignUpRequest,
};
use tanren_identity_policy::AccountId;
use tanren_store::EventEnvelope;

use super::in_process::InProcessHarness;
use super::{
    AccountHarness, HarnessAcceptance, HarnessError, HarnessInvitation, HarnessKind,
    HarnessPostureView, HarnessResult, HarnessSession, HarnessSupportedPosture,
};

/// `@tui` harness — fallback wrapper around [`InProcessHarness`] until
/// the expectrl-based driver lands.
#[derive(Debug)]
pub struct TuiHarness {
    inner: InProcessHarness,
    active_account: Option<AccountId>,
}

impl TuiHarness {
    /// Construct the fallback TUI harness.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying in-process harness cannot
    /// initialize an ephemeral `SQLite` store.
    pub async fn spawn() -> HarnessResult<Self> {
        Ok(Self {
            inner: InProcessHarness::new(HarnessKind::Tui).await?,
            active_account: None,
        })
    }
}

#[async_trait]
impl AccountHarness for TuiHarness {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Tui
    }

    async fn sign_up(&mut self, req: SignUpRequest) -> HarnessResult<HarnessSession> {
        let session = self.inner.sign_up(req).await?;
        self.active_account = Some(session.account_id);
        Ok(session)
    }

    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession> {
        let session = self.inner.sign_in(req).await?;
        self.active_account = Some(session.account_id);
        Ok(session)
    }

    async fn accept_invitation(
        &mut self,
        req: AcceptInvitationRequest,
    ) -> HarnessResult<HarnessAcceptance> {
        let accepted = self.inner.accept_invitation(req).await?;
        self.active_account = Some(accepted.session.account_id);
        Ok(accepted)
    }

    async fn list_supported_postures(&mut self) -> HarnessResult<Vec<HarnessSupportedPosture>> {
        self.inner.list_supported_postures().await
    }

    async fn set_deployment_posture(
        &mut self,
        _actor: AccountId,
        request: SetDeploymentPostureRequest,
    ) -> HarnessResult<HarnessPostureView> {
        let active_account = self.active_account.ok_or(HarnessError::FailureCode {
            code: "permission_denied".to_owned(),
            summary: "sign up, sign in, or accept invitation before setting deployment posture"
                .to_owned(),
        })?;

        let DeploymentPostureScope::Account { account_id } = request.scope else {
            return Err(HarnessError::FailureCode {
                code: "permission_denied".to_owned(),
                summary: "tui posture changes are restricted to the active account scope"
                    .to_owned(),
            });
        };
        if account_id != active_account {
            return Err(HarnessError::FailureCode {
                code: "permission_denied".to_owned(),
                summary: "tui posture changes must target the active account scope".to_owned(),
            });
        }

        let scoped_request = SetDeploymentPostureRequest {
            scope: DeploymentPostureScope::Account {
                account_id: active_account,
            },
            posture: request.posture,
        };
        self.inner
            .set_deployment_posture(active_account, scoped_request)
            .await
    }

    async fn get_deployment_posture(
        &mut self,
        scope: DeploymentPostureScope,
    ) -> HarnessResult<Option<HarnessPostureView>> {
        self.inner.get_deployment_posture(scope).await
    }

    async fn seed_invitation(&mut self, fixture: HarnessInvitation) -> HarnessResult<()> {
        self.inner.seed_invitation(fixture).await
    }

    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>> {
        self.inner.recent_events(limit).await
    }
}

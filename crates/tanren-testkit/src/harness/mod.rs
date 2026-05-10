//! Per-interface BDD wire-harness wiring (R-0001 sub-9).
//!
//! Every account-flow BDD scenario tagged with one of the closed
//! interface tags (`@api`, `@cli`, `@mcp`, `@tui`, `@web`) routes
//! through the matching [`AccountHarness`] implementation rather than
//! calling `tanren_app_services::Handlers::*` directly. The harness is
//! the wire-level seam — `@api` drives a real axum server via
//! reqwest with a cookie jar, `@cli` shells out to the `tanren-cli`
//! binary, `@mcp` drives the rmcp server through the rmcp client, and
//! `@tui` drives the `tanren-tui` binary in a pseudo-terminal. The
//! `xtask check-bdd-wire-coverage` guard rejects any step body that
//! references `Handlers::sign_up`/`sign_in`/`accept_invitation`
//! directly, so adding a new step that bypasses this seam fails CI.
//!
//! See `docs/architecture/subsystems/behavior-proof.md` §
//! "Per-interface BDD wire-harness wiring (R-0001)" and
//! `profiles/rust-cargo/testing/bdd-wire-harness.md`.
//!
mod api;
mod cli;
mod in_process;
mod mcp;
mod role_utils;
mod tui;
mod tui_binary;
mod tui_codec;
mod tui_driver;
mod tui_errors;
mod web;

use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use tanren_contract::{
    AcceptInvitationRequest, AccountFailureReason, AccountView, ApplyRoleRequest,
    ApplyRoleResponse, CreateRoleRequest, CreateRoleResponse, DeleteRoleRequest,
    DeleteRoleResponse, EditRoleRequest, EditRoleResponse, PermissionCheckRequest,
    PermissionCheckResponse, PermissionGrantView, RoleFailureReason, RoleTemplateView,
    SignInRequest, SignUpRequest,
};
use tanren_identity_policy::{
    AccountId, InvitationToken, OrgId, PermissionName, PermissionScope, PrincipalRef, RoleId,
    RoleName, RoleScope, ScopedRole,
};
use tanren_store::{ApplyRole, EventEnvelope, NewRole, RoleStore};

pub use api::ApiHarness;
pub use cli::CliHarness;
pub use in_process::InProcessHarness;
pub use mcp::McpHarness;
pub(crate) use role_utils::{permission_grant_view, read_all_direct_grants, role_template_view};
pub use tui::TuiHarness;
pub use web::WebHarness;

/// Identifier for the active wire-harness — derived from the cucumber
/// scenario tags by the BDD World.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessKind {
    /// Direct-`Handlers` dispatch — the legacy path; fallback for
    /// untagged scenarios.
    InProcess,
    /// Spawns the `tanren-api` server on an ephemeral port; reqwest
    /// with cookie jar.
    Api,
    /// Shells out to the `tanren-cli` binary.
    Cli,
    /// Spawns the `tanren-mcp` server on an ephemeral port; rmcp
    /// streamable-HTTP client.
    Mcp,
    /// Drives the `tanren-tui` binary inside a pseudo-terminal.
    Tui,
    /// Drives the web frontend via Playwright (deferred to PR 11 —
    /// falls back to in-process).
    Web,
}

impl HarnessKind {
    /// Map the cucumber scenario tags onto the harness to instantiate.
    /// The closed allowlist of interface tags is the single source of
    /// truth — anything else falls back to [`HarnessKind::InProcess`].
    #[must_use]
    pub fn from_tags<I, S>(tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for tag in tags {
            let raw = tag.as_ref();
            let normalized = raw.strip_prefix('@').unwrap_or(raw);
            match normalized {
                "api" => return Self::Api,
                "cli" => return Self::Cli,
                "mcp" => return Self::Mcp,
                "tui" => return Self::Tui,
                "web" => return Self::Web,
                _ => {}
            }
        }
        Self::InProcess
    }
}

/// Outcome of a successful sign-up / sign-in / accept-invitation call
/// against any harness. The `session.has_token` field aggregates
/// "received a session token" across cookie + bearer transports —
/// `@api` cookies count just as much as `@cli`/`@mcp`/`@tui` bearer
/// tokens do.
#[derive(Debug, Clone)]
pub struct HarnessSession {
    /// Project-side view of the account.
    pub account: AccountView,
    /// Account id (mirrors `account.id` for ergonomics).
    pub account_id: AccountId,
    /// Wall-clock expiry of the session.
    pub expires_at: DateTime<Utc>,
    /// True when the surface delivered a session token — either as a
    /// `Set-Cookie: tanren_session=...` header (api) or in the
    /// response body (cli/mcp/tui/in-process).
    pub has_token: bool,
}

/// Outcome of a successful invitation-acceptance call.
#[derive(Debug, Clone)]
pub struct HarnessAcceptance {
    /// Session minted on accept.
    pub session: HarnessSession,
    /// Organization the new account joined.
    pub joined_org: OrgId,
}

/// Failure surface — every harness collapses transport-specific
/// failures down to a [`AccountFailureReason`] (matched on the wire
/// `code`) plus an opaque message used for diagnostic output.
#[derive(Debug, thiserror::Error)]
pub enum HarnessError {
    /// A taxonomy failure with a known `code`.
    #[error("{0:?}: {1}")]
    Account(AccountFailureReason, String),
    /// A non-taxonomy failure (transport, parse, connection, etc.).
    #[error("transport: {0}")]
    Transport(String),
}

impl HarnessError {
    /// Project the wire `code` for a [`HarnessError`]. Mirrors the
    /// shape every interface returns under the shared error taxonomy.
    #[must_use]
    pub fn code(&self) -> String {
        match self {
            Self::Account(reason, _) => reason.code().to_owned(),
            Self::Transport(_) => "transport_error".to_owned(),
        }
    }
}

/// Convenient alias for harness fallibility.
pub type HarnessResult<T> = Result<T, HarnessError>;

/// Role-harness failure surface.
#[derive(Debug, thiserror::Error)]
pub enum RoleHarnessError {
    /// Role taxonomy failure with stable `code`.
    #[error("{0:?}: {1}")]
    Role(RoleFailureReason, String),
    /// Non-taxonomy failure (transport, parse, connection, etc.).
    #[error("transport: {0}")]
    Transport(String),
}

impl RoleHarnessError {
    /// Project wire `code` for this role failure.
    #[must_use]
    pub fn code(&self) -> String {
        match self {
            Self::Role(reason, _) => reason.code().to_owned(),
            Self::Transport(_) => "transport_error".to_owned(),
        }
    }
}

/// Convenient alias for role-harness fallibility.
pub type RoleHarnessResult<T> = Result<T, RoleHarnessError>;

/// Seed fixture for inserting role templates directly into harness
/// proof-state storage.
#[derive(Debug, Clone)]
pub struct HarnessRoleTemplate {
    /// Stable role id to insert.
    pub id: RoleId,
    /// Role scope.
    pub scope: RoleScope,
    /// Role display name.
    pub name: RoleName,
    /// Permission bundle.
    pub permissions: Vec<PermissionName>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last-updated timestamp.
    pub updated_at: DateTime<Utc>,
}

impl HarnessRoleTemplate {
    /// Projection for request-driven interfaces that require a scoped role id.
    #[must_use]
    pub fn scoped_role(&self) -> ScopedRole {
        ScopedRole {
            role_id: self.id,
            scope: self.scope,
        }
    }
}

/// Role methods available on wire harnesses.
#[async_trait]
pub trait RoleHarness: Send + std::fmt::Debug {
    /// Create a role template through the harness wire surface.
    async fn create_role(
        &mut self,
        req: CreateRoleRequest,
    ) -> RoleHarnessResult<CreateRoleResponse>;

    /// Edit a role template through the harness wire surface.
    async fn edit_role(&mut self, req: EditRoleRequest) -> RoleHarnessResult<EditRoleResponse>;

    /// Delete a role template through the harness wire surface.
    async fn delete_role(
        &mut self,
        req: DeleteRoleRequest,
    ) -> RoleHarnessResult<DeleteRoleResponse>;

    /// Apply a role template through the harness wire surface.
    async fn apply_role(&mut self, req: ApplyRoleRequest) -> RoleHarnessResult<ApplyRoleResponse>;

    /// Check permission through the harness wire surface.
    async fn check_permission(
        &mut self,
        req: PermissionCheckRequest,
    ) -> RoleHarnessResult<PermissionCheckResponse>;

    /// Seed role-template proof state directly through the harness store.
    async fn seed_role_template(&mut self, fixture: HarnessRoleTemplate) -> RoleHarnessResult<()>;

    /// Seed role-admin direct grants for the currently authenticated actor.
    async fn seed_role_admin_for_authenticated_actor(
        &mut self,
        scope: RoleScope,
        permissions: Vec<PermissionName>,
    ) -> RoleHarnessResult<()>;

    /// Read one role-template snapshot from harness proof-state storage.
    async fn read_role_template(
        &self,
        role: ScopedRole,
    ) -> RoleHarnessResult<Option<RoleTemplateView>>;

    /// Read all direct grants for one principal from harness proof-state storage.
    async fn read_direct_grants(
        &self,
        principal: PrincipalRef,
    ) -> RoleHarnessResult<Vec<PermissionGrantView>>;
}

/// Specification for an invitation seeded into the harness's backing
/// store. Per-harness implementations translate this into the shape
/// their underlying `Store` requires.
#[derive(Debug, Clone)]
pub struct HarnessInvitation {
    /// The opaque token callers will accept against.
    pub token: InvitationToken,
    /// Inviting organization id.
    pub inviting_org: OrgId,
    /// Expiry instant.
    pub expires_at: DateTime<Utc>,
}

/// Per-interface seam used by the BDD step-definition crate. Every
/// implementation drives the matching real surface end-to-end: api
/// scenarios go through reqwest, cli scenarios through subprocess,
/// mcp scenarios through the rmcp client, etc. The trait keeps
/// [`tanren_app_services::Handlers`] out of `tanren-bdd` —
/// `xtask check-bdd-wire-coverage` rejects any step that bypasses
/// this seam.
#[async_trait]
pub trait AccountHarness: Send + std::fmt::Debug {
    /// Identifier for diagnostic output.
    fn kind(&self) -> HarnessKind;

    /// Self-signup against the underlying surface.
    async fn sign_up(&mut self, req: SignUpRequest) -> HarnessResult<HarnessSession>;

    /// Sign-in against the underlying surface.
    async fn sign_in(&mut self, req: SignInRequest) -> HarnessResult<HarnessSession>;

    /// Accept an invitation against the underlying surface.
    async fn accept_invitation(
        &mut self,
        req: AcceptInvitationRequest,
    ) -> HarnessResult<HarnessAcceptance>;

    /// Fan out N invitation-acceptance requests in parallel against the
    /// underlying surface. Used by the `@falsification @api` race
    /// scenario to prove `consume_invitation`'s atomicity. The default
    /// implementation runs the requests serially via [`accept_invitation`];
    /// `ApiHarness` overrides this to dispatch each request as an
    /// independent `tokio::spawn` so the race actually happens.
    ///
    /// Returns one `HarnessResult<HarnessAcceptance>` per input request
    /// in the order submitted.
    ///
    /// [`accept_invitation`]: AccountHarness::accept_invitation
    async fn accept_invitations_concurrent(
        &mut self,
        requests: Vec<AcceptInvitationRequest>,
    ) -> Vec<HarnessResult<HarnessAcceptance>> {
        let mut out = Vec::with_capacity(requests.len());
        for r in requests {
            out.push(self.accept_invitation(r).await);
        }
        out
    }

    /// Seed a fresh invitation into the harness's backing store.
    async fn seed_invitation(&mut self, fixture: HarnessInvitation) -> HarnessResult<()>;

    /// Read recent events from the harness's backing store.
    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>>;
}

/// Default short-window timeout used by the wire harnesses.
pub(crate) const HARNESS_DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

/// Per-actor state captured by `Given <actor> has signed up ...` steps
/// so subsequent steps can sign them in or assert on the prior outcome.
/// The state is harness-agnostic — all transport bookkeeping lives on
/// the harness implementation, this struct is pure result-tracking.
///
/// The cached password is wrapped in `SecretString` so the BDD World's
/// `Debug` output (and any incidental tracing) cannot leak the cleartext
/// — the `Then signs in with the same credentials` step needs to recall
/// the value end-to-end, but it should never appear in logs.
#[derive(Debug, Default, Clone)]
pub struct ActorState {
    /// Identifier (email) the actor signed up with.
    pub identifier: Option<String>,
    /// Password the actor signed up with — kept opaque via `SecretString`
    /// so step bookkeeping doesn't keep a plaintext copy in `Debug` output.
    pub password: Option<secrecy::SecretString>,
    /// Last successful sign-up session.
    pub sign_up: Option<HarnessSession>,
    /// Last successful sign-in session.
    pub sign_in: Option<HarnessSession>,
    /// Last successful invitation acceptance.
    pub accept_invitation: Option<HarnessAcceptance>,
    /// Last failure (taxonomy code), if any.
    pub last_failure: Option<AccountFailureReason>,
}

/// Outcome of the most recent action.
#[derive(Debug, Clone)]
pub enum HarnessOutcome {
    /// Successful sign-up.
    SignedUp(HarnessSession),
    /// Successful sign-in.
    SignedIn(HarnessSession),
    /// Successful invitation acceptance.
    AcceptedInvitation(HarnessAcceptance),
    /// Account-flow taxonomy failure (with the wire `code`).
    Failure(AccountFailureReason),
    /// Non-taxonomy infrastructure failure.
    Other(String),
}

impl HarnessOutcome {
    /// Project the failure code for this outcome. Mirrors the shape
    /// the existing `Then the request fails with code "<code>"` step
    /// asserts on.
    #[must_use]
    pub fn failure_code(&self) -> Option<String> {
        match self {
            Self::Failure(reason) => Some(reason.code().to_owned()),
            Self::SignedUp(_)
            | Self::SignedIn(_)
            | Self::AcceptedInvitation(_)
            | Self::Other(_) => None,
        }
    }
}

/// Project a [`HarnessError`] into the actor-state + outcome pair the
/// step bodies record. Lifted out of `account.rs` so each step body
/// stays a one-liner against the harness trait.
pub fn record_failure(err: HarnessError, entry: &mut ActorState) -> HarnessOutcome {
    match err {
        HarnessError::Account(reason, _) => {
            entry.last_failure = Some(reason);
            HarnessOutcome::Failure(reason)
        }
        HarnessError::Transport(message) => HarnessOutcome::Other(format!("transport: {message}")),
    }
}

pub(crate) async fn seed_role_admin_grants<S>(
    store: &S,
    actor: AccountId,
    scope: RoleScope,
    permissions: Vec<PermissionName>,
) -> RoleHarnessResult<()>
where
    S: RoleStore + ?Sized,
{
    if permissions.is_empty() {
        return Ok(());
    }
    let now = Utc::now();
    let role = store
        .create_role(NewRole {
            id: RoleId::fresh(),
            scope,
            name: RoleName::parse("bdd-role-admin-bootstrap")
                .expect("bdd bootstrap role name literal must parse"),
            permissions,
            created_at: now,
            updated_at: now,
        })
        .await
        .map_err(|e| RoleHarnessError::Transport(format!("seed_role_admin/create_role: {e}")))?;
    let grant_scope = match role.scope {
        RoleScope::Account { account_id } => PermissionScope::Account { account_id },
        RoleScope::Organization { org_id } => PermissionScope::Organization { org_id },
        RoleScope::Project { project_id } => PermissionScope::Project { project_id },
    };
    store
        .apply_role(ApplyRole {
            role: role.scoped_role(),
            principal: PrincipalRef::Account { account_id: actor },
            grant_scope,
            granted_by: PrincipalRef::Account { account_id: actor },
            granted_at: now,
        })
        .await
        .map_err(|e| RoleHarnessError::Transport(format!("seed_role_admin/apply_role: {e}")))?;
    Ok(())
}

/// Filter `recent_events` rows by their `payload.kind` field — the
/// shape the existing `Then a "<kind>" event is recorded` step
/// asserts on.
#[must_use]
pub fn event_kinds(events: &[EventEnvelope]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| {
            e.payload
                .get("kind")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .collect()
}

/// Track concurrent invitation-acceptance outcomes for the falsification
/// race scenario (`When 20 actors concurrently accept invitation ...`).
#[derive(Debug, Default)]
pub struct ConcurrentAcceptanceTally {
    /// Number of outcomes that returned a fresh session.
    pub successes: usize,
    /// Failures bucketed by wire `code`.
    pub failures_by_code: HashMap<String, usize>,
    /// Non-taxonomy errors (timeouts, transport failures).
    pub other: Vec<String>,
}

impl ConcurrentAcceptanceTally {
    /// Record a single outcome.
    pub fn record(&mut self, outcome: Result<HarnessAcceptance, HarnessError>) {
        match outcome {
            Ok(_) => self.successes += 1,
            Err(HarnessError::Account(reason, _)) => {
                let code = reason.code().to_owned();
                *self.failures_by_code.entry(code).or_insert(0) += 1;
            }
            Err(HarnessError::Transport(msg)) => self.other.push(msg),
        }
    }

    /// Number of failures matching `code`.
    #[must_use]
    pub fn failures_with_code(&self, code: &str) -> usize {
        self.failures_by_code.get(code).copied().unwrap_or(0)
    }
}

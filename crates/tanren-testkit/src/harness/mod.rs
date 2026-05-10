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
//! See `docs/architecture/subsystems/behavior-proof.md` §
//! "Per-interface BDD wire-harness wiring (R-0001)" and
//! `profiles/rust-cargo/testing/bdd-wire-harness.md`.
//!
//! - `@api` — full impl. Spawns `tanren_api_app::build_app_with_store`
//!   on an ephemeral port, drives via `reqwest::Client` with
//!   `cookie_store(true)`. The "session token received" check passes
//!   when the cookie jar contains a `tanren_session` cookie OR the
//!   response body returned a bearer token.
//! - `@cli` — full impl. Spawns the `tanren-cli` binary via
//!   `tokio::process::Command` against a shared `SQLite` file. Parses
//!   the `account_id=... session=...` stdout shape.
//! - `@mcp` — full impl. Spawns `tanren_mcp_app::build_router_with_store`
//!   on an ephemeral port and drives the three account-flow tools via
//!   the rmcp streamable-HTTP client.
//! - `@tui` — account actions currently dispatch through
//!   [`InProcessHarness`], but `my_permissions` rendering runs through
//!   `tanren-tui-app`'s ratatui draw path so the witness asserts on the
//!   real TUI output surface.
//! - `@web` — falls back to [`InProcessHarness`]. PR 11 stands up a
//!   parallel Node-side Playwright harness for the same `@web` Gherkin
//!   scenarios via `playwright-bdd`. The two layers prove themselves
//!   independently against the same scenario file. See `harness::web`.
//! - untagged / fallback — [`InProcessHarness`] (direct-`Handlers`
//!   dispatch on an ephemeral `SQLite` store).

mod api;
mod api_ready;
mod cli;
mod in_process;
mod mcp;
mod tui;
mod web;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use tanren_contract::{
    AcceptInvitationRequest, AccountFailureReason, AccountView, MyAccountCapabilitiesResponse,
    MyPermissionsResponse, SignInRequest, SignUpRequest,
};
use tanren_identity_policy::{
    AccountId, InvitationToken, OrgId, PermissionGrantSource, PermissionName,
    PolicyConstraintReason, PolicyConstraintSource, ProjectId,
};
use tanren_store::EventEnvelope;

pub use api::ApiHarness;
pub use cli::CliHarness;
pub use in_process::InProcessHarness;
pub use mcp::McpHarness;
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
    /// Drives the `tanren-tui` binary inside a pty (deferred — falls
    /// back to in-process for PR 9).
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
    /// A taxonomy failure with a non-account `code` (for example
    /// `permission_denied` from self-permission introspection).
    #[error("{code}: {summary}")]
    FailureCode { code: String, summary: String },
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
            Self::FailureCode { code, .. } => code.clone(),
            Self::Transport(_) => "transport_error".to_owned(),
        }
    }
}

/// Convenient alias for harness fallibility.
pub type HarnessResult<T> = Result<T, HarnessError>;

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

/// Scope for a seeded permission grant fixture.
#[derive(Debug, Clone, Copy)]
pub enum HarnessPermissionScope {
    /// Organization-level grant.
    Organization(OrgId),
    /// Project-level grant.
    Project(ProjectId),
}

/// Optional policy-constraint fixture attached to a grant.
#[derive(Debug, Clone)]
pub struct HarnessPermissionConstraintFixture {
    /// Human-readable reason surfaced by the self-permissions view.
    pub reason: PolicyConstraintReason,
    /// Policy scope that produced the constraint.
    pub source: PolicyConstraintSource,
}

/// Seed specification for one permission grant fixture.
#[derive(Debug, Clone)]
pub struct HarnessPermissionGrantFixture {
    /// Account receiving the permission.
    pub account_id: AccountId,
    /// Scope where the grant applies.
    pub scope: HarnessPermissionScope,
    /// Canonical permission identifier.
    pub permission: PermissionName,
    /// Grant source metadata (direct vs role template).
    pub grant_source: PermissionGrantSource,
    /// Optional policy constraint to attach to this grant.
    pub policy_constraint: Option<HarnessPermissionConstraintFixture>,
}

/// Wire/output projection of a self-permission query.
#[derive(Debug, Clone)]
pub struct HarnessPermissionsView {
    /// Structured response from the surface.
    pub response: MyPermissionsResponse,
    /// Surface-native textual rendering captured from the same call.
    pub rendered: String,
}

/// Wire/output projection of self-permissions capability discovery.
#[derive(Debug, Clone)]
pub struct HarnessPermissionsCapabilityView {
    /// Structured capability response from the surface.
    pub response: MyAccountCapabilitiesResponse,
    /// Surface-native textual rendering captured from the same call.
    pub rendered: String,
}

/// Optional pagination hints for self-permission introspection.
#[derive(Debug, Clone, Default)]
pub struct HarnessMyPermissionsQuery {
    pub limit: Option<u16>,
    pub cursor: Option<String>,
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

    /// Read effective permissions for the signed-in actor. Implementations
    /// must enforce self-scope semantics when `requested_account_id`
    /// differs from `session_account_id`.
    async fn my_permissions(
        &mut self,
        session_account_id: AccountId,
        requested_account_id: Option<AccountId>,
    ) -> HarnessResult<HarnessPermissionsView>;

    /// Read effective permissions with optional pagination hints.
    async fn my_permissions_query(
        &mut self,
        session_account_id: AccountId,
        requested_account_id: Option<AccountId>,
        query: HarnessMyPermissionsQuery,
    ) -> HarnessResult<HarnessPermissionsView>;

    /// Discover whether the actor may open the self-permissions view.
    /// `requested_account_id` is optional to support falsification paths
    /// where the caller asks for another account.
    async fn my_permissions_capability(
        &mut self,
        session_account_id: AccountId,
        requested_account_id: Option<AccountId>,
    ) -> HarnessResult<HarnessPermissionsCapabilityView> {
        let view = self
            .my_permissions(session_account_id, requested_account_id)
            .await?;
        Ok(HarnessPermissionsCapabilityView {
            response: MyAccountCapabilitiesResponse {
                can_view_my_permissions: true,
            },
            rendered: view.rendered,
        })
    }

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

    /// Seed one permission grant (and optional constraint) into the
    /// harness backing store for BDD fixtures.
    async fn seed_permission_grant(
        &mut self,
        fixture: HarnessPermissionGrantFixture,
    ) -> HarnessResult<()>;

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
    /// Non-account taxonomy failure with a stable wire `code`.
    FailureCode(String),
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
            Self::FailureCode(code) => Some(code.clone()),
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
        HarnessError::FailureCode { code, .. } => HarnessOutcome::FailureCode(code),
        HarnessError::Transport(message) => HarnessOutcome::Other(format!("transport: {message}")),
    }
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

/// Verify a read-only permissions check by comparing a pre-query
/// checkpoint against later events and rejecting any new permission
/// mutation kinds.
pub fn assert_no_permission_request_or_grant_events(
    event_ids_before: &HashSet<String, impl std::hash::BuildHasher>,
    events_after: &[EventEnvelope],
) -> HarnessResult<()> {
    let mut forbidden = Vec::new();
    for event in events_after {
        if event_ids_before.contains(&event.id.to_string()) {
            continue;
        }
        if let Some(kind) = event.payload.get("kind").and_then(Value::as_str)
            && (kind == "permission_requested" || kind == "permission_granted")
        {
            forbidden.push(kind.to_owned());
        }
    }
    if forbidden.is_empty() {
        return Ok(());
    }
    Err(HarnessError::Transport(format!(
        "permissions view should be read-only; saw forbidden events {forbidden:?}"
    )))
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
            Err(HarnessError::FailureCode { code, .. }) => {
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

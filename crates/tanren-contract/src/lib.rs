//! Wire-shape contracts for Tanren's API and MCP boundaries.
//!
//! Types in this crate are the serialization surface that interface binaries
//! (`tanren-api`, `tanren-mcp`, the generated web client) expose to external
//! callers. Orchestration logic does not live here — this crate stays a pure
//! shape layer so that wire compatibility is reviewable in isolation.

pub mod account;
pub mod cli_output;
pub mod install;

pub use account::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason, AccountView,
    SessionEnvelope, SessionView, SignInRequest, SignInResponse, SignUpRequest, SignUpResponse,
};

pub use cli_output::{
    CommandName, DriftDetailStatus, DriftStatus, InstallStatus, OutputKey, RecordKind,
    parse_bracket_list_percent_decoded, parse_kv_fields_percent_decoded, percent_decode,
    percent_encode, write_bracket_list, write_count_field, write_field, write_typed_field,
};

pub use install::{
    AssetClass, InstallContractError, InstallIntegration, InstallManifest, InstallProfile,
    InstallSelection, ManifestEntry, PreservationPolicy, RepoRelativePath, Sha256Hex,
    Sha256HexParseError, parse_integration_selection, sha256_hex,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Wire-shape version for Tanren's external contract surface.
///
/// Bumped on breaking changes to the request/response shapes this crate
/// exports. The wire version moves independently of [`tanren_domain`'s
/// `DomainVersion`](../tanren_domain/struct.DomainVersion.html) — the wire
/// format may stay stable across domain refactors and vice versa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContractVersion(u32);

impl ContractVersion {
    /// Current wire-contract version.
    pub const CURRENT: Self = Self(0);

    /// Construct a contract version from its numeric form.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// The numeric value of this wire version.
    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }
}

/// Errors raised when a wire payload fails contract-level validation.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ContractError {
    /// The payload's declared contract version is incompatible with this build.
    #[error("incompatible contract version: payload={payload:?}, supported={supported:?}")]
    IncompatibleVersion {
        /// Version declared by the incoming payload.
        payload: ContractVersion,
        /// Version this build understands.
        supported: ContractVersion,
    },
}

//! Capability-scope ingestion from `TANREN_PHASE_CAPABILITIES`.

use tanren_app_services::methodology::{CapabilityScope, parse_scope_env};

/// Parse the env-var capability list. An empty / missing value yields
/// an empty scope (denies every tool call) so misconfigured dispatchers
/// fail fast and safely.
pub(crate) fn parse_from_env() -> CapabilityScope {
    let raw = std::env::var("TANREN_PHASE_CAPABILITIES").unwrap_or_default();
    parse_scope_env(&raw)
}

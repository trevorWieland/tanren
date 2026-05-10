//! MCP bootstrap-credential authorization port.
//!
//! The MCP transport passes raw header values into this module; parsing and
//! verification happen here so interface crates do not own secret comparison
//! behavior.

use std::sync::Arc;

use secrecy::{ExposeSecret, SecretString};
use tanren_identity_policy::{Argon2idVerifier, CredentialVerifier, IdentityError};
use thiserror::Error;

/// Environment variable carrying the MCP bootstrap credential.
pub const MCP_API_KEY_ENV: &str = "TANREN_MCP_API_KEY";

/// Raw authentication context supplied by a transport adapter.
#[derive(Debug, Clone, Default)]
pub struct McpActorContext {
    authorization_header: Option<SecretString>,
    x_api_key_header: Option<SecretString>,
}

impl McpActorContext {
    /// Construct an actor context from raw header values.
    #[must_use]
    pub fn new(authorization_header: Option<String>, x_api_key_header: Option<String>) -> Self {
        Self {
            authorization_header: authorization_header.map(SecretString::from),
            x_api_key_header: x_api_key_header.map(SecretString::from),
        }
    }

    fn parse_presented_credential(&self) -> Option<SecretString> {
        let bearer = self
            .authorization_header
            .as_ref()
            .map(ExposeSecret::expose_secret)
            .and_then(|value| {
                value
                    .strip_prefix("Bearer ")
                    .or_else(|| value.strip_prefix("bearer "))
            });
        let credential = bearer.or(self
            .x_api_key_header
            .as_ref()
            .map(ExposeSecret::expose_secret))?;
        let normalized = credential.trim();
        if normalized.is_empty() {
            return None;
        }
        Some(SecretString::from(normalized.to_owned()))
    }
}

/// App-service authorization config for MCP bootstrap credential checks.
#[derive(Debug, Clone)]
pub struct McpAuthConfig {
    verifier: Arc<dyn CredentialVerifier>,
    bootstrap_key_hash: Option<String>,
}

impl McpAuthConfig {
    /// Build config from the raw env-var lookup result.
    ///
    /// # Errors
    ///
    /// Returns [`McpAuthConfigError`] when the env var cannot be read,
    /// when it is present but empty after trimming, or when hashing fails.
    pub fn from_env_var(
        raw: Result<String, std::env::VarError>,
    ) -> Result<Self, McpAuthConfigError> {
        let bootstrap_key = match raw {
            Ok(value) => {
                let normalized = value.trim();
                if normalized.is_empty() {
                    return Err(McpAuthConfigError::EmptyBootstrapKey);
                }
                Some(SecretString::from(normalized.to_owned()))
            }
            Err(std::env::VarError::NotPresent) => None,
            Err(err) => return Err(McpAuthConfigError::ReadEnvironment(err)),
        };
        Self::from_bootstrap_key(bootstrap_key)
    }

    /// Build config from an explicit bootstrap credential.
    ///
    /// # Errors
    ///
    /// Returns [`McpAuthConfigError::HashFailed`] if hashing fails.
    pub fn from_bootstrap_key(
        bootstrap_key: Option<SecretString>,
    ) -> Result<Self, McpAuthConfigError> {
        let verifier: Arc<dyn CredentialVerifier> = Arc::new(Argon2idVerifier::production());
        let bootstrap_key_hash = match bootstrap_key {
            Some(key) => Some(
                verifier
                    .hash(&key)
                    .map_err(McpAuthConfigError::HashFailed)?,
            ),
            None => None,
        };
        Ok(Self {
            verifier,
            bootstrap_key_hash,
        })
    }

    /// Whether a bootstrap credential hash is configured.
    #[must_use]
    pub const fn is_configured(&self) -> bool {
        self.bootstrap_key_hash.is_some()
    }

    /// Authorize an MCP request's presented credential.
    pub fn authorize(&self, actor: &McpActorContext) -> Result<(), McpAuthFailure> {
        let Some(stored_hash) = self.bootstrap_key_hash.as_deref() else {
            return Err(McpAuthFailure::Unavailable);
        };
        let Some(presented) = actor.parse_presented_credential() else {
            return Err(McpAuthFailure::AuthRequired);
        };

        match self.verifier.verify(&presented, stored_hash) {
            Ok(()) => Ok(()),
            Err(IdentityError::InvalidCredential) => Err(McpAuthFailure::PermissionDenied),
            Err(_) => Err(McpAuthFailure::Unavailable),
        }
    }
}

/// Startup-time errors while building [`McpAuthConfig`].
#[derive(Debug, Error)]
pub enum McpAuthConfigError {
    /// The env var exists but is blank after trimming.
    #[error("{MCP_API_KEY_ENV} must not be empty or whitespace-only")]
    EmptyBootstrapKey,
    /// Reading the env var failed.
    #[error("read {MCP_API_KEY_ENV}: {0}")]
    ReadEnvironment(#[source] std::env::VarError),
    /// Hashing the bootstrap credential failed.
    #[error("hash bootstrap credential: {0}")]
    HashFailed(#[source] IdentityError),
}

/// Redacted failure reasons for MCP bootstrap credential authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpAuthFailure {
    /// Credential store/bootstrap key is not configured.
    Unavailable,
    /// No credential was presented.
    AuthRequired,
    /// A credential was presented but not authorized.
    PermissionDenied,
}

impl McpAuthFailure {
    /// Stable wire error code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::AuthRequired => "auth_required",
            Self::PermissionDenied => "permission_denied",
        }
    }

    /// Human-readable, redacted summary.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::Unavailable => {
                "MCP credential store is not configured. Set TANREN_MCP_API_KEY (bootstrap key) until R-0008 lands the real store."
            }
            Self::AuthRequired => "Missing Authorization: Bearer <api-key> or X-API-Key header.",
            Self::PermissionDenied => {
                "Presented credential is not authorized for this MCP service."
            }
        }
    }
}

//! Install profile/integration typing and catalog plumbing.

use std::collections::BTreeSet;
use std::str::FromStr;

pub mod catalog;
pub mod error;
pub mod manifest;

pub use error::InstallError;

/// Supported Tanren standards profiles for local repository bootstrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InstallProfile {
    /// Install the Rust + Cargo standards profile.
    RustCargo,
}

impl InstallProfile {
    /// Canonical profile identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RustCargo => "rust-cargo",
        }
    }
}

impl FromStr for InstallProfile {
    type Err = InstallError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "rust-cargo" => Ok(Self::RustCargo),
            unknown => Err(InstallError::UnsupportedProfile {
                name: unknown.to_owned(),
            }),
        }
    }
}

/// Supported agent integration targets for generated command assets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum InstallIntegration {
    Claude,
    Codex,
    OpenCode,
}

impl InstallIntegration {
    /// Canonical integration identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::OpenCode => "opencode",
        }
    }

    /// Return all supported integrations.
    #[must_use]
    pub fn all() -> BTreeSet<Self> {
        [Self::Claude, Self::Codex, Self::OpenCode]
            .into_iter()
            .collect()
    }
}

impl FromStr for InstallIntegration {
    type Err = InstallError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            "opencode" => Ok(Self::OpenCode),
            unknown => Err(InstallError::UnsupportedIntegration {
                name: unknown.to_owned(),
            }),
        }
    }
}

/// Parse integration selection from comma-separated CLI values.
///
/// The parser validates all names before install planning. `None` means
/// "install all supported integrations".
pub fn parse_integration_selection(
    selection: Option<&str>,
) -> Result<BTreeSet<InstallIntegration>, InstallError> {
    let Some(raw_selection) = selection else {
        return Ok(InstallIntegration::all());
    };

    let mut selected = BTreeSet::new();
    for raw_token in raw_selection.split(',') {
        let token = raw_token.trim();
        if token.is_empty() {
            return Err(InstallError::EmptyIntegrationSelection);
        }
        selected.insert(token.parse()?);
    }

    if selected.is_empty() {
        return Err(InstallError::EmptyIntegrationSelection);
    }

    Ok(selected)
}

//! Canonical install/drift contract types shared between production and testkit.
//!
//! Types in this module are the single source of truth for:
//! - Repository-relative path validation ([`RepoRelativePath`])
//! - SHA-256 hex digest ([`Sha256Hex`])
//! - Profile and integration enums ([`InstallProfile`], [`InstallIntegration`])
//! - Manifest entry and manifest structures ([`ManifestEntry`], [`InstallManifest`])
//! - Install selection parsing ([`parse_integration_selection`])
//!
//! Both `tanren-cli-app` and `tanren-testkit` re-export from this module so
//! there is exactly one definition site for each contract type.

use std::collections::BTreeSet;
use std::fmt;
use std::fs::File;
use std::io::Read as _;
use std::path::{Component, Path};
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use thiserror::Error;

const NIBBLES: &[u8; 16] = b"0123456789abcdef";
const SHA256_HEX_LENGTH: usize = 64;

/// Install manifest schema version.
pub const INSTALL_MANIFEST_VERSION: u32 = 1;
/// Repo-local metadata path for persisted install state.
pub const INSTALL_MANIFEST_REPO_PATH: &str = ".tanren/install-manifest.toml";

/// Supported Tanren standards profiles for local repository bootstrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
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

impl fmt::Display for InstallProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for InstallProfile {
    type Err = InstallContractError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "rust-cargo" => Ok(Self::RustCargo),
            unknown => Err(InstallContractError::UnsupportedProfile {
                name: unknown.to_owned(),
            }),
        }
    }
}

/// Supported agent integration targets for generated command assets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
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
            Self::OpenCode => "open-code",
        }
    }

    /// Destination root for generated command assets.
    #[must_use]
    pub const fn destination_root(self) -> &'static str {
        match self {
            Self::Claude => ".claude/commands/",
            Self::Codex => ".codex/skills/",
            Self::OpenCode => ".opencode/commands/",
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
    type Err = InstallContractError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            "open-code" | "opencode" => Ok(Self::OpenCode),
            unknown => Err(InstallContractError::UnsupportedIntegration {
                name: unknown.to_owned(),
            }),
        }
    }
}

/// Installed-asset classification used by install drift and apply planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssetClass {
    /// Tanren methodology command material rendered per integration.
    MethodologyCommand,
    /// Standards profile guidance file.
    StandardsProfile,
}

/// Preservation contract for install/update behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PreservationPolicy {
    /// Tanren-owned generated file that can be replaced on re-install.
    ReplaceGenerated,
    /// User-editable standards file that should not be overwritten silently.
    PreserveUserEdits,
}

/// Strict repository-relative path (no absolute roots, no `..` traversal).
///
/// `CurDir` (`.`) segments are stripped during parsing so that paths like
/// `./.codex/skills/x.md` and `.codex/skills/x.md` produce the same
/// canonical value.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepoRelativePath(String);

impl RepoRelativePath {
    /// Validate and construct a normalized repository-relative path.
    ///
    /// `CurDir` (`.`) segments are stripped so that every accepted input
    /// produces a canonical string. The normalized form contains only
    /// `Component::Normal` segments joined by `/`.
    pub fn parse(path: &str) -> Result<Self, InstallContractError> {
        if path.is_empty() {
            return Err(InstallContractError::InvalidRepoRelativePath {
                path: path.to_owned(),
            });
        }

        let candidate = Path::new(path);
        if candidate.is_absolute() {
            return Err(InstallContractError::InvalidRepoRelativePath {
                path: path.to_owned(),
            });
        }

        let mut normal_segments: Vec<&std::ffi::OsStr> = Vec::new();
        for component in candidate.components() {
            match component {
                Component::Normal(segment) => normal_segments.push(segment),
                Component::CurDir => {}
                _ => {
                    return Err(InstallContractError::InvalidRepoRelativePath {
                        path: path.to_owned(),
                    });
                }
            }
        }

        if normal_segments.is_empty() {
            return Err(InstallContractError::InvalidRepoRelativePath {
                path: path.to_owned(),
            });
        }

        let normalized = normal_segments
            .iter()
            .map(|segment| segment.to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");

        Ok(Self(normalized))
    }

    /// Borrow the validated path string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Borrow as a [`Path`].
    #[must_use]
    pub fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }
}

impl Serialize for RepoRelativePath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RepoRelativePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        RepoRelativePath::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// Parse failure for [`Sha256Hex`].
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("content hash must be exactly {SHA256_HEX_LENGTH} lowercase hex characters")]
pub struct Sha256HexParseError;

/// Strict lowercase SHA-256 digest encoded as 64 hex characters.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sha256Hex(String);

impl Sha256Hex {
    /// Validate and construct a SHA-256 hex digest.
    pub fn parse(value: &str) -> Result<Self, Sha256HexParseError> {
        if value.len() != SHA256_HEX_LENGTH
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(Sha256HexParseError);
        }

        Ok(Self(value.to_owned()))
    }

    /// Borrow the validated digest string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Sha256Hex {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for Sha256Hex {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Sha256Hex {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Sha256Hex::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// Manifest row written/checked by future install workflows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub path: RepoRelativePath,
    pub content_hash: Sha256Hex,
    pub asset_class: AssetClass,
    pub integration: Option<InstallIntegration>,
    pub preservation: PreservationPolicy,
}

/// Install manifest stored under repo-local metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallManifest {
    pub manifest_version: u32,
    pub profile: InstallProfile,
    pub integrations: Vec<InstallIntegration>,
    pub entries: Vec<ManifestEntry>,
}

impl InstallManifest {
    /// Create a deterministic manifest from typed install inputs.
    #[must_use]
    pub fn from_entries(
        profile: InstallProfile,
        integrations: Vec<InstallIntegration>,
        entries: Vec<ManifestEntry>,
    ) -> Self {
        Self {
            manifest_version: INSTALL_MANIFEST_VERSION,
            profile,
            integrations,
            entries,
        }
    }
}

/// Parse integration selection from comma-separated CLI values.
///
/// The parser validates all names before install planning. `None` means
/// "install all supported integrations".
pub fn parse_integration_selection(
    selection: Option<&str>,
) -> Result<BTreeSet<InstallIntegration>, InstallContractError> {
    let Some(raw_selection) = selection else {
        return Ok(InstallIntegration::all());
    };

    let mut selected = BTreeSet::new();
    for raw_token in raw_selection.split(',') {
        let token = raw_token.trim();
        if token.is_empty() {
            return Err(InstallContractError::EmptyIntegrationSelection);
        }
        selected.insert(token.parse()?);
    }

    if selected.is_empty() {
        return Err(InstallContractError::EmptyIntegrationSelection);
    }

    Ok(selected)
}

/// Validated install/drift selection pairing a profile with its integration set.
///
/// Both `InstallCommand::run` and `DriftCommand::run` delegate selection
/// parsing to `[InstallSelection::parse]` so there is a single code path
/// from raw CLI strings to typed inputs.
#[derive(Debug, Clone)]
pub struct InstallSelection {
    profile: InstallProfile,
    integrations: BTreeSet<InstallIntegration>,
}

impl InstallSelection {
    /// Parse a profile name and optional comma-separated integration selection.
    ///
    /// When `integrations` is `None` the full set of supported integrations
    /// is selected via `[InstallIntegration::all]`.
    pub fn parse(profile: &str, integrations: Option<&str>) -> Result<Self, InstallContractError> {
        Ok(Self {
            profile: InstallProfile::from_str(profile)?,
            integrations: parse_integration_selection(integrations)?,
        })
    }

    /// The resolved profile.
    #[must_use]
    pub const fn profile(&self) -> InstallProfile {
        self.profile
    }

    /// The resolved integration set.
    #[must_use]
    pub fn integrations(&self) -> &BTreeSet<InstallIntegration> {
        &self.integrations
    }

    /// Comma-separated integration identifiers (deterministic order).
    #[must_use]
    pub fn integrations_csv(&self) -> String {
        self.integrations
            .iter()
            .map(|integration| integration.as_str())
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// Hash bytes as lowercase SHA-256 hex.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> Sha256Hex {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push(char::from(NIBBLES[(byte >> 4) as usize]));
        hex.push(char::from(NIBBLES[(byte & 0x0f) as usize]));
    }
    Sha256Hex(hex)
}

/// Streaming SHA-256 hash of a file using a fixed-size 64 KiB buffer.
///
/// Memory usage is independent of file size. Produces the same hex digest
/// as [`sha256_hex`] on identical byte content.
pub fn sha256_file_streaming(path: &Path) -> std::io::Result<Sha256Hex> {
    let mut reader = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push(char::from(NIBBLES[(byte >> 4) as usize]));
        hex.push(char::from(NIBBLES[(byte & 0x0f) as usize]));
    }
    Ok(Sha256Hex(hex))
}

/// Errors raised during install contract validation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InstallContractError {
    /// The requested standards profile is not supported.
    #[error("unsupported install profile '{name}'")]
    UnsupportedProfile { name: String },
    /// The requested integration is not supported.
    #[error("unsupported install integration '{name}'")]
    UnsupportedIntegration { name: String },
    /// Integration selection was provided but contained no integration names.
    #[error("integration selection is empty")]
    EmptyIntegrationSelection,
    /// The catalog path is not repository-relative.
    #[error("catalog path must be repo-relative and cannot contain parent traversal: '{path}'")]
    InvalidRepoRelativePath { path: String },
}

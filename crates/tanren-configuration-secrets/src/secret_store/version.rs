use super::CredentialSealingFailure;

/// Supported credential seal contract versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialSealVersion {
    /// Version 1 user-credential sealing contract.
    V1,
}

impl CredentialSealVersion {
    pub(crate) const CURRENT: Self = Self::V1;

    /// Stable integer value used for persistence.
    #[must_use]
    pub const fn as_i16(self) -> i16 {
        match self {
            Self::V1 => 1,
        }
    }

    pub(crate) fn parse(value: i16) -> Result<Self, CredentialSealingFailure> {
        match value {
            1 => Ok(Self::V1),
            version => Err(CredentialSealingFailure::UnsupportedSealVersion { version }),
        }
    }
}

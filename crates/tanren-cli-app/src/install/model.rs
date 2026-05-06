use std::path::PathBuf;

use thiserror::Error;

const SUPPORTED_PROFILES: &[&str] = &["default", "react-ts-pnpm", "rust-cargo"];
const SUPPORTED_INTEGRATIONS: &[&str] = &["claude", "codex", "opencode"];

#[derive(Debug, Clone)]
pub(crate) struct ProfileName(String);

impl ProfileName {
    pub(crate) fn parse(input: &str) -> Result<Self, InstallValidationError> {
        if SUPPORTED_PROFILES.contains(&input) {
            Ok(Self(input.to_owned()))
        } else {
            Err(InstallValidationError::UnknownProfile {
                input: input.to_owned(),
                supported: SUPPORTED_PROFILES.join(", "),
            })
        }
    }
}

impl std::fmt::Display for ProfileName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum IntegrationName {
    Claude,
    Codex,
    Opencode,
}

impl IntegrationName {
    pub(crate) fn parse(input: &str) -> Result<Self, InstallValidationError> {
        match input {
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            "opencode" => Ok(Self::Opencode),
            _ => Err(InstallValidationError::UnknownIntegration {
                input: input.to_owned(),
                supported: SUPPORTED_INTEGRATIONS.join(", "),
            }),
        }
    }
}

pub(crate) struct InstallInput {
    pub(crate) repo: PathBuf,
    pub(crate) profile: ProfileName,
    pub(crate) integrations: Vec<IntegrationName>,
}

impl InstallInput {
    pub(crate) fn effective_integrations(&self) -> Vec<IntegrationName> {
        if self.integrations.is_empty() {
            vec![
                IntegrationName::Claude,
                IntegrationName::Codex,
                IntegrationName::Opencode,
            ]
        } else {
            self.integrations.clone()
        }
    }
}

#[derive(Debug, Error)]
pub(crate) enum InstallValidationError {
    #[error("unknown profile '{input}' — supported profiles: {supported}")]
    UnknownProfile { input: String, supported: String },
    #[error("unknown integration '{input}' — supported integrations: {supported}")]
    UnknownIntegration { input: String, supported: String },
    #[error("repository path does not exist: {0}")]
    RepoNotFound(PathBuf),
    #[error("repository path is not a directory: {0}")]
    NotADirectory(PathBuf),
}

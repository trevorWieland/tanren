//! Bounded typed inputs for project flows.
//!
//! [`ProjectName`], [`ProviderHost`], and [`RepositoryUrl`] validate
//! length, format, and safety constraints *before* any provider or
//! store call. Repository identity normalization lives here as the
//! single shared implementation consumed by app services and test
//! fixtures.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;

const PROJECT_NAME_MAX: usize = 100;
const PROVIDER_HOST_MAX: usize = 253;
const REPOSITORY_URL_MAX: usize = 2048;

/// Validation errors for project-flow bounded inputs.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ProjectInputError {
    #[error("project name is empty")]
    EmptyName,
    #[error("project name exceeds {PROJECT_NAME_MAX} bytes")]
    NameTooLong,
    #[error("project name contains control characters")]
    NameControlChar,
    #[error("provider host is empty")]
    EmptyHost,
    #[error("provider host exceeds {PROVIDER_HOST_MAX} bytes")]
    HostTooLong,
    #[error("repository URL is empty")]
    EmptyUrl,
    #[error("repository URL exceeds {REPOSITORY_URL_MAX} bytes")]
    UrlTooLong,
    #[error("repository URL is not a valid HTTPS or SSH URL")]
    InvalidUrl,
    #[error("repository URL contains credentials")]
    UrlCredentials,
    #[error("repository URL contains a query string")]
    UrlQuery,
    #[error("repository URL contains a fragment")]
    UrlFragment,
    #[error("repository URL uses an unsupported scheme")]
    UrlUnsupportedScheme,
}

/// Validated, bounded project name. [`ProjectName::parse`] trims, checks
/// length ≤ 100, and rejects control characters. Serializes transparently.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String)]
pub struct ProjectName(String);

impl ProjectName {
    /// Parse and validate a raw project name.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectInputError`] when the name is empty, too long,
    /// or contains control characters.
    pub fn parse(raw: &str) -> Result<Self, ProjectInputError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ProjectInputError::EmptyName);
        }
        if trimmed.len() > PROJECT_NAME_MAX {
            return Err(ProjectInputError::NameTooLong);
        }
        if trimmed.chars().any(char::is_control) {
            return Err(ProjectInputError::NameControlChar);
        }
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ProjectName {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for ProjectName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Validated, bounded provider host. [`ProviderHost::parse`] trims,
/// lowercases, and checks length ≤ 253 (RFC 1035). Serializes transparently.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String)]
pub struct ProviderHost(String);

impl ProviderHost {
    /// Parse and validate a raw provider host.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectInputError`] when the host is empty or too long.
    pub fn parse(raw: &str) -> Result<Self, ProjectInputError> {
        let trimmed = raw.trim().to_lowercase();
        if trimmed.is_empty() {
            return Err(ProjectInputError::EmptyHost);
        }
        if trimmed.len() > PROVIDER_HOST_MAX {
            return Err(ProjectInputError::HostTooLong);
        }
        Ok(Self(trimmed))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ProviderHost {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for ProviderHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Validated, bounded repository URL.
///
/// [`RepositoryUrl::parse`] accepts HTTPS, HTTP (dev/test), and SSH forms
/// (`ssh://` and `git@host:path`). URLs with embedded credentials, query
/// strings, or fragments are rejected at parse time so they can never reach
/// storage, events, logs, or UI views. Use [`RepositoryUrl::redacted`]
/// for display in events and logs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String)]
pub struct RepositoryUrl(String);

impl RepositoryUrl {
    /// Parse and validate a raw repository URL.
    ///
    /// Accepted forms: `https://host/path`, `http://host/path`,
    /// `ssh://[git@]host/path`, `git@host:path`.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectInputError`] when the URL is empty, too long,
    /// malformed, contains credentials/query/fragment, or uses an
    /// unsupported scheme.
    pub fn parse(raw: &str) -> Result<Self, ProjectInputError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ProjectInputError::EmptyUrl);
        }
        if trimmed.len() > REPOSITORY_URL_MAX {
            return Err(ProjectInputError::UrlTooLong);
        }
        if let Ok(parsed) = url::Url::parse(trimmed) {
            Self::validate_parsed_url(&parsed)?;
            return Ok(Self(trimmed.to_owned()));
        }
        if let Some(rest) = trimmed.strip_prefix("git@") {
            Self::validate_scp_form(rest)?;
            return Ok(Self(trimmed.to_owned()));
        }
        Err(ProjectInputError::InvalidUrl)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Extract the host from this validated URL. Returns `None` only if
    /// the URL somehow lacks a host (should not happen after `parse`).
    #[must_use]
    pub fn host(&self) -> Option<&str> {
        extract_host(&self.0)
    }

    /// Produce a redacted form safe for events, logs, and UI views.
    /// Replaces the path with `***`. Example:
    /// `https://github.com/org/repo` → `https://github.com/***`.
    #[must_use]
    pub fn redacted(&self) -> String {
        if let Some(host) = self.host() {
            if self.0.starts_with("git@") {
                return format!("git@{host}:***");
            }
            if let Ok(parsed) = url::Url::parse(&self.0) {
                return format!("{}://{}/***", parsed.scheme(), host);
            }
        }
        "***".to_owned()
    }

    fn validate_parsed_url(parsed: &url::Url) -> Result<(), ProjectInputError> {
        match parsed.scheme() {
            "https" | "http" => {
                if !parsed.username().is_empty() || parsed.password().is_some() {
                    return Err(ProjectInputError::UrlCredentials);
                }
            }
            "ssh" => {
                if parsed.password().is_some() {
                    return Err(ProjectInputError::UrlCredentials);
                }
                let user = parsed.username();
                if !user.is_empty() && user != "git" {
                    return Err(ProjectInputError::UrlCredentials);
                }
            }
            _ => return Err(ProjectInputError::UrlUnsupportedScheme),
        }
        if parsed.query().is_some() {
            return Err(ProjectInputError::UrlQuery);
        }
        if parsed.fragment().is_some() {
            return Err(ProjectInputError::UrlFragment);
        }
        if parsed.host_str().is_none_or(str::is_empty) {
            return Err(ProjectInputError::InvalidUrl);
        }
        Ok(())
    }

    fn validate_scp_form(rest: &str) -> Result<(), ProjectInputError> {
        let Some(colon) = rest.find(':') else {
            return Err(ProjectInputError::InvalidUrl);
        };
        let host = &rest[..colon];
        let path = &rest[colon + 1..];
        if host.is_empty() || path.is_empty() {
            return Err(ProjectInputError::InvalidUrl);
        }
        if path.contains('?') {
            return Err(ProjectInputError::UrlQuery);
        }
        if path.contains('#') {
            return Err(ProjectInputError::UrlFragment);
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for RepositoryUrl {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for RepositoryUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn extract_host(url: &str) -> Option<&str> {
    if let Some(stripped) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| url.strip_prefix("ssh://"))
    {
        let after_user = if stripped.contains('@') {
            stripped.split('@').nth(1)?
        } else {
            stripped
        };
        let host = after_user.split(['/', ':']).next()?;
        return if host.is_empty() { None } else { Some(host) };
    }
    if let Some(rest) = url.strip_prefix("git@") {
        let host = rest.split([':', '/']).next()?;
        return if host.is_empty() { None } else { Some(host) };
    }
    None
}

/// Compute a canonical identity string from a repository URL.
///
/// Normalization strips scheme/prefix, lowercases, replaces `:` with `/`,
/// and strips `.git` suffix and trailing `/` so that equivalent URLs
/// compare equal. This is the single shared implementation used by
/// app services and the BDD harness — no other copy should exist.
#[must_use]
pub fn normalize_repository_identity(url: &str) -> String {
    if let Ok(parsed) = url::Url::parse(url) {
        let host = parsed.host_str().unwrap_or("");
        let path = parsed.path().trim_end_matches('/');
        let path = path.strip_suffix(".git").unwrap_or(path);
        return format!("{host}{path}").to_lowercase();
    }
    let stripped = url.strip_prefix("git@").unwrap_or(url);
    let replaced = stripped.replace(':', "/");
    let trimmed = replaced.strip_suffix(".git").unwrap_or(replaced.as_str());
    trimmed.trim_end_matches('/').to_lowercase()
}

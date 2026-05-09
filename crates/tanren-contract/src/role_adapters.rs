//! Shared role scope/principal adapters for interface surfaces.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::{
    AccountId, OrgId, PermissionScope, PrincipalRef, ProjectId, RoleId, RoleScope,
};
use thiserror::Error;
use utoipa::ToSchema;
use uuid::Uuid;

/// Scope kind adapter shared by interface clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RoleScopeKind {
    /// Account-level scope.
    Account,
    /// Organization-level scope.
    Organization,
    /// Project-level scope.
    Project,
}

impl RoleScopeKind {
    /// Parse a scope kind from user-provided text.
    ///
    /// # Errors
    ///
    /// Returns [`RoleAdapterError::InvalidKind`] when the value is not
    /// `account`, `organization`, or `project`.
    pub fn parse(raw: &str, field_name: &'static str) -> Result<Self, RoleAdapterError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "account" => Ok(Self::Account),
            "organization" => Ok(Self::Organization),
            "project" => Ok(Self::Project),
            _ => Err(RoleAdapterError::InvalidKind {
                field_name,
                expected: "account, organization, or project",
            }),
        }
    }

    /// Render this scope kind as its canonical `snake_case` wire value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Account => "account",
            Self::Organization => "organization",
            Self::Project => "project",
        }
    }

    /// Build a role scope from this kind and a parsed UUID.
    #[must_use]
    pub const fn into_role_scope(self, id: Uuid) -> RoleScope {
        match self {
            Self::Account => RoleScope::Account {
                account_id: AccountId::new(id),
            },
            Self::Organization => RoleScope::Organization {
                org_id: OrgId::new(id),
            },
            Self::Project => RoleScope::Project {
                project_id: ProjectId::new(id),
            },
        }
    }

    /// Build a permission scope from this kind and a parsed UUID.
    #[must_use]
    pub const fn into_permission_scope(self, id: Uuid) -> PermissionScope {
        match self {
            Self::Account => PermissionScope::Account {
                account_id: AccountId::new(id),
            },
            Self::Organization => PermissionScope::Organization {
                org_id: OrgId::new(id),
            },
            Self::Project => PermissionScope::Project {
                project_id: ProjectId::new(id),
            },
        }
    }
}

/// Principal kind adapter shared by interface clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalKind {
    /// Account principal.
    Account,
    /// Role principal.
    Role,
}

impl PrincipalKind {
    /// Parse a principal kind from user-provided text.
    ///
    /// # Errors
    ///
    /// Returns [`RoleAdapterError::InvalidKind`] when the value is not
    /// `account` or `role`.
    pub fn parse(raw: &str, field_name: &'static str) -> Result<Self, RoleAdapterError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "account" => Ok(Self::Account),
            "role" => Ok(Self::Role),
            _ => Err(RoleAdapterError::InvalidKind {
                field_name,
                expected: "account or role",
            }),
        }
    }

    /// Render this principal kind as its canonical `snake_case` wire value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Account => "account",
            Self::Role => "role",
        }
    }

    /// Build a principal reference from this kind and a parsed UUID.
    #[must_use]
    pub const fn into_principal(self, id: Uuid) -> PrincipalRef {
        match self {
            Self::Account => PrincipalRef::Account {
                account_id: AccountId::new(id),
            },
            Self::Role => PrincipalRef::Role {
                role_id: RoleId::new(id),
            },
        }
    }
}

/// Parsing and projection failures for role interface adapters.
#[derive(Debug, Error)]
pub enum RoleAdapterError {
    /// Kind field does not match the closed role adapter taxonomy.
    #[error("{field_name} must be {expected}")]
    InvalidKind {
        /// Field name used in error rendering.
        field_name: &'static str,
        /// Accepted values for this field.
        expected: &'static str,
    },
    /// UUID field is invalid.
    #[error("parse {field_name} as uuid: {source}")]
    InvalidUuid {
        /// Field name used in error rendering.
        field_name: &'static str,
        /// UUID parse error.
        source: uuid::Error,
    },
}

/// Parse a UUID field for role adapters.
///
/// # Errors
///
/// Returns [`RoleAdapterError::InvalidUuid`] when the supplied value is not a
/// UUID.
pub fn parse_uuid_field(raw: &str, field_name: &'static str) -> Result<Uuid, RoleAdapterError> {
    Uuid::parse_str(raw.trim())
        .map_err(|source| RoleAdapterError::InvalidUuid { field_name, source })
}

/// Parse a role identifier for role adapters.
///
/// # Errors
///
/// Returns [`RoleAdapterError::InvalidUuid`] when the supplied value is not a
/// UUID.
pub fn parse_role_id_field(
    raw: &str,
    field_name: &'static str,
) -> Result<RoleId, RoleAdapterError> {
    parse_uuid_field(raw, field_name).map(RoleId::new)
}

/// Parse a role scope from `kind + id` text fields.
///
/// # Errors
///
/// Returns [`RoleAdapterError`] when kind or id parsing fails.
pub fn parse_role_scope_field(
    kind: &str,
    id: &str,
    kind_field_name: &'static str,
    id_field_name: &'static str,
) -> Result<RoleScope, RoleAdapterError> {
    let kind = RoleScopeKind::parse(kind, kind_field_name)?;
    let id = parse_uuid_field(id, id_field_name)?;
    Ok(kind.into_role_scope(id))
}

/// Parse a permission scope from `kind + id` text fields.
///
/// # Errors
///
/// Returns [`RoleAdapterError`] when kind or id parsing fails.
pub fn parse_permission_scope_field(
    kind: &str,
    id: &str,
    kind_field_name: &'static str,
    id_field_name: &'static str,
) -> Result<PermissionScope, RoleAdapterError> {
    let kind = RoleScopeKind::parse(kind, kind_field_name)?;
    let id = parse_uuid_field(id, id_field_name)?;
    Ok(kind.into_permission_scope(id))
}

/// Parse a principal from `kind + id` text fields.
///
/// # Errors
///
/// Returns [`RoleAdapterError`] when kind or id parsing fails.
pub fn parse_principal_field(
    kind: &str,
    id: &str,
    kind_field_name: &'static str,
    id_field_name: &'static str,
) -> Result<PrincipalRef, RoleAdapterError> {
    let kind = PrincipalKind::parse(kind, kind_field_name)?;
    let id = parse_uuid_field(id, id_field_name)?;
    Ok(kind.into_principal(id))
}

/// Render a role scope using `scope_kind:id` format.
#[must_use]
pub fn format_role_scope(scope: RoleScope) -> String {
    match scope {
        RoleScope::Account { account_id } => format!("account:{account_id}"),
        RoleScope::Organization { org_id } => format!("organization:{org_id}"),
        RoleScope::Project { project_id } => format!("project:{project_id}"),
    }
}

/// Render a permission scope using `scope_kind:id` format.
#[must_use]
pub fn format_permission_scope(scope: PermissionScope) -> String {
    match scope {
        PermissionScope::Account { account_id } => format!("account:{account_id}"),
        PermissionScope::Organization { org_id } => format!("organization:{org_id}"),
        PermissionScope::Project { project_id } => format!("project:{project_id}"),
    }
}

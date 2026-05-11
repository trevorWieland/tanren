//! Typed unique-constraint classification for organization-create writes.

use sea_orm::DbErr;

/// Unique constraints relevant to organization create operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OrganizationCreateConstraint {
    /// Unique index on `organizations.name`.
    OrganizationName,
    /// Composite primary key on `(account_id, idempotency_key)`.
    IdempotencyKey,
}

/// Classify unique-constraint failures for organization-create writes.
#[must_use]
pub(crate) fn classify_organization_create_constraint(
    err: &DbErr,
) -> Option<OrganizationCreateConstraint> {
    let Some(sea_orm::SqlErr::UniqueConstraintViolation(details)) = err.sql_err() else {
        return None;
    };

    // Postgres reports index/constraint names (`idx_organizations_name_unique`,
    // `organization_create_idempotency_pkey`); SQLite reports
    // `table.column` tuples. Match both stable shapes without falling back to
    // free-form lowercase message probes.
    if details.contains("idx_organizations_name_unique") || details.contains("organizations.name") {
        return Some(OrganizationCreateConstraint::OrganizationName);
    }
    if details.contains("organization_create_idempotency_pkey")
        || details
            .contains("organization_create_idempotency.account_id, organization_create_idempotency.idempotency_key")
    {
        return Some(OrganizationCreateConstraint::IdempotencyKey);
    }

    None
}

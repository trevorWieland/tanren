//! Helpers for classifying constraint conflicts from `SeaORM` database errors.

use sea_orm::{DbErr, RuntimeErr, sqlx};

const ACCOUNTS_IDENTIFIER_UNIQUE_CONSTRAINT: &str = "accounts_identifier_key";
const PROJECTS_SINGLE_ACTIVE_PER_ACCOUNT_INDEX: &str = "idx_projects_single_active_per_account";
const PROJECT_REPOSITORIES_OWNING_ACCOUNT_REPO_UNIQUE_INDEX: &str =
    "idx_project_repositories_owning_account_repo_unique";
const PROJECT_COMMAND_RESERVATIONS_PRIMARY_KEY: &str = "project_command_reservations_pkey";

/// Structured metadata for a unique-constraint failure.
#[derive(Debug, Clone, Copy)]
pub(crate) struct UniqueViolationMetadata<'a> {
    pub(crate) constraint: Option<&'a str>,
    pub(crate) table: Option<&'a str>,
    pub(crate) message: &'a str,
}

/// Return structured metadata when `err` is a unique-constraint violation.
pub(crate) fn unique_violation_metadata(err: &DbErr) -> Option<UniqueViolationMetadata<'_>> {
    let db_err = database_error(err)?;
    if !db_err.is_unique_violation() {
        return None;
    }
    Some(UniqueViolationMetadata {
        constraint: db_err.constraint(),
        table: db_err.table(),
        message: db_err.message(),
    })
}

/// Match a unique-constraint conflict by a known backend constraint/index name.
pub(crate) fn unique_violation_matches_named_constraint(
    err: &DbErr,
    constraint_name: &str,
) -> bool {
    let Some(meta) = unique_violation_metadata(err) else {
        return false;
    };
    meta.constraint
        .is_some_and(|actual| actual.eq_ignore_ascii_case(constraint_name))
        || message_contains_token(meta.message, constraint_name)
}

/// Match a unique-constraint conflict by a known `table.column` signature.
pub(crate) fn unique_violation_matches_table_columns(
    err: &DbErr,
    table: &str,
    columns: &[&str],
) -> bool {
    let Some(meta) = unique_violation_metadata(err) else {
        return false;
    };

    if meta
        .table
        .is_some_and(|actual| actual.eq_ignore_ascii_case(table))
        && columns
            .iter()
            .all(|column| message_contains_token(meta.message, column))
    {
        return true;
    }

    let expected_columns = table_columns_signature(table, columns);
    message_contains_token(meta.message, &expected_columns)
}

/// Account identifier conflict classifier for account-row insert paths.
pub(crate) fn is_duplicate_account_identifier_conflict(err: &DbErr) -> bool {
    unique_violation_matches_named_constraint(err, ACCOUNTS_IDENTIFIER_UNIQUE_CONSTRAINT)
        || unique_violation_matches_table_columns(err, "accounts", &["identifier"])
}

/// Project repository duplicate conflict classifier.
pub(crate) fn is_project_repository_unique_conflict(err: &DbErr) -> bool {
    unique_violation_matches_named_constraint(
        err,
        PROJECT_REPOSITORIES_OWNING_ACCOUNT_REPO_UNIQUE_INDEX,
    ) || unique_violation_matches_table_columns(
        err,
        "project_repositories",
        &["owning_account_id", "provider_family", "repository_ref"],
    )
}

/// Active-project uniqueness conflict classifier.
pub(crate) fn is_projects_single_active_unique_conflict(err: &DbErr) -> bool {
    unique_violation_matches_named_constraint(err, PROJECTS_SINGLE_ACTIVE_PER_ACCOUNT_INDEX)
        || unique_violation_matches_table_columns(
            err,
            "projects",
            &["owning_account_id", "active_selection_guard"],
        )
}

/// Project-command reservation key conflict classifier.
pub(crate) fn is_project_command_reservation_unique_conflict(err: &DbErr) -> bool {
    unique_violation_matches_named_constraint(err, PROJECT_COMMAND_RESERVATIONS_PRIMARY_KEY)
        || unique_violation_matches_table_columns(
            err,
            "project_command_reservations",
            &["owning_account_id", "provider_family", "repository_ref"],
        )
}

fn database_error(err: &DbErr) -> Option<&(dyn sqlx::error::DatabaseError + 'static)> {
    match err {
        DbErr::Exec(RuntimeErr::SqlxError(sqlx::Error::Database(db_err)))
        | DbErr::Query(RuntimeErr::SqlxError(sqlx::Error::Database(db_err))) => {
            Some(db_err.as_ref())
        }
        _ => None,
    }
}

fn table_columns_signature(table: &str, columns: &[&str]) -> String {
    columns
        .iter()
        .map(|column| format!("{table}.{column}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn message_contains_token(haystack: &str, needle: &str) -> bool {
    haystack
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

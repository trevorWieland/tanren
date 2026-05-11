//! Shared utility helpers for the role store adapter.

use std::collections::BTreeSet;

use sea_orm::{DbErr, SqlErr};
use tanren_identity_policy::PermissionName;

use crate::{ROLE_GRANT_LIST_PAGE_MAX, StoreError};

/// Chunk size for large role-permission and apply-role insert batches.
pub(crate) const ROLE_GRANT_INSERT_BATCH_SIZE: usize = 200;

pub(crate) fn dedup_permission_names(permissions: &[PermissionName]) -> Vec<String> {
    permissions
        .iter()
        .map(|permission| permission.as_str().to_owned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
}

pub(crate) fn is_unique_violation(err: &DbErr) -> bool {
    matches!(err.sql_err(), Some(SqlErr::UniqueConstraintViolation(_)))
}

pub(crate) fn map_store_txn_error(err: sea_orm::TransactionError<StoreError>) -> StoreError {
    match err {
        sea_orm::TransactionError::Connection(db_err) => StoreError::from(db_err),
        sea_orm::TransactionError::Transaction(inner) => inner,
    }
}

pub(crate) fn map_wrapped_txn_error<E>(err: sea_orm::TransactionError<E>) -> E
where
    E: From<StoreError>,
{
    match err {
        sea_orm::TransactionError::Connection(db_err) => StoreError::from(db_err).into(),
        sea_orm::TransactionError::Transaction(inner) => inner,
    }
}

pub(crate) fn clamp_page_limit(limit: u64) -> u64 {
    limit.clamp(1, ROLE_GRANT_LIST_PAGE_MAX)
}
